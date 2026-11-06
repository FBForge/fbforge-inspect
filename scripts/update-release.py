"""Generate/validate updater metadata; never publish, execute or re-sign an installer."""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = 'https://github.com/FBForge/fbforge-inspect/releases/download/'


def metadata(installer, version, signature, notes=''):
    if not re.fullmatch(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)', version):
        raise ValueError('update_stable_version_required')
    if installer != f'FBForge-Inspect-{version}-windows-x64-setup.exe':
        raise ValueError('update_installer_mismatch')
    if not signature or len(signature) > 4096 or not re.fullmatch(r'[A-Za-z0-9+/=]+', signature):
        raise ValueError('update_signature_invalid')
    if len(notes.encode()) > 12000:
        raise ValueError('update_notes_too_large')
    return {'version': version, 'notes': notes, 'platforms': {'windows-x86_64': {
        'url': ROOT + version + '/' + installer.replace(' ', '.'), 'signature': signature}}}


def validate(files, manifest):
    """The Rust verifier independently checks the signature before retaining the candidate."""
    installer, version = manifest['installer'], manifest['version']
    signature = files[installer + '.sig'].decode('utf-8-sig').strip()
    value = json.loads(files['latest.json'].decode('utf-8-sig'))
    expected = metadata(installer, version, signature, value.get('notes', ''))
    if value != expected:
        raise ValueError('update_manifest_mismatch')
    if not re.fullmatch(r'[a-f0-9]{64}', manifest.get('updatePublicKeySha256', '')):
        raise ValueError('update_public_key_provenance_missing')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('directory', type=Path)
    parser.add_argument('--public-key', type=Path, required=True)
    args = parser.parse_args()
    path = args.directory / 'build-info.json'
    manifest = json.loads(path.read_text(encoding='utf-8-sig'))
    key = args.public_key.read_text().strip()
    if not key:
        raise ValueError('update_public_key_missing')
    signature = (args.directory / (manifest['installer'] + '.sig')).read_text().strip()
    value = metadata(manifest['installer'], manifest['version'], signature,
                     f"FBForge Inspect {manifest['version']} – Versionshinweise im GitHub-Release.")
    manifest['updatePublicKeySha256'] = hashlib.sha256(key.encode()).hexdigest()
    path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (args.directory / 'latest.json').write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


if __name__ == '__main__':
    main()
