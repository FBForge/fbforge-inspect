"""Download the immutable, independently published 0.1.0 upgrade baseline by pinned hash."""
import hashlib
import os
from pathlib import Path
import tempfile
import urllib.request

URL = 'https://github.com/FBForge/fbforge-inspect/releases/download/0.1.0/FBForge-Inspect-0.1.0-windows-x64-setup.exe'
SHA256 = '7709169ee04a6e557dd6a2e2bd79457299019ef1a9a9a4539b877b9f0857a664'
LIMIT = 64 * 1024 * 1024


def main():
    root = Path(tempfile.mkdtemp(prefix='inspect-upgrade-baseline-', dir=os.environ['RUNNER_TEMP']))
    target = root / 'inspect-0.1.0-setup.exe'
    digest = hashlib.sha256()
    count = 0
    with urllib.request.urlopen(URL, timeout=60) as response, target.open('xb') as output:
        while block := response.read(1024 * 1024):
            count += len(block)
            if count > LIMIT:
                raise RuntimeError('upgrade_baseline_size_limit')
            digest.update(block)
            output.write(block)
    if digest.hexdigest() != SHA256:
        target.unlink()
        raise RuntimeError('upgrade_baseline_hash_invalid')
    with Path(os.environ['GITHUB_ENV']).open('a', encoding='utf-8') as environment:
        environment.write('INSPECT_UPGRADE_BASELINE=' + str(target) + '\n')
    print('Upgrade baseline hash verified. Authenticode and actual installation are checked on Windows.')


if __name__ == '__main__':
    main()
