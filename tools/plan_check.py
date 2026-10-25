#!/usr/bin/env python3
"""Validate planning structure. Passing is not proof of gameplay completion."""
import argparse
import json
from pathlib import Path
import re
import sys
from datetime import date

ROOT = Path(__file__).resolve().parents[1]
LEDGER = ROOT / 'docs/planning/requirements.json'
STATES = {'pending', 'active', 'blocked', 'done'}
GATES = {'automated', 'cross-platform', 'human', 'device', 'apple'}


def validate(document, root):
    errors = []
    if document.get('schema') != 1:
        errors.append('unsupported ledger schema')
    rows = document.get('requirements', [])
    if not rows:
        return errors + ['empty requirements ledger']
    by_id = {}
    for row in rows:
        ident = row.get('id', '')
        if not re.fullmatch(r'R\d{2,}', ident) or ident in by_id:
            errors.append(f'invalid or duplicate requirement ID: {ident}')
        by_id[ident] = row
        if row.get('status') not in STATES:
            errors.append(f'{ident}: unknown status')
        if row.get('gate') not in GATES:
            errors.append(f'{ident}: unknown gate')
        for field in ('title', 'acceptance'):
            if not isinstance(row.get(field), str) or not row[field].strip():
                errors.append(f'{ident}: missing {field}')
        if not isinstance(row.get('depends_on'), list):
            errors.append(f'{ident}: dependencies must be a list')
        if not isinstance(row.get('evidence'), list):
            errors.append(f'{ident}: evidence must be a list')
    visiting, visited = set(), set()

    def walk(ident):
        if ident in visiting:
            errors.append(f'dependency cycle at {ident}')
            return
        if ident in visited:
            return
        visiting.add(ident)
        for dependency in by_id[ident].get('depends_on', []):
            if dependency not in by_id:
                errors.append(f'{ident}: missing dependency {dependency}')
            else:
                walk(dependency)
        visiting.remove(ident)
        visited.add(ident)

    for ident, row in by_id.items():
        walk(ident)
        if row.get('status') != 'done':
            continue
        for dependency in row.get('depends_on', []):
            if by_id.get(dependency, {}).get('status') != 'done':
                errors.append(f'{ident}: dependency {dependency} is not done')
        evidence = row.get('evidence', [])
        if not evidence:
            errors.append(f'{ident}: done without evidence')
        for relative in evidence:
            if not isinstance(relative, str):
                errors.append(f'{ident}: invalid evidence path')
                continue
            path = (root / relative).resolve()
            if not path.is_relative_to(root.resolve()) or not path.is_file():
                errors.append(f'{ident}: missing or out-of-repo evidence {relative}')
                continue
            try:
                record = json.loads(path.read_text())
            except (ValueError, OSError) as exc:
                errors.append(f'{ident}: invalid evidence JSON: {exc}')
                continue
            if record.get('requirement') != ident or record.get('result') != 'pass':
                errors.append(f'{ident}: evidence must name requirement and pass')
            if not re.fullmatch(r'[0-9a-f]{40}', record.get('commit', '')):
                errors.append(f'{ident}: evidence needs full tested commit')
            for field in ('procedure', 'artifacts', 'limitations', 'verified_at'):
                if not record.get(field):
                    errors.append(f'{ident}: evidence missing {field}')
            try:
                verified = date.fromisoformat(record.get('verified_at', ''))
                if verified > date.today():
                    errors.append(f'{ident}: future-dated evidence')
            except ValueError:
                errors.append(f'{ident}: verified_at must be an ISO date')
            if row['gate'] != 'automated' and not record.get('verifier'):
                errors.append(f'{ident}: platform/human evidence needs named verifier')
            if row['gate'] == 'device' and not all(record.get(k) for k in ('device', 'os', 'build')):
                errors.append(f'{ident}: device evidence needs device, os and build')
            if row['gate'] == 'apple' and not all(record.get(k) for k in ('build', 'apple_status')):
                errors.append(f'{ident}: Apple evidence needs build and apple_status')
    return errors


def validate_execution(document, requirements, root):
    errors = []
    tasks = document.get('tasks', [])
    if document.get('schema') != 1 or not tasks:
        return ['invalid or empty execution ledger']
    by_id = {}
    for task in tasks:
        ident = task.get('id', '')
        if not re.fullmatch(r'P\d{2,}', ident) or ident in by_id:
            errors.append(f'invalid or duplicate packet ID: {ident}')
        by_id[ident] = task
        if task.get('status') not in STATES:
            errors.append(f'{ident}: unknown packet status')
        for required in task.get('requirements', []):
            if required not in requirements:
                errors.append(f'{ident}: unknown requirement {required}')
        spec = (root / task.get('spec', '')).resolve()
        if not spec.is_relative_to(root.resolve()) or not spec.is_file():
            errors.append(f'{ident}: missing/out-of-repo rule spec')
        checklist = task.get('checklist', [])
        if not checklist or any(not x.get('text') or type(x.get('done')) is not bool for x in checklist):
            errors.append(f'{ident}: invalid checklist')
        if task.get('status') == 'done':
            if not all(x.get('done') for x in checklist) or not task.get('evidence'):
                errors.append(f'{ident}: completion needs checklist and evidence')
            for reference in task.get('evidence', []):
                path = (root / reference).resolve()
                if not path.is_relative_to(root.resolve()) or not path.is_file():
                    errors.append(f'{ident}: missing/out-of-repo packet evidence')
    visiting, visited = set(), set()

    def walk(ident):
        if ident in visiting:
            errors.append(f'packet dependency cycle at {ident}')
            return
        if ident in visited:
            return
        visiting.add(ident)
        for dep in by_id[ident].get('depends_on', []):
            if dep not in by_id:
                errors.append(f'{ident}: unknown packet dependency {dep}')
            else:
                if by_id[ident]['status'] == 'done' and by_id[dep]['status'] != 'done':
                    errors.append(f'{ident}: packet dependency {dep} is not done')
                walk(dep)
        visiting.remove(ident)
        visited.add(ident)

    for ident in by_id:
        walk(ident)
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--release', action='store_true', help='also require every release item done')
    args = parser.parse_args()
    try:
        document = json.loads(LEDGER.read_text())
        errors = validate(document, ROOT)
        execution = json.loads((ROOT / "docs/planning/execution.json").read_text())
        errors.extend(validate_execution(execution, {r["id"] for r in document["requirements"]}, ROOT))
    except (ValueError, OSError, TypeError, AttributeError) as exc:
        print(f'Invalid ledger: {exc}', file=sys.stderr)
        return 1
    rows = document['requirements']
    if args.release:
        errors.extend(f'{t["id"]}: packet incomplete' for t in execution["tasks"] if t["status"] != "done")
        errors.extend(f'{r["id"]}: release incomplete ({r["status"]})' for r in rows if r['status'] != 'done')
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    counts = {state: sum(r['status'] == state for r in rows) for state in sorted(STATES)}
    print(f'Ledger structure valid: {counts}. Evidence still requires review.')
    ready = [r['id'] for r in rows if r['status'] == 'pending' and all(
        next(x for x in rows if x['id'] == dep)['status'] == 'done' for dep in r['depends_on'])]
    print('Dependency-ready requirements: ' + ', '.join(ready))
    tasks = execution['tasks']
    print(f'Execution ledger valid: {len(tasks)} packets, {sum(len(t["checklist"]) for t in tasks)} checklist steps.')
    return 0


if __name__ == '__main__':
    sys.exit(main())
