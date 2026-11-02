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
            installer = root / 'fixture-setup.exe'
            app.write_bytes(b'fictional signed application')
            installer.write_bytes(b'fictional signed installer')
            signer = 'B' * 40
            info = {'application': 'FBForge Inspect', 'repository': publisher.REPOSITORY,
                    'version': '0.1.0', 'commit': self.commit, 'platform': 'windows-x64',
                    'signing': 'authenticode', 'runId': '123', 'validationRunId': '456',
                    'published': False, 'certificateThumbprint': signer,
                    'installer': installer.name, 'bytes': installer.stat().st_size,
                    'sha256': publisher.sha256(installer)}
            signatures = {'commit': self.commit, 'runId': '123', 'validationRunId': '456'}
            for role, file in [('application', app), ('installer', installer)]:
                signatures[role] = {'verified': True, 'sha256': publisher.sha256(file),
                                    'certificateThumbprint': signer, 'timestampCertificateThumbprint': 'C' * 40}
            (root / 'signature-info.json').write_text(json.dumps(signatures))
            def checksums():
                files = sorted(path for path in root.iterdir() if path.name != 'SHA256SUMS.txt')
                (root / 'SHA256SUMS.txt').write_text(''.join(publisher.sha256(file) + '  ' + file.name + '\n' for file in files))
            checksums()
            publisher.validate_package(root, info, self.commit, '0.1.0', '123', signer)
            app.write_bytes(b'changed application')
            with self.assertRaisesRegex(RuntimeError, 'checksum_mismatch'):
                publisher.validate_package(root, info, self.commit, '0.1.0', '123', signer)
            app.write_bytes(b'fictional signed application')
            (root / 'extra.txt').write_text('unexpected')
            with self.assertRaisesRegex(RuntimeError, 'inventory_mismatch'):
                publisher.validate_package(root, info, self.commit, '0.1.0', '123', signer)
            (root / 'extra.txt').unlink()
            changed = copy.deepcopy(info)
            changed['certificateThumbprint'] = 'D' * 40
            with self.assertRaisesRegex(RuntimeError, 'signer_mismatch'):
                publisher.validate_package(root, changed, self.commit, '0.1.0', '123', signer)


if __name__ == '__main__':
    unittest.main()
