"""Operation-local byte verification for unchanged routing metadata validators.

Only the four named metadata entrypoints below memoize successful return values.
Science, qualification, witness and decision entrypoints are never memoized.
``digest`` is an explicit dependency-verifier callback for ready-wrapper readers;
it must not replace mutable native-output or per-call verification. Call
``before_producer`` after admission and immediately before math/native work.
Context exit fully rehashes the read closure before returning a successful report.

This is an integrity guard for the maintained, single-threaded validators, not a
sandbox for arbitrary Python. The audit dispatcher remains registered (Python
cannot unregister it), but has no active operation after a metadata call exits.
All installed function aliases are restored when the operation closes.
"""
from __future__ import annotations

import contextvars
import copy
import functools
import hashlib
import importlib
import json
import math
import os
from pathlib import Path
import stat
import sys
import threading
import time
import types

_ACTIVE = contextvars.ContextVar('routing_metadata_validation', default=None)
_INSTALLED = None
_METADATA = (('routing_blocks', ('load_context', 'context_bindings')),
             ('routing_block_deployment', ('activation_core', 'context_bindings')))
_DIGESTS = (('native_io', 'digest'), ('time_exact', 'sha256_file'),
            ('time_blocks', 'digest'), ('block_host', 'digest'))
# These frozen validators import helpers lazily. Load code before auditing reads:
# an importer may probe nonexistent pyc paths or attempt to write bytecode.
_PRELOAD = ('routing_parallel', 'routing_window', 'routing_science_completion',
            'routing_cases', 'routing_oracle', 'cmgschur')


def _full_digest(path, observed=None):
    result = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b''):
            if observed is not None:
                observed(len(chunk))
            result.update(chunk)
    return result.hexdigest()


def _stamp(path):
    value = Path(path).stat()
    if not stat.S_ISREG(value.st_mode):
        raise ValueError('verification requires a regular file: ' + str(path))
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _typed(value):
    """Lossless keys for the actual metadata argument domain, including order."""
    kind = type(value)
    if value is None:
        return ['none']
    if kind in (str, bool, int):
        return [kind.__name__, value]
    if kind is float and math.isfinite(value):
        return ['float', value.hex()]
    if isinstance(value, Path):
        return ['path', str(value)]
    if kind in (tuple, list):
        return [kind.__name__, [_typed(item) for item in value]]
    if kind is dict:
        return ['dict', [[_typed(key), _typed(item)] for key, item in value.items()]]
    raise ValueError('unsupported metadata argument/result type: ' + kind.__name__)


def _identity(value):
    return hashlib.sha256(json.dumps(_typed(value), separators=(',', ':'),
                                    ensure_ascii=True).encode()).hexdigest()


