import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from check_complexity import file_metrics, modules, growth_errors
from rust_source import _production_tokens


class ComplexityTests(unittest.TestCase):
    def test_module_graph_excludes_external_tests_and_keeps_ffi(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root/'lib.rs').write_text('#[cfg(test)] mod tests;\n#[cfg(feature="ffi")] mod ffi;\n#[path="working.rs"] pub(crate) mod renamed;\n')
            (root/'tests.rs').write_text('fn test_only() {}')
            (root/'ffi.rs').write_text('fn runtime_ffi() {}')
            (root/'working.rs').write_text('fn renamed_runtime() {}')
            self.assertEqual({p.name for p,_,_ in modules(root, root/'lib.rs')}, {'lib.rs','ffi.rs','working.rs'})

    def test_production_tokens_and_sensitive_conditions(self):
        source = '''fn active(n: usize, chunk_size: usize) {
// if weights {}
if n > 10 { work(); }
if chunk_size > 2 { work(); }
}
#[cfg(test)] mod tests { fn excluded() {} }
'''
        item = file_metrics(source, _production_tokens(source))
        self.assertEqual(item['production_lines'], 4)
        self.assertEqual(set(item['functions']), {'active#1'})
        self.assertEqual(len(item['dispatch_candidates']), 1)
        self.assertEqual(len(item['chunk_candidates']), 1)

    def test_growth_needs_exact_scoped_reason_and_new_size_violations_fail(self):
        old = {'metrics':{'core':{'production_lines':50}},'files':{}}
        new = {'metrics':{'core':{'production_lines':53}},'files':{}}
        reason = {'metric':'core/production_lines','before':50,'after':53,'reason':'Named independent correctness regression'}
        self.assertTrue(growth_errors(old,new,[]))
        self.assertEqual(growth_errors(old,new,[reason]), [])
        reason['after'] = 54
        self.assertTrue(growth_errors(old,new,[reason]))
        new['files']['new.rs'] = {'production_lines':20,'functions':{'too_big':{'lines':151}}}
        self.assertTrue(any('new size violation' in x for x in growth_errors(old,new,[])))

    def test_reductions_cannot_leave_permanent_growth_allowances(self):
        old = {'metrics':{'core':{'production_lines':50}},'files':{}}
        new = {'metrics':{'core':{'production_lines':40}},'files':{}}
        self.assertEqual(growth_errors(old,new,[]), [])
        self.assertTrue(growth_errors(old,new,[{'metric':'core/production_lines','before':50,'after':53,'reason':'stale'}]))

    def test_test_comparison_and_char_do_not_hide_production(self):
        source = "#[cfg(test)] const T: bool = 1 < 2;\n#[cfg(test)] const C: char = 'x';\nfn production() {}"
        values = [t.value for t in _production_tokens(source)]
        self.assertIn('production', values)
        self.assertNotIn('T', values)
        self.assertNotIn('C', values)

    def test_test_only_attributes_and_variants_do_not_hide_following_items(self):
        source = "#[derive(Debug)] #[cfg(test)] pub struct Hidden {}\nenum Route { #[cfg(test)] TestOnly, Public, }\npub enum Visible { Value }"
        values = [t.value for t in _production_tokens(source)]
        for absent in ['Hidden','TestOnly','Debug']:self.assertNotIn(absent,values)
        for present in ['Public','Visible','Value']:self.assertIn(present,values)

    def test_non_test_cfg_variants_and_literals_remain_counted(self):
        source = '#[cfg(any(test, feature="ffi"))] fn active() { let s = r#"{ hi }"#; /* braces { } */ }'
        item = file_metrics(source, _production_tokens(source))
        self.assertIn('active#1', item['functions'])


if __name__ == '__main__': unittest.main()
