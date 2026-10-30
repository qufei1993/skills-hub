#!/usr/bin/env python3
"""Capture explicit project documents and compare adoption baselines. Never edits documents."""
import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import tempfile

LIMIT = 1024 * 1024
STATE = '.dsh-engineering/adoption.json'


def path_at(root, name, document=True):
    parts = PurePosixPath(name)
    if not name or parts.is_absolute() or '..' in parts.parts or '\\' in name or parts.as_posix() != name:
        raise ValueError(f'unsafe path: {name}')
    if document and (parts.suffix.lower() not in ('.md', '.rst', '.txt', '.adoc') or
                     any(p.startswith('.') and p != '.github' for p in parts.parts) or
                     any(p.lower() in ('credentials', 'secrets') for p in parts.parts)):
        raise ValueError(f'not an allowed document path: {name}')
    target = root
    for part in parts.parts:
        target /= part
        if target.is_symlink():
            raise ValueError(f'symlink is not allowed: {name}')
    return target


def digest(content):
    return hashlib.sha256(content.encode()).hexdigest() if content is not None else None


def entry(content):
    return {'content': content, 'sha256': digest(content)}


def read_doc(root, name):
    path = path_at(root, name)
    if not path.exists():
        return entry(None)
    if not path.is_file() or path.stat().st_size > LIMIT:
        raise ValueError(f'not a regular document under 1 MiB: {name}')
    # Preserve CRLF as well as final newline; hashes describe original bytes for UTF-8.
    return entry(path.read_bytes().decode('utf-8'))


def checked_entry(value, label):
    if not isinstance(value, dict) or set(value) != {'content', 'sha256'}:
        raise ValueError(f'invalid content entry: {label}')
    content = value['content']
    if content is not None and (not isinstance(content, str) or len(content.encode()) > LIMIT):
        raise ValueError(f'invalid document content: {label}')
    if value['sha256'] != digest(content):
        raise ValueError(f'hash mismatch: {label}')
    return value


def read_json(path):
    # Inputs are explicit, but do not follow links to other files.
    path = Path(path).absolute()
    if path.is_symlink():
        raise ValueError(f'symlink input: {path}')
    # Explicit external inputs may live under OS aliases such as macOS /var.
    # Project-owned paths were already checked component by component by path_at.
    path = path.parent.resolve(strict=True) / path.name
    if path.stat().st_size > 20 * LIMIT:
        raise ValueError(f'JSON input too large: {path}')
    return json.loads(path.read_text())


def load_state(root):
    path = path_at(root, STATE, False)
    if not path.exists():
        raise ValueError('no adoption baseline; inspect project and create a new adoption snapshot explicitly')
    state = read_json(path)
    if state.get('schema') != 1 or not isinstance(state.get('files'), dict) or not state['files']:
        raise ValueError('invalid adoption baseline schema')
    for name, data in state['files'].items():
        path_at(root, name)
        if not isinstance(data, dict) or set(data) != {'original', 'baseline'}:
            raise ValueError(f'invalid baseline: {name}')
        for kind in ('original', 'baseline'):
            checked_entry(data[kind], f'{name}/{kind}')
    return state


def json_bytes(data):
    return (json.dumps(data, indent=2, sort_keys=True, ensure_ascii=False) + '\n').encode()


def snapshot(root, paths, output):
    data = {'schema': 1, 'project': str(root), 'files': {name: read_doc(root, name) for name in paths}}
    output = Path(output).absolute()
    if output.is_symlink():
        raise ValueError(f'symlink output: {output}')
    output = output.parent.resolve(strict=True) / output.name
    # Do not overwrite a snapshot or any existing document.
    with output.open('xb') as stream:
        stream.write(json_bytes(data))
    return {'snapshot': str(output), 'documents': len(data['files'])}


def record(root, snapshot_path, version, upstream):
    if not re.fullmatch(r'[0-9a-f]{40}', upstream) or not version.strip():
        raise ValueError('version and full upstream SHA required')
    snap = read_json(snapshot_path)
    if snap.get('schema') != 1 or snap.get('project') != str(root) or not isinstance(snap.get('files'), dict) or not snap['files']:
        raise ValueError('snapshot schema or project mismatch')
    for name, data in snap['files'].items():
        path_at(root, name)
        checked_entry(data, name)
    target = path_at(root, STATE, False)
    state = load_state(root) if target.exists() else {'schema': 1, 'files': {}}
    state.update(version=version, upstream=upstream)
    for name, original in snap['files'].items():
        first_original = state['files'].get(name, {}).get('original', original)
        state['files'][name] = {'original': first_original, 'baseline': read_doc(root, name)}
    payload = json_bytes(state)
    if target.exists() and target.read_bytes() == payload:
        return {'changed': False, 'state': STATE}
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.adoption-', dir=target.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(payload)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, target)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)
    return {'changed': True, 'state': STATE}


def compare(root, proposal_path=None):
    state = load_state(root)
    proposals = read_json(proposal_path)['files'] if proposal_path else {}
    if not isinstance(proposals, dict) or set(proposals) - state['files'].keys():
        raise ValueError('proposal contains paths without an adoption baseline')
    result = {'version': state['version'], 'upstream': state['upstream'], 'files': {}}
    for name, data in state['files'].items():
        current = read_doc(root, name)
        baseline = data['baseline']
        result['files'][name] = item = {**data, 'current': current, 'local_changes': current != baseline}
        item['diff'] = ''.join(difflib.unified_diff((baseline['content'] or '').splitlines(True),
                                                   (current['content'] or '').splitlines(True), fromfile=f'baseline/{name}', tofile=f'current/{name}'))
        if name in proposals:
            content = proposals[name]
            if content is not None and not isinstance(content, str):
                raise ValueError(f'invalid proposed content: {name}')
            proposed = checked_entry(entry(content), name)
            item['proposal'] = proposed
            item['conflict'] = current != baseline and proposed != baseline and proposed != current
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    for command in ('snapshot', 'record', 'compare'):
        sub = commands.add_parser(command)
        sub.add_argument('--project', type=Path, required=True)
        if command == 'snapshot':
            sub.add_argument('--paths', nargs='+', required=True)
            sub.add_argument('--output', type=Path, required=True)
        elif command == 'record':
            sub.add_argument('--snapshot', type=Path, required=True)
            sub.add_argument('--version', required=True)
            sub.add_argument('--upstream', required=True)
        else:
            sub.add_argument('--proposal', type=Path)
    args = parser.parse_args()
    try:
        root = args.project.resolve(strict=True)
        if not root.is_dir():
            raise ValueError('project must be a directory')
        if args.command == 'snapshot':
            result = snapshot(root, args.paths, args.output)
        elif args.command == 'record':
            result = record(root, args.snapshot, args.version, args.upstream)
        else:
            result = compare(root, args.proposal)
        print(json.dumps(result, indent=2, ensure_ascii=False))
    except (ValueError, OSError, KeyError, TypeError, AttributeError) as error:
        parser.exit(1, f'error: {error}\n')


if __name__ == '__main__':
    main()
