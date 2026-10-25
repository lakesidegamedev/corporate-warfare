"""Prevent false completion and malformed dependency graphs in the handoff ledger."""
import copy
import unittest
from pathlib import Path
from plan_check import validate, validate_execution


class LedgerValidation(unittest.TestCase):
    def setUp(self):
        self.doc = {'schema': 1, 'requirements': [
            {'id': 'R01', 'title': 'Example', 'acceptance': 'A verified outcome',
             'depends_on': [], 'gate': 'automated', 'status': 'pending', 'evidence': []}
        ]}

    def check(self, doc):
        return validate(doc, Path(__file__).resolve().parents[1])

    def test_pending_is_structurally_valid(self):
        self.assertEqual(self.check(self.doc), [])

    def test_done_requires_evidence(self):
        self.doc['requirements'][0]['status'] = 'done'
        self.assertTrue(any('done without evidence' in e for e in self.check(self.doc)))

    def test_unknown_dependency_fails(self):
        self.doc['requirements'][0]['depends_on'] = ['R99']
        self.assertTrue(any('missing dependency' in e for e in self.check(self.doc)))

    def test_cycle_fails(self):
        self.doc['requirements'][0]['depends_on'] = ['R01']
        self.assertTrue(any('cycle' in e for e in self.check(self.doc)))

    def test_duplicate_id_fails(self):
        self.doc['requirements'].append(copy.deepcopy(self.doc['requirements'][0]))
        self.assertTrue(any('duplicate' in e for e in self.check(self.doc)))

    def test_incomplete_dependency_prevents_done(self):
        other = copy.deepcopy(self.doc['requirements'][0])
        other.update(id='R02', status='done', depends_on=['R01'])
        self.doc['requirements'].append(other)
        self.assertTrue(any('is not done' in e for e in self.check(self.doc)))

    def test_evidence_cannot_escape_repo(self):
        self.doc['requirements'][0].update(status='done', evidence=['/etc/hosts'])
        self.assertTrue(any('out-of-repo' in e for e in self.check(self.doc)))


class PacketValidation(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[1]
        self.doc = {'schema': 1, 'tasks': [{
            'id': 'P00', 'status': 'pending', 'requirements': ['R01'],
            'depends_on': [], 'spec': 'docs/rules/INTERFACE.md',
            'checklist': [{'text': 'Verify the outcome', 'done': False}], 'evidence': []
        }]}

    def test_packet_cannot_complete_from_status_alone(self):
        self.doc['tasks'][0]['status'] = 'done'
        errors = validate_execution(self.doc, {'R01'}, self.root)
        self.assertTrue(any('completion needs' in e for e in errors))

    def test_packet_cannot_reference_unknown_requirement(self):
        errors = validate_execution(self.doc, set(), self.root)
        self.assertTrue(any('unknown requirement' in e for e in errors))

    def test_packet_cycle_fails(self):
        self.doc['tasks'][0]['depends_on'] = ['P00']
        errors = validate_execution(self.doc, {'R01'}, self.root)
        self.assertTrue(any('cycle' in e for e in errors))


if __name__ == '__main__':
    unittest.main()
