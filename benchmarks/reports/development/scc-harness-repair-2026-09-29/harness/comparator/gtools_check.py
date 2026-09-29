"""Validate the observed gtools log and all exact sample-publication checks."""
import csv
import re


def fixture_passed(folder, process_rc):
    log, checks = folder / 'gtools.log', folder / 'checks.tsv'
    if process_rc != 0 or not log.is_file() or not checks.is_file():
        return False
    text = log.read_text()
    if text.splitlines().count('GTOOLS_SAMPLE_FIXTURE_COMPLETE') != 1 or re.search(r'^r\([1-9][0-9]*\);', text, re.M):
        return False
    with checks.open() as stream:
        rows = list(csv.DictReader(stream, delimiter='\t'))
    if [r.get('case') for r in rows] != ['1','2','3','4','empty_sample']:
        return False
    for row, count in zip(rows[:4], ['600','585','584','585']):
        if row.get('N') != count or row.get('sample_exact') != '1' or row.get('b_V_unchanged') != '1':
            return False
    return rows[-1].get('sample_exact') == 'no_e_b' and rows[-1].get('b_V_unchanged') == 'not_ranked'
