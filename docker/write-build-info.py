#!/usr/bin/env python3
import hashlib
import json
from pathlib import Path
import sys

mode, version, revision, accelerator = sys.argv[1:]
root = Path('/opt/lettuceai')
if version in ('auto', 'unknown', 'source', 'prebuilt'):
    version_file = root / 'package-version'
    version = version_file.read_text().strip() if version_file.is_file() else 'unknown'
with (root / 'bin/lettuceai').open('rb') as binary:
    digest = hashlib.file_digest(binary, 'sha256').hexdigest()
(root / 'container-build.json').write_text(json.dumps({
    'mode': mode, 'version': version, 'revision': revision,
    'accelerator': accelerator, 'binary_sha256': digest,
    'installed_keyring_provider': (root / 'keyring-provider').read_text().strip(),
}))
