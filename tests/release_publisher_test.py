import copy
import importlib.util
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location('publisher', Path(__file__).resolve().parents[1] / 'scripts/ci/publish-release.py')
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class PublisherTests(unittest.TestCase):
    def setUp(self):
        self.commit = 'a' * 40
        self.run = {'head_sha': self.commit, 'head_branch': 'main',
                    'head_repository': {'full_name': publisher.REPOSITORY},
                    'path': '.github/workflows/sign-windows.yml', 'event': 'workflow_dispatch',
                    'status': 'completed', 'conclusion': 'success'}
        self.jobs = {'total_count': 3, 'jobs': [{'name': name, 'conclusion': 'success'}
                     for name in ('verify-source', 'sign', 'verify-package')]}

    def validate(self, run=None, jobs=None):
        publisher.validate_run(self.run if run is None else run, self.jobs if jobs is None else jobs,
                               self.commit, '.github/workflows/sign-windows.yml', {'workflow_dispatch'},
                               ('verify-source', 'sign', 'verify-package'))

    def test_complete_exact_signing_run_is_accepted(self):
        self.validate()

    def test_failed_foreign_or_wrong_source_runs_are_rejected(self):
        for field, value in [('head_sha', 'b' * 40), ('head_branch', 'feature'),
                             ('head_repository', {'full_name': 'outsider/fork'}),
                             ('path', '.github/workflows/other.yml'), ('event', 'pull_request'),
                             ('status', 'in_progress'), ('conclusion', 'failure')]:
            with self.subTest(field=field), self.assertRaises(RuntimeError):
                self.validate(dict(self.run, **{field: value}))

    def test_missing_duplicate_or_failed_independent_verification_is_rejected(self):
        for jobs in (self.jobs['jobs'][:2], self.jobs['jobs'] * 2,
                     [*self.jobs['jobs'][:2], {'name': 'verify-package', 'conclusion': 'failure'}]):
            with self.subTest(jobs=jobs), self.assertRaises(RuntimeError):
                self.validate(jobs={'total_count': len(jobs), 'jobs': jobs})

    def test_escaping_paths_and_archive_aliases_are_rejected(self):
        for name in ('../x', '/x', 'a/../x', 'a//x', 'a\\x', 'C:/x', './x'):
            with self.subTest(name=name), self.assertRaises(RuntimeError):
                publisher.relative_path(name)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for index, names in enumerate((('../outside',), ('File', 'file'))):
                archive = root / f'{index}.zip'
                with zipfile.ZipFile(archive, 'w') as target:
                    for name in names:
                        target.writestr(name, b'fixture')
                with self.assertRaises(RuntimeError):
                    publisher.extract_archive(archive, root / f'out-{index}')
            self.assertFalse((root / 'outside').exists())

    def test_verified_file_inventory_rejects_mutation_extra_files_and_signer_substitution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            app = root / 'fbforge-inspect.exe'
            installer = root / 'FBForge-Inspect-0.2.0-windows-x64-setup.exe'
            app.write_bytes(b'fictional signed application')
            installer.write_bytes(b'fictional signed installer')
            signer = 'B' * 40
            info = {'application': 'FBForge Inspect', 'repository': publisher.REPOSITORY,
                    'version': '0.2.0', 'commit': self.commit, 'platform': 'windows-x64',
                    'signing': 'authenticode', 'runId': '123', 'validationRunId': '456',
                    'published': False, 'certificateThumbprint': signer,
                    'installer': installer.name, 'bytes': installer.stat().st_size,
                    'sha256': publisher.sha256(installer), 'updatePublicKeySha256': 'e' * 64}
            signatures = {'commit': self.commit, 'runId': '123', 'validationRunId': '456'}
            signatures['update'] = {'verified': True, 'product': 'FBForge Inspect', 'version': '0.2.0', 'publicKeySha256': 'e' * 64}
            (root / (installer.name + '.sig')).write_text('c3ludGhldGlj')
            (root / 'latest.json').write_text(json.dumps(publisher.updates.metadata(installer.name, '0.2.0', 'c3ludGhldGlj')))
            for role, file in [('application', app), ('installer', installer)]:
                signatures[role] = {'verified': True, 'sha256': publisher.sha256(file),
                                    'certificateThumbprint': signer, 'timestampCertificateThumbprint': 'C' * 40}
            (root / 'signature-info.json').write_text(json.dumps(signatures))
            def checksums():
                files = sorted(path for path in root.iterdir() if path.name != 'SHA256SUMS.txt')
                (root / 'SHA256SUMS.txt').write_text(''.join(publisher.sha256(file) + '  ' + file.name + '\n' for file in files))
            checksums()
            publisher.validate_package(root, info, self.commit, '0.2.0', '123', signer)
            for field, value in [('verified', False), ('product', 'FBForge Connect'), ('version', '0.3.0'), ('publicKeySha256', 'f' * 64)]:
                with self.subTest(update_field=field):
                    changed_signatures = copy.deepcopy(signatures)
                    changed_signatures['update'][field] = value
                    (root / 'signature-info.json').write_text(json.dumps(changed_signatures))
                    checksums()
                    with self.assertRaisesRegex(RuntimeError, 'update_verification_missing'):
                        publisher.validate_package(root, info, self.commit, '0.2.0', '123', signer)
            (root / 'signature-info.json').write_text(json.dumps(signatures))
            metadata = publisher.updates.metadata(installer.name, '0.2.0', 'c3ludGhldGlj')
            metadata['platforms']['windows-x86_64']['url'] = 'https://github.com/FBForge/fbforge-connect/releases/download/v0.3.0/setup.exe'
            (root / 'latest.json').write_text(json.dumps(metadata))
            checksums()
            with self.assertRaisesRegex(RuntimeError, 'update_metadata_invalid'):
                publisher.validate_package(root, info, self.commit, '0.2.0', '123', signer)
            (root / 'latest.json').write_text(json.dumps(publisher.updates.metadata(installer.name, '0.2.0', 'c3ludGhldGlj')))
            checksums()
            app.write_bytes(b'changed application')
            with self.assertRaisesRegex(RuntimeError, 'checksum_mismatch'):
                publisher.validate_package(root, info, self.commit, '0.2.0', '123', signer)
            app.write_bytes(b'fictional signed application')
            (root / 'extra.txt').write_text('unexpected')
            with self.assertRaisesRegex(RuntimeError, 'inventory_mismatch'):
                publisher.validate_package(root, info, self.commit, '0.2.0', '123', signer)
            (root / 'extra.txt').unlink()
            changed = copy.deepcopy(info)
            changed['certificateThumbprint'] = 'D' * 40
            with self.assertRaisesRegex(RuntimeError, 'signer_mismatch'):
                publisher.validate_package(root, changed, self.commit, '0.2.0', '123', signer)


    def test_release_assets_keep_exact_verified_engine_scripts_licenses_and_update_signature(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / 'package'
            package.mkdir()
            names = ['engine/inspect-engine.jar', 'cli/inspect.ps1', 'cli/inspect.sh',
                     'cli/README.txt', 'LICENSE', 'THIRD_PARTY_NOTICES.md',
                     'engine/licenses/fixture/LICENSE.txt', 'fbforge-inspect.exe',
                     'FBForge-Inspect-0.2.0-windows-x64-setup.exe',
                     'FBForge-Inspect-0.2.0-windows-x64-setup.exe.sig', 'latest.json']
            for name in names:
                target = package / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(('synthetic public bytes: ' + name).encode())
            files = [package / name for name in names]
            installer = package / names[8]
            info = dict(createdAt='2026-11-06T12:00:00Z', commit=self.commit,
                        validationRunId='456', runId='123', certificateThumbprint='B'*40,
                        updatePublicKeySha256='e'*64)
            proof = publisher.build_assets(package, info, files, installer, root / 'assets',
                                           dict(id=789, digest='sha256:'+'f'*64), '0.2.0')
            self.assertEqual((root / 'assets' / installer.name).read_bytes(), installer.read_bytes())
            self.assertEqual(proof['cli']['sha256'], publisher.sha256(root / 'assets' / proof['cli']['name']))
            with zipfile.ZipFile(root / 'assets' / proof['cli']['name']) as archive:
                self.assertEqual(archive.read('FBForge Inspect CLI/inspect-engine.jar'),
                                 (package / 'engine/inspect-engine.jar').read_bytes())
                self.assertEqual(archive.read('FBForge Inspect CLI/licenses/fixture/LICENSE.txt'),
                                 (package / 'engine/licenses/fixture/LICENSE.txt').read_bytes())
                self.assertFalse(any('runtime/' in name or name.endswith('.exe') for name in archive.namelist()))
            with self.assertRaisesRegex(RuntimeError, 'cli_source_missing'):
                publisher.build_assets(package, info, files[1:], installer, root / 'incomplete',
                                       dict(id=789, digest='sha256:'+'f'*64), '0.2.0')


if __name__ == '__main__':
    unittest.main()