class ByteVerificationPass:
    """Immutable file bindings, with first full hash and stat checks on each use."""
    def __init__(self, sources=()):
        self.files = {}
        self.aliases = {}
        self._aliases_by_target = {}
        self.poisoned = None
        self.closed = False
        self.full_checks = 0
        self.hash_counts = {'first_hashes': 0, 'hash_calls': 0, 'digest_hits': 0,
                            'track_hits': 0, 'bytes_read': 0, 'hash_seconds': 0.0}
        self.thread = threading.get_ident()
        self._busy = False
        self._stack = []
        self.sources = set()
        for path in (Path(__file__), Path(sys.executable), *sources):
            self.sources.add(self.track(path))

    def fail(self, message):
        self.poisoned = self.poisoned or message
        raise ValueError(self.poisoned)

    def healthy(self):
        if self.closed or self.poisoned:
            raise ValueError(self.poisoned or 'verification operation is closed')
        if threading.get_ident() != self.thread:
            self.fail('verification operation requires its original thread')

    def _poison(self, error):
        self.poisoned = self.poisoned or ('verification failed: ' + repr(error))

    def _hash(self, path):
        def observed(size):
            self.hash_counts['bytes_read'] += size
        self.hash_counts['hash_calls'] += 1
        started = time.perf_counter()
        try:
            return _full_digest(path, observed)
        finally:
            self.hash_counts['hash_seconds'] += time.perf_counter() - started

    def track(self, source):
        self.healthy()
        try:
            if isinstance(source, int):
                self.fail('untracked descriptor in verification')
            alias = os.path.abspath(os.fsdecode(source))
            path = str(Path(alias).resolve(strict=True))
            if alias in self.aliases and self.aliases[alias] != path:
                self.fail('verification path target changed: ' + alias)
            before = _stamp(path)
            previous = self.files.get(path)
            if previous is None:
                self.hash_counts['first_hashes'] += 1
                old_busy, self._busy = self._busy, True
                try:
                    sha = self._hash(path)
                finally:
                    self._busy = old_busy
                if _stamp(path) != before or str(Path(alias).resolve(strict=True)) != path:
                    self.fail('verification file changed during first hash: ' + alias)
                self.files[path] = {'stamp': before, 'sha256': sha}
            elif previous['stamp'] != before:
                self.fail('verification file identity changed: ' + path)
            else:
                self.hash_counts['track_hits'] += 1
            self.aliases[alias] = path
            self._aliases_by_target.setdefault(path, set()).add(alias)
            for dependencies in self._stack:
                dependencies.add(path)
            return path
        except BaseException as error:
            self._poison(error)
            raise

    def digest(self, source):
        """Explicit callback: immutable dependency bytes only, never native output."""
        first_hashes = self.hash_counts['first_hashes']
        path = self.track(source)
        if self.hash_counts['first_hashes'] == first_hashes:
            self.hash_counts['digest_hits'] += 1
        # A new canonical spelling must not conceal a previously retargeted alias.
        self.check_stats({path})
        return self.files[path]['sha256']

    def check(self, bound):
        self.healthy()
        try:
            if (not isinstance(bound, dict) or set(bound) != {'path', 'sha256'}
                    or not isinstance(bound['path'], str)
                    or not isinstance(bound['sha256'], str)):
                self.fail('invalid verification binding')
            path = Path(bound['path']).resolve(strict=True)
            if str(path) != bound['path'] or self.digest(path) != bound['sha256']:
                self.fail('verification binding changed')
            return path
        except BaseException as error:
            self._poison(error)
            raise

    def check_stats(self, dependencies=None):
        self.healthy()
        try:
            keys = set(self.files) if dependencies is None else set(dependencies)
            for path in keys:
                for alias in self._aliases_by_target[path]:
                    if str(Path(alias).resolve(strict=True)) != path:
                        self.fail('verification path target changed: ' + alias)
            for path in keys:
                if _stamp(path) != self.files[path]['stamp']:
                    self.fail('verification file identity changed: ' + path)
        except BaseException as error:
            self._poison(error)
            raise

    def check_bytes(self):
        self.healthy()
        try:
            if self._stack:
                self.fail('full byte check inside metadata validation')
            self.check_stats()
            for path, value in self.files.items():
                if self._hash(path) != value['sha256']:
                    self.fail('verification bytes changed: ' + path)
            self.check_stats()
            self.full_checks += 1
        except BaseException as error:
            self._poison(error)
            raise

    def before_producer(self):
        """Mandatory full-byte boundary immediately before math/native work."""
        self.check_bytes()

    def report(self):
        return {'kind': 'routing_metadata_verification_v1', 'poisoned': self.poisoned,
                'full_byte_checks': self.full_checks, 'hashing': dict(self.hash_counts),
                'path_aliases': dict(sorted(self.aliases.items())),
                'closure': [{'path': path, 'sha256': value['sha256'],
                             'stamp': list(value['stamp'])}
                            for path, value in sorted(self.files.items())]}

    def finish(self):
        try:
            self.check_bytes()
            return self.report()
        finally:
            self.closed = True

    def __enter__(self):
        self.healthy()
        return self

    def __exit__(self, kind, error, traceback):
        if error is not None:
            self._poison(error)
            self.closed = True
        else:
            self.finish()
        return False


def _audit(event, args):
    operation = _ACTIVE.get()
    if operation is not None and not operation._busy:
        operation._audit(event, args)


sys.addaudithook(_audit)


