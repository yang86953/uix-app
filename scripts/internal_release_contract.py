"""UIX 当前内部候选的共同载荷合同（不是正式 Registry 发布）。"""
import hashlib
import re
from pathlib import PurePosixPath

CRATES = ('uix', 'uix-derive', 'uix-lang-compiler', 'uix-lang-runtime')
DOCS = ('README.md', 'CHANGELOG.md', 'THIRD_PARTY_NOTICES.md')
DOC_PREFIX = 'projects/uix-app/docs/delivery'
VERSION = re.compile(r'[0-9]+\.[0-9]+\.[0-9]+')
SHA256 = re.compile(r'[0-9a-f]{64}')
COMMIT = re.compile(r'[0-9a-f]{40}')
TARGETS = {'linux-x64': 'x86_64-unknown-linux-gnu', 'win-x64': 'x86_64-pc-windows-msvc'}
MANIFEST_NAME = 'SHA256SUMS.txt'


def binary_name(platform):
    return 'bin/uix-lang-demo' if platform == 'linux-x64' else 'uix-lang-demo.exe'


def payload(version, platform):
    if not VERSION.fullmatch(version) or platform not in TARGETS:
        raise ValueError('invalid candidate version or platform')
    return (binary_name(platform), *(f'{name}-{version}.crate' for name in CRATES),
            'LICENSE', *DOCS, 'assets/images/demo.png', 'RUST_THIRD_PARTY_NOTICES.json',
            'inputs/Cargo.lock', 'inputs/demo-Cargo.lock', 'assemble_candidate.py', 'CANDIDATE.json')


def archive_name(version, platform):
    return f'uix-{version}-internal-{platform}' + ('.tar.gz' if platform == 'linux-x64' else '.zip')


def safe_name(name):
    if (not name or '\\' in name or ':' in name or name.startswith('/')
            or any(p in ('', '.', '..') for p in name.split('/'))
            or str(PurePosixPath(name)) != name):
        raise ValueError(f'unsafe archive entry: {name}')


def hash_file(path):
    with open(path, 'rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()
