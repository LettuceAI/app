#!/usr/bin/env python3
import argparse
import json
import os
from pathlib import Path
import tempfile
from datetime import datetime, timezone


def namespace():
    return os.readlink('/proc/self/ns/pid')


def process_start(pid):
    try:
        return Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()[19]
    except (OSError, IndexError):
        return None


def main():
    parser = argparse.ArgumentParser(description='LettuceAI container application status')
    parser.add_argument('action', choices=('write', 'show'), nargs='?', default='show')
    parser.add_argument('phase', nargs='?')
    parser.add_argument('--pid', type=int)
    parser.add_argument('--provider')
    parser.add_argument('--reason')
    parser.add_argument('--exit-code', type=int)
    args = parser.parse_args()
    root = Path(os.environ.get('XDG_STATE_HOME', str(Path.home() / '.local/state')))
    path = root / 'lettuceai-container/status.json'
    if args.action == 'write':
        if not args.phase:
            parser.error('write requires a phase')
        path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        state = {
            'phase': args.phase,
            'updated_at': datetime.now(timezone.utc).isoformat(),
            'namespace': namespace(),
            'container_start': process_start(1),
        }
        for key in ('pid', 'provider', 'reason', 'exit_code'):
            if (value := getattr(args, key)) is not None:
                state[key] = value
        if args.pid is not None:
            state['process_start'] = process_start(args.pid)
        with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as stream:
            temporary = Path(stream.name)
            try:
                json.dump(state, stream)
                stream.flush()
                os.fsync(stream.fileno())
                os.replace(temporary, path)
            finally:
                temporary.unlink(missing_ok=True)
        return
    try:
        state = json.loads(path.read_text())
    except FileNotFoundError:
        state = {'phase': 'not_started'}
    if (state.get('namespace') not in (None, namespace())
            or state.get('container_start') not in (None, process_start(1))):
        state = {'phase': 'not_started', 'previous_phase': state.get('phase')}
    if state.get('phase') == 'running':
        alive = state.get('process_start') is not None and process_start(state.get('pid')) == state.get('process_start')
        state['running'] = alive
        if not alive:
            state['phase'] = 'stopped'
    metadata = Path('/opt/lettuceai/container-build.json')
    state['image'] = json.loads(metadata.read_text()) if metadata.is_file() else {}
    print(json.dumps(state, indent=2))


if __name__ == '__main__':
    main()
