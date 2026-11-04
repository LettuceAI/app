import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class ManagementTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        scripts = self.root / 'docker'
        scripts.mkdir()
        shutil.copy(ROOT / 'manage.sh', scripts / 'manage.sh')
        (scripts / '.env').write_text('VNC_PW=test-only\n')
        self.log = self.root / 'commands.jsonl'
        executable = self.root / 'docker-cli'
        executable.mkdir()
        (executable / 'docker').write_text('''#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
with open(os.environ['DOCKER_TEST_LOG'], 'a') as log:
    log.write(json.dumps(args) + '\\n')
if args[:2] == ['image', 'inspect']:
    print('sha256:previous' if 'rollback-' in args[-1] else 'sha256:tagged-but-not-running')
elif args[0] == 'compose':
    if 'build' in args and os.environ.get('FAIL_BUILD') == '1': sys.exit(7)
    if 'up' in args and os.environ.get('FAIL_UP') == '1': sys.exit(8)
    if 'ps' in args and '-q' in args: print('test-container')
elif args[0] == 'inspect': print('sha256:current' if args[2] == '{{.Image}}' else 'Image ID: sha256:current')
elif args[0] == 'exec': print('{"phase":"waiting_for_keyring"}')
''')
        (executable / 'docker').chmod(0o755)
        self.env = {
            **os.environ, 'PATH': str(executable) + os.pathsep + os.environ['PATH'],
            'DOCKER_TEST_LOG': str(self.log),
        }

    def run_management(self, action, **extra):
        return subprocess.run(
            ['bash', str(self.root / 'docker/manage.sh'), action, 'cuda'],
            env={**self.env, **extra}, capture_output=True, text=True,
        )

    def commands(self):
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_failed_build_keeps_running_container_and_previous_image(self):
        result = self.run_management('update', FAIL_BUILD='1')
        self.assertNotEqual(result.returncode, 0)
        commands = self.commands()
        self.assertTrue(any('build' in command for command in commands))
        self.assertFalse(any('up' in command or 'down' in command or command[0] == 'tag' for command in commands))

    def test_update_builds_before_replacing_and_never_removes_volumes(self):
        result = self.run_management('update')
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        build = next(i for i, command in enumerate(commands) if 'build' in command)
        backup = next(i for i, command in enumerate(commands) if command[:2] == ['tag', 'sha256:current'])
        recreate = next(i for i, command in enumerate(commands) if 'up' in command)
        self.assertLess(build, backup)
        self.assertLess(backup, recreate)
        self.assertIn('--no-build', commands[recreate])
        self.assertIn('--force-recreate', commands[recreate])
        self.assertFalse(any('down' in command or 'volume' in command or 'prune' in command for command in commands))

    def test_failed_rollback_restores_the_current_image_tag(self):
        result = self.run_management('rollback', FAIL_UP='1')
        self.assertNotEqual(result.returncode, 0)
        tags = [command for command in self.commands() if command[0] == 'tag']
        self.assertEqual(tags, [
            ['tag', 'sha256:previous', 'lettuceai:kasm-cuda'],
            ['tag', 'sha256:current', 'lettuceai:kasm-cuda'],
        ])

    def test_status_preserves_keyring_and_gpu_overrides_without_mutation(self):
        result = self.run_management('status', LETTUCE_KEYRING_SECRET='/test/password', LETTUCE_GPU_CONFIG='/test/gpus.yaml')
        self.assertEqual(result.returncode, 0, result.stderr)
        commands = self.commands()
        for command in commands:
            if command[0] == 'compose':
                self.assertIn(str(self.root / 'docker/compose.keyring.yaml'), command)
                self.assertIn('/test/gpus.yaml', command)
        self.assertIn('waiting_for_keyring', result.stdout)
        self.assertFalse(any('build' in command or 'up' in command or command[0] == 'tag' for command in commands))


class StateTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.env = {**os.environ, 'XDG_STATE_HOME': self.directory.name}

    def status(self, *arguments):
        result = subprocess.run(['python3', str(ROOT / 'container-state.py'), *arguments], env=self.env, capture_output=True, text=True, check=True)
        return json.loads(result.stdout) if result.stdout else None

    def test_running_pid_is_checked_and_stopped_process_is_not_reported_running(self):
        process = subprocess.Popen(['sleep', '30'])
        try:
            self.status('write', 'running', '--pid', str(process.pid))
            self.assertTrue(self.status()['running'])
        finally:
            process.terminate()
            process.wait()
        self.assertEqual(self.status()['phase'], 'stopped')

    def test_record_from_previous_container_is_not_current(self):
        self.status('write', 'waiting_for_keyring')
        path = Path(self.directory.name) / 'lettuceai-container/status.json'
        state = json.loads(path.read_text())
        state['namespace'] = 'pid:[previous-container]'
        path.write_text(json.dumps(state))
        self.assertEqual(self.status()['phase'], 'not_started')

    def test_reused_namespace_does_not_make_previous_container_current(self):
        self.status('write', 'waiting_for_keyring')
        path = Path(self.directory.name) / 'lettuceai-container/status.json'
        state = json.loads(path.read_text())
        state['container_start'] = 'previous-container-start'
        path.write_text(json.dumps(state))
        self.assertEqual(self.status()['phase'], 'not_started')


if __name__ == '__main__':
    unittest.main()
