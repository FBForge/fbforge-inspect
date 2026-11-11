"""Promote a completed signing run to the existing, immutable source tag."""

import hashlib
import importlib.util
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
import zipfile
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath

REPOSITORY = 'FBForge/fbforge-inspect'
_update_spec = importlib.util.spec_from_file_location('inspect_update_release', Path(__file__).resolve().parents[1] / 'update-release.py')
updates = importlib.util.module_from_spec(_update_spec)
_update_spec.loader.exec_module(updates)


def require(condition, reason):
    if not condition:
        raise RuntimeError(reason)


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def api(path, method='GET', payload=None, binary=None):
    command = ['gh', 'api', '--method', method, path]
    temporary = None
    try:
        if payload is not None:
            with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8', delete=False) as stream:
                json.dump(payload, stream)
                temporary = Path(stream.name)
            command += ['--input', str(temporary)]
        if binary is not None:
            command += ['-H', 'Content-Type: application/octet-stream', '--input', str(binary)]
        result = subprocess.run(command, capture_output=True, timeout=300, check=False)
        require(result.returncode == 0, 'github_release_request_failed')
        return json.loads(result.stdout)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def validate_run(run, jobs, commit, workflow, events, names):
    require(run.get('head_sha') == commit and run.get('head_branch') == 'main', 'release_source_mismatch')
    require(run.get('head_repository', {}).get('full_name') == REPOSITORY, 'release_repository_mismatch')
    require(run.get('path') == workflow and run.get('event') in events, 'release_workflow_mismatch')
    require(run.get('status') == 'completed' and run.get('conclusion') == 'success', 'release_run_not_successful')
    require(jobs.get('total_count', 0) <= 100, 'release_job_limit')
    for name in names:
        matches = [job for job in jobs['jobs'] if job.get('name') == name]
        require(len(matches) == 1 and matches[0].get('conclusion') == 'success', 'release_validation_missing:' + name)


def relative_path(name):
    require(isinstance(name, str) and name and not name.startswith('/') and '\\' not in name and ':' not in name,
            'release_path_invalid')
    require(all(part not in ('', '.', '..') for part in name.split('/')), 'release_path_invalid')
    return PurePosixPath(name)


def extract_archive(archive, destination):
    with zipfile.ZipFile(archive) as source:
        entries = source.infolist()
        require(len(entries) <= 10000 and sum(entry.file_size for entry in entries) <= 1024 ** 3,
                'release_archive_budget')
        seen = set()
        for entry in entries:
            name = entry.filename.rstrip('/')
            relative_path(name)
            require(name.casefold() not in seen, 'release_duplicate_path')
            seen.add(name.casefold())
            require(stat.S_IFMT(entry.external_attr >> 16) in (0, stat.S_IFREG, stat.S_IFDIR), 'release_link_denied')
            require(entry.file_size <= 512 * 1024 ** 2, 'release_file_budget')
        destination.mkdir()
        for entry in entries:
            target = destination.joinpath(*PurePosixPath(entry.filename).parts)
            if not target.resolve().is_relative_to(destination.resolve()):
                raise RuntimeError('release_path_escape')
            if entry.is_dir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                with source.open(entry) as content, target.open('xb') as output:
                    shutil.copyfileobj(content, output)


def validate_package(package, info, commit, version, signing_run, signer):
    require(info.get('application') == 'FBForge Inspect' and info.get('repository') == REPOSITORY,
            'release_package_identity')
    require(info.get('commit') == commit and info.get('version') == version
            and info.get('platform') == 'windows-x64' and info.get('signing') == 'authenticode',
            'release_package_source')
    require(str(info.get('runId')) == signing_run and info.get('published') is False,
            'release_package_run')
    require(info.get('certificateThumbprint', '').upper() == signer.upper(), 'release_signer_mismatch')
    expected = {}
    for line in (package / 'SHA256SUMS.txt').read_text(encoding='utf-8-sig').splitlines():
        match = re.fullmatch(r'([a-f0-9]{64})  (.+)', line)
        require(match is not None, 'release_checksum_format')
        path = relative_path(match[2])
        require(match[2] not in expected and match[2] != 'SHA256SUMS.txt', 'release_checksum_duplicate')
        file = package.joinpath(*path.parts)
        require(file.is_file() and not file.is_symlink() and sha256(file) == match[1], 'release_checksum_mismatch')
        expected[match[2]] = match[1]
    files = [path for path in package.rglob('*') if path.is_file()]
    require(len(files) == len(expected) + 1 and not any(path.is_symlink() for path in package.rglob('*')),
            'release_inventory_mismatch')
    name = info.get('installer', '')
    relative_path(name)
    require('/' not in name and name.endswith('-setup.exe'), 'release_installer_name')
    installer = package / name
    require(sha256(installer) == info.get('sha256') and installer.stat().st_size == info.get('bytes'),
            'release_installer_mismatch')
    signatures = json.loads((package / 'signature-info.json').read_text(encoding='utf-8-sig'))
    require(signatures.get('commit') == commit and str(signatures.get('runId')) == signing_run
            and str(signatures.get('validationRunId')) == str(info.get('validationRunId')),
            'release_signature_provenance')
    for role, file in (('application', package / 'fbforge-inspect.exe'), ('installer', installer)):
        signature = signatures.get(role, {})
        require(signature.get('verified') is True and signature.get('sha256') == sha256(file)
                and signature.get('certificateThumbprint', '').upper() == signer.upper()
                and re.fullmatch(r'[a-fA-F0-9]{40}', signature.get('timestampCertificateThumbprint', '')),
                'release_signature_mismatch')
    update = signatures.get('update', {})
    require(update.get('verified') is True and update.get('product') == 'FBForge Inspect'
            and update.get('version') == version
            and update.get('publicKeySha256') == info.get('updatePublicKeySha256'),
            'release_update_verification_missing')
    try:
        updates.validate({name + '.sig': (package / (name + '.sig')).read_bytes(),
                          'latest.json': (package / 'latest.json').read_bytes()}, info)
    except (ValueError, KeyError, OSError, UnicodeError):
        raise RuntimeError('release_update_metadata_invalid') from None
    return files, installer