class MetadataValidation(ByteVerificationPass):
    """Scoped exact memoization of four original metadata functions only."""
    def __init__(self, sources=()):
        super().__init__(sources)
        self.cache = {}
        self.counts = {}
        self._originals = []
        self._digest_wrappers = {}
        self._restore_aliases = {}

    def _audit(self, event, args):
        self.healthy()
        denied = {'subprocess.Popen', 'os.system', 'os.posix_spawn', 'os.fork',
                  'os.forkpty', 'mmap.__new__', 'os.remove', 'os.rename', 'os.mkdir',
                  'os.rmdir', 'os.chmod', 'os.chown', 'os.utime', 'os.truncate',
                  'os.link', 'os.symlink', 'shutil.copyfile'}
        if event in denied or event.startswith('os.exec'):
            self.fail('unsupported metadata operation: ' + event)
        if event != 'open':
            return
        path, mode, flags = args
        if isinstance(path, int) or mode is None:
            self.fail('unsupported descriptor in metadata validation')
        if flags & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND):
            self.fail('metadata validator attempted to write: ' + str(path))
        self.track(path)

    def _wrap(self, name, function):
        @functools.wraps(function)
        def wrapped(*args, **kwargs):
            self.healthy()
            try:
                key = (name, _identity((args, kwargs)))
                count = self.counts.setdefault(name, {'hits': 0, 'misses': 0, 'uncached_executes': 0})
                if key in self.cache:
                    value, dependencies = self.cache[key]
                    self.check_stats(dependencies)
                    for parent in self._stack:
                        parent.update(dependencies)
                    count['hits'] += 1
                    return copy.deepcopy(value)
                dependencies = set(self.sources)
                count['misses'] += 1
                self._stack.append(dependencies)
                token = _ACTIVE.set(self)
                try:
                    count['uncached_executes'] += 1
                    value = function(*args, **kwargs)
                    if key[1] != _identity((args, kwargs)):
                        self.fail('metadata validator mutated its arguments: ' + name)
                    self.check_stats(dependencies)
                    saved = copy.deepcopy(value)
                    # Reject unsupported result objects before they can be reused.
                    _identity(saved)
                    self.cache[key] = saved, set(dependencies)
                    return copy.deepcopy(saved)
                finally:
                    _ACTIVE.reset(token)
                    self._stack.pop()
                    for parent in self._stack:
                        parent.update(dependencies)
            except BaseException as error:
                self._poison(error)
                raise
        return wrapped

    def _digest_wrapper(self, original):
        @functools.wraps(original)
        def wrapped(*args, **kwargs):
            if _ACTIVE.get() is self:
                return self.digest(*args, **kwargs)
            return original(*args, **kwargs)
        return wrapped

    def install(self, blocks=None, deployment=None):
        """Install after any reviewed dispatch wrappers; only four slots qualify."""
        global _INSTALLED
        self.healthy()
        if _INSTALLED is not None or self._originals:
            self.fail('another metadata operation is installed')
        try:
            for name in _PRELOAD:
                importlib.import_module(name)
            modules = (blocks or importlib.import_module('routing_blocks'),
                       deployment or importlib.import_module('routing_block_deployment'))
            originals = [getattr(importlib.import_module(module), name)
                         for module, name in _DIGESTS]
            for module, (_, names) in zip(modules, _METADATA):
                self.sources.add(self.track(module.__file__))
                for name in names:
                    original = getattr(module, name)
                    self.sources.add(self.track(original.__code__.co_filename))
                    self._originals.append((module, name, original))
                    wrapped = self._wrap(module.__name__ + '.' + name, original)
                    self._restore_aliases[wrapped] = original
                    setattr(module, name, wrapped)
            for original in originals:
                self.sources.add(self.track(original.__code__.co_filename))
                wrapped = self._digest_wrapper(original)
                self._digest_wrappers[original] = wrapped
                self._restore_aliases[wrapped] = original
            roots = (Path(__file__).resolve().parent, Path(__file__).resolve().parent.parent / 'scripts')
            for module in tuple(sys.modules.values()):
                source = getattr(module, '__file__', None) if isinstance(module, types.ModuleType) else None
                if source and any(Path(source).resolve().is_relative_to(root) for root in roots):
                    self.sources.add(self.track(source))
            # Match identities, never names: unrelated digest functions stay intact.
            for module in tuple(sys.modules.values()):
                if isinstance(module, types.ModuleType):
                    for name, value in tuple(vars(module).items()):
                        for original, wrapped in self._digest_wrappers.items():
                            if value is original:
                                self._originals.append((module, name, original))
                                setattr(module, name, wrapped)
            _INSTALLED = self
        except BaseException as error:
            self._poison(error)
            self.restore()
            raise
        return self

    def restore(self):
        global _INSTALLED
        # Imports performed during an operation may have acquired a wrapped alias.
        reverse = self._restore_aliases
        for module in tuple(sys.modules.values()):
            if isinstance(module, types.ModuleType):
                for name, value in tuple(vars(module).items()):
                    if isinstance(value, types.FunctionType) and value in reverse:
                        setattr(module, name, reverse[value])
        for module, name, original in reversed(self._originals):
            setattr(module, name, original)
        self._originals.clear()
        self._digest_wrappers.clear()
        self._restore_aliases.clear()
        if _INSTALLED is self:
            _INSTALLED = None

    def report(self):
        result = super().report()
        result.update(counts=copy.deepcopy(self.counts), entries=len(self.cache),
                      cached_metadata=[{'function': name, 'arguments_sha256': key,
                                        'value_sha256': _identity(value),
                                        'dependencies': sorted(dependencies)}
                                       for (name, key), (value, dependencies)
                                       in sorted(self.cache.items())])
        return result

    def finish(self):
        try:
            return super().finish()
        finally:
            self.restore()

    def __enter__(self):
        self.healthy()
        if not self._originals:
            self.install()
        return self

    def __exit__(self, kind, error, traceback):
        try:
            return super().__exit__(kind, error, traceback)
        finally:
            self.restore()
