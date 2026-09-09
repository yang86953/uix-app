#!/usr/bin/env python3
"""只测试候选协议，不启动框架/图形测试。所有制品均为显式合成 fixture。"""
import io
import json
import tempfile
import tarfile
import unittest
from pathlib import Path
from build_internal_release import make_archive, write_json
from internal_release_contract import CRATES, DOCS, TARGETS, archive_name, hash_file, payload, safe_name
from verify_internal_release import verify
from assemble_candidate import assemble


class CandidateContractTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.stage = self.root / 'stage'
        self.stage.mkdir()

    def fixture(self, platform='linux-x64'):
        version = '0.0.8'
        for name in payload(version, platform):
            path = self.stage / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('synthetic test fixture\n')
        for name in CRATES:
            with tarfile.open(self.stage / f'{name}-{version}.crate', 'w:gz') as archive:
                for filename, content in {
                    'Cargo.toml': f'[package]\nname="{name}"\nversion="{version}"\nlicense-file="LICENSE"\n',
                    'LICENSE': 'synthetic license fixture\n',
                }.items():
                    data = content.encode()
                    info = tarfile.TarInfo(f'{name}-{version}/{filename}')
                    info.size = len(data)
                    archive.addfile(info, io.BytesIO(data))
        license_path = self.stage / 'LICENSE'
        write_json(self.stage / 'RUST_THIRD_PARTY_NOTICES.json', {'schema': 1, 'packages': [{
            'id': 'test-package', 'name': 'test-package', 'files': [{
                'path': 'LICENSE', 'text': license_path.read_text(), 'sha256': hash_file(license_path)}]}]})
        identity = {
            'schema': 1, 'version': version, 'platform': platform, 'target': TARGETS[platform],
            'first_party_crates': list(CRATES), 'distribution': 'internal-candidate-not-published',
            'source': {'commit': '1' * 40}, 'docs': {'commit': '2' * 40, 'sha256': {
                name: hash_file(self.stage / name) for name in DOCS}},
            'locks': {name: hash_file(self.stage / 'inputs' / name) for name in ('Cargo.lock', 'demo-Cargo.lock')},
            'tools': {'cargo': 'synthetic', 'rustc': 'synthetic'}, 'demo_features': {'synthetic': ['default']},
            'rust_build_packages': ['test-package'],
            'payload_sha256': {name: hash_file(self.stage / name) for name in payload(version, platform) if name != 'CANDIDATE.json'},
        }
        write_json(self.stage / 'CANDIDATE.json', identity)
        path = self.root / archive_name(version, platform)
        make_archive(self.stage, path, version, platform)
        return path

    def test_linux_and_windows_share_identity_contract(self):
        for platform in TARGETS:
            path = self.fixture(platform)
            self.assertEqual(verify(path, '0.0.8', platform), hash_file(path))

    def test_payload_tamper_rejected_even_with_updated_outer_hash(self):
        path = self.fixture()
        (self.stage / 'README.md').write_text('tampered')
        make_archive(self.stage, path, '0.0.8', 'linux-x64')
        with self.assertRaisesRegex(ValueError, 'input payload hashes mismatch'):
            verify(path, '0.0.8', 'linux-x64')

    def test_version_mismatch_rejected(self):
        path = self.fixture()
        with self.assertRaisesRegex(ValueError, 'archive name mismatch'):
            verify(path, '0.0.7', 'linux-x64')

    def test_unsafe_names_rejected(self):
        for name in ('../x', '/x', 'a//b', 'C:x', 'a\\b', 'a/./b', ''):
            with self.subTest(name=name), self.assertRaises(ValueError):
                safe_name(name)

    def test_consumer_will_not_replace_existing_directory(self):
        self.fixture()
        output = self.root / 'consumer'
        assemble(self.stage, output)
        self.assertTrue((output / 'crates/uix-0.0.8/Cargo.toml').is_file())
        with self.assertRaisesRegex(ValueError, 'already exists'):
            assemble(self.stage, output)

    def test_consumer_rejects_modified_actual_crate(self):
        self.fixture()
        (self.stage / 'uix-0.0.8.crate').write_bytes(b'tampered')
        with self.assertRaisesRegex(ValueError, 'digest mismatch'):
            assemble(self.stage, self.root / 'consumer')
        self.assertFalse((self.root / 'consumer').exists())


if __name__ == '__main__':
    unittest.main()