def tag_commit(tag):
    item = api(f'repos/{REPOSITORY}/git/ref/tags/{tag}')['object']
    for _ in range(5):
        require(re.fullmatch('[a-f0-9]{40}', item.get('sha', '')), 'release_tag_invalid')
        if item.get('type') == 'commit':
            return item['sha']
        require(item.get('type') == 'tag', 'release_tag_invalid')
        item = api(f'repos/{REPOSITORY}/git/tags/{item["sha"]}')['object']
    raise RuntimeError('release_tag_depth')


def build_assets(package, info, files, installer, output, artifact, version):
    output.mkdir()
    prefix = 'FBForge-Inspect-' + version + '-windows-x64'
    target = output / (prefix + '-setup.exe')
    shutil.copyfile(installer, target)
    shutil.copyfile(package / (installer.name + '.sig'), output / (target.name + '.sig'))
    shutil.copyfile(package / 'latest.json', output / 'latest.json')
    archive = output / (prefix + '.zip')
    created = datetime.fromisoformat(info['createdAt'].replace('Z', '+00:00')).astimezone(timezone.utc)
    with zipfile.ZipFile(archive, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as destination:
        for file in sorted(files):
            entry = zipfile.ZipInfo('FBForge Inspect/' + file.relative_to(package).as_posix(), created.timetuple()[:6])
            entry.compress_type = zipfile.ZIP_DEFLATED
            entry.external_attr = (stat.S_IFREG | 0o644) << 16
            with file.open('rb') as source, destination.open(entry, 'w') as sink:
                shutil.copyfileobj(source, sink)
    cli_archive = output / ('FBForge-Inspect-' + version + '-cli-java.zip')
    with zipfile.ZipFile(cli_archive, 'x', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as destination:
        for source, name in [
            (package / 'engine/inspect-engine.jar', 'inspect-engine.jar'),
            (package / 'cli/inspect.ps1', 'inspect.ps1'),
            (package / 'cli/inspect.sh', 'inspect.sh'),
            (package / 'cli/README.txt', 'README.txt'),
            (package / 'LICENSE', 'LICENSE'),
            (package / 'THIRD_PARTY_NOTICES.md', 'THIRD_PARTY_NOTICES.md'),
        ]:
            require(source in files, 'release_cli_source_missing')
            destination.write(source, 'FBForge Inspect CLI/' + name)
        for source in files:
            if source.is_relative_to(package / 'engine/licenses'):
                destination.write(source, 'FBForge Inspect CLI/licenses/' + source.relative_to(package / 'engine/licenses').as_posix())
    proof = {
        'application': 'FBForge Inspect', 'version': version, 'tag': version, 'repository': REPOSITORY,
        'commit': info['commit'], 'validationRun': int(info['validationRunId']), 'signingRun': int(info['runId']),
        'artifactId': artifact['id'], 'artifactDigest': artifact['digest'],
        'certificateThumbprint': info['certificateThumbprint'], 'verifiedPackageFileCount': len(files),
        'publisherRun': os.environ.get('GITHUB_RUN_ID'), 'publisherCommit': os.environ.get('GITHUB_SHA'),
        'installer': {'name': target.name, 'sha256': sha256(target)},
        'archive': {'name': archive.name, 'sha256': sha256(archive)},
        'cli': {'name': cli_archive.name, 'sha256': sha256(cli_archive)},
        'updatePublicKeySha256': info['updatePublicKeySha256'],
    }
    (output / 'release-info.json').write_text(json.dumps(proof, indent=2) + '\n')
    sums = ''.join(sha256(file) + '  ' + file.name + '\n' for file in sorted(output.iterdir()))
    (output / 'SHA256SUMS.txt').write_text(sums)
    return proof


def main():
    require(os.environ.get('GITHUB_REPOSITORY') == REPOSITORY
            and os.environ.get('GITHUB_REF') == 'refs/heads/main'
            and os.environ.get('GITHUB_EVENT_NAME') == 'workflow_dispatch', 'release_context_denied')
    signing_run = os.environ.get('SIGNING_RUN', '')
    signer = os.environ.get('EXPECTED_SIGNER', '')
    require(re.fullmatch('[1-9][0-9]{0,18}', signing_run), 'release_run_invalid')
    require(re.fullmatch('[a-fA-F0-9]{40}', signer), 'release_signer_missing')
    version = json.loads(Path('package.json').read_text())['version']
    require(re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', version), 'release_version_invalid')
    commit = tag_commit(version)
    run = api(f'repos/{REPOSITORY}/actions/runs/{signing_run}')
    jobs = api(f'repos/{REPOSITORY}/actions/runs/{signing_run}/jobs?filter=latest&per_page=100')
    validate_run(run, jobs, commit, '.github/workflows/sign-windows.yml', {'workflow_dispatch'},
                 ('verify-source', 'sign', 'verify-package'))
    artifacts = api(f'repos/{REPOSITORY}/actions/runs/{signing_run}/artifacts?name=windows-signed-verified&per_page=100')
    require(artifacts.get('total_count') == 1, 'release_artifact_ambiguous')
    artifact = artifacts['artifacts'][0]
    require(artifact.get('name') == 'windows-signed-verified' and artifact.get('expired') is False
            and artifact.get('workflow_run', {}).get('id') == int(signing_run)
            and artifact['workflow_run'].get('head_sha') == commit, 'release_artifact_source')
    with tempfile.TemporaryDirectory(prefix='inspect-publish-', dir=os.environ['RUNNER_TEMP']) as directory:
        root = Path(directory)
        archive = root / 'verified.zip'
        with archive.open('wb') as stream:
            result = subprocess.run(['gh', 'api', f'repos/{REPOSITORY}/actions/artifacts/{artifact["id"]}/zip'],
                                    stdout=stream, stderr=subprocess.PIPE, timeout=300)
        require(result.returncode == 0 and artifact.get('digest') == 'sha256:' + sha256(archive),
                'release_artifact_digest_mismatch')
        extract_archive(archive, root / 'extracted')
        candidates = list((root / 'extracted').rglob('build-info.json'))
        require(len(candidates) == 1, 'release_package_ambiguous')
        package = candidates[0].parent
        info = json.loads(candidates[0].read_text(encoding='utf-8-sig'))
        files, installer = validate_package(package, info, commit, version, signing_run, signer)
        # Verify these exact Windows-built engine bytes on Linux before publishing the portable CLI.
        probe = subprocess.run(['python3', 'scripts/ci/cli-smoke.py', str(package / 'engine/inspect-engine.jar')],
                               timeout=120, check=False)
        require(probe.returncode == 0, 'release_cli_cross_platform_validation_failed')
        validation = str(info['validationRunId'])
        require(re.fullmatch('[1-9][0-9]{0,18}', validation), 'release_validation_run_invalid')
        validate_run(api(f'repos/{REPOSITORY}/actions/runs/{validation}'),
                     api(f'repos/{REPOSITORY}/actions/runs/{validation}/jobs?filter=latest&per_page=100'),
                     commit, '.github/workflows/verify.yml', {'push', 'workflow_dispatch'}, ('verify', 'windows'))
        proof = build_assets(package, info, files, installer, root / 'assets', artifact, version)
        releases = api(f'repos/{REPOSITORY}/releases?per_page=100')
        matches = [release for release in releases if release.get('tag_name') == version]
        require(len(matches) == 1 and matches[0].get('draft') is True, 'release_draft_required')
        release = matches[0]
        endpoint = f'repos/{REPOSITORY}/releases/{release["id"]}'
        existing = {asset['name']: asset for asset in release['assets']}
        assets = sorted((root / 'assets').iterdir())
        require(set(existing).issubset({file.name for file in assets}), 'release_extra_asset')
        for file in assets:
            if file.name not in existing:
                api(f'https://uploads.github.com/{endpoint}/assets?name={file.name}', method='POST', binary=file)
        uploaded = api(endpoint)
        received = {asset['name']: asset for asset in uploaded['assets']}
        require(set(received) == {file.name for file in assets}, 'release_asset_inventory')
        for file in assets:
            asset = received[file.name]
            require(asset.get('state') == 'uploaded' and asset.get('size') == file.stat().st_size
                    and asset.get('digest') == 'sha256:' + sha256(file), 'release_uploaded_digest_mismatch')
        require(tag_commit(version) == commit, 'release_tag_changed')
        published = api(endpoint, method='PATCH', payload={
            'tag_name': version, 'target_commitish': commit, 'name': release['name'],
            'body': release['body'], 'draft': False, 'prerelease': False, 'make_latest': 'true',
        })
        require(published.get('draft') is False and published.get('tag_name') == version, 'release_not_published')
        proof['releaseUrl'] = published['html_url']
        print(json.dumps(proof, indent=2))
        with Path(os.environ['GITHUB_STEP_SUMMARY']).open('a') as summary:
            summary.write(f'Release [{version}]({published["html_url"]}) published from `{commit}`.\n')


if __name__ == '__main__':
    main()
