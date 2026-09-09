#!/usr/bin/env python3
"""独立校验当前候选容器、实际载荷摘要及输入身份；Python 3.11+。"""
import argparse
import hashlib
import io
import json
import re
import sys
import tarfile
import tomllib
import zipfile
from pathlib import Path
from internal_release_contract import (CRATES, DOCS, TARGETS, COMMIT, MANIFEST_NAME,
                                       archive_name, binary_name, hash_file, payload, safe_name)

MAX_JSON = 32 * 1024 * 1024


def validate_contents(read, version, platform):
    names = payload(version, platform)
    sums = read(MANIFEST_NAME, 65536)
    if not sums.endswith(b'\n') or b'\r' in sums:
        raise ValueError('invalid hash manifest newline')
    lines = sums.decode('ascii').splitlines()
    if len(lines) != len(names):
        raise ValueError('hash manifest entry count mismatch')
    actual = {}
    for name, line in zip(names, lines, strict=True):
        match = re.fullmatch(r'([0-9a-f]{64})  (.+)', line)
        if not match or match[2] != name:
            raise ValueError(f'hash manifest entry mismatch: {name}')
        content = read(name)
        actual[name] = hashlib.sha256(content).hexdigest()
        if not content or actual[name] != match[1]:
            raise ValueError(f'empty payload or hash mismatch: {name}')
    identity = json.loads(read('CANDIDATE.json', MAX_JSON))
    if (identity['schema'] != 1 or identity['version'] != version
            or identity['platform'] != platform or identity['target'] != TARGETS[platform]
            or identity['first_party_crates'] != list(CRATES)
            or identity['distribution'] != 'internal-candidate-not-published'):
        raise ValueError('candidate identity mismatch')
    if not COMMIT.fullmatch(identity['source']['commit']) or not COMMIT.fullmatch(identity['docs']['commit']):
        raise ValueError('candidate source/docs must identify full commits')
    expected = {n: h for n, h in actual.items() if n != 'CANDIDATE.json'}
    if identity['payload_sha256'] != expected:
        raise ValueError('candidate input payload hashes mismatch')
    for name in DOCS:
        if identity['docs']['sha256'][name] != actual[name]:
            raise ValueError(f'document input mismatch: {name}')
    for name in ('Cargo.lock', 'demo-Cargo.lock'):
        if identity['locks'][name] != actual['inputs/' + name]:
            raise ValueError(f'lock input mismatch: {name}')
    if not identity['tools']['cargo'] or not identity['tools']['rustc'] or not identity['demo_features']:
        raise ValueError('missing toolchain/features identity')
    for name in CRATES:
        with tarfile.open(fileobj=io.BytesIO(read(f'{name}-{version}.crate')), mode='r:gz') as crate:
            seen = set()
            for member in crate.getmembers():
                safe_name(member.name)
                if (not member.isfile() or not member.name.startswith(f'{name}-{version}/')
                        or member.name.casefold() in seen):
                    raise ValueError(f'unsafe crate entry: {member.name}')
                seen.add(member.name.casefold())
            package = tomllib.loads(crate.extractfile(f'{name}-{version}/Cargo.toml').read().decode())
            if package['package']['name'] != name or package['package']['version'] != version:
                raise ValueError(f'crate identity mismatch: {name}')
            license_path = package['package']['license-file']
            safe_name(license_path)
            if not crate.extractfile(f'{name}-{version}/{license_path}').read().strip():
                raise ValueError(f'missing crate license: {name}')
    notices = json.loads(read('RUST_THIRD_PARTY_NOTICES.json', MAX_JSON))
    if notices['schema'] != 1 or not notices['packages']:
        raise ValueError('missing Rust license inventory')
    for package in notices['packages']:
        if not package['files']:
            raise ValueError(f'missing Rust license texts: {package["name"]}')
        for entry in package['files']:
            safe_name(entry['path'])
            if not entry['text'].strip() or hashlib.sha256(entry['text'].encode()).hexdigest() != entry['sha256']:
                raise ValueError('Rust license content/hash mismatch')
    expected_packages = sorted(identity['rust_build_packages'])
    if expected_packages != sorted(p['id'] for p in notices['packages']):
        raise ValueError('Rust notice inventory does not match recorded build package closure')


def verify(path, version, platform):
    names = payload(version, platform) + (MANIFEST_NAME,)
    if path.name != archive_name(version, platform):
        raise ValueError(f'archive name mismatch: expected {archive_name(version, platform)}')
    if platform == 'linux-x64':
        with path.open('rb') as stream:
            header = stream.read(10)
        if len(header) != 10 or header[:4] != b'\x1f\x8b\x08\x00' or header[4:8] != bytes(4):
            raise ValueError('noncanonical gzip header')
        with tarfile.open(path, 'r:gz') as archive:
            members = archive.getmembers()
            for member in members:
                safe_name(member.name)
                mode = 0o755 if member.name == binary_name(platform) else 0o644
                if (not member.isfile() or member.pax_headers or member.mtime != 0
                        or member.uid != 0 or member.gid != 0 or member.mode != mode
                        or member.uname or member.gname):
                    raise ValueError(f'noncanonical tar metadata: {member.name}')
            if tuple(m.name for m in members) != names:
                raise ValueError('archive entry order/closure mismatch')
            def read(name, limit=None):
                member = archive.getmember(name)
                if limit is not None and member.size > limit:
                    raise ValueError(f'oversize metadata: {name}')
                with archive.extractfile(member) as stream:
                    return stream.read()
            validate_contents(read, version, platform)
    else:
        with zipfile.ZipFile(path) as archive:
            members = archive.infolist()
            for member in members:
                safe_name(member.filename)
                if (member.is_dir() or member.date_time != (1980, 1, 1, 0, 0, 0)
                        or member.extra or member.comment or (member.external_attr >> 16) != 0o100644):
                    raise ValueError(f'noncanonical zip metadata: {member.filename}')
            if tuple(m.filename for m in members) != names or archive.comment:
                raise ValueError('archive entry order/closure mismatch')
            def read(name, limit=None):
                if limit is not None and archive.getinfo(name).file_size > limit:
                    raise ValueError(f'oversize metadata: {name}')
                return archive.read(name)
            validate_contents(read, version, platform)
    return hash_file(path)


def verify_archive(path, version):
    return verify(path, version, 'linux-x64')


def verify_zip_archive(path, version):
    return verify(path, version, 'win-x64')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--archive', type=Path, required=True)
    parser.add_argument('--version', required=True)
    args = parser.parse_args()
    try:
        platform = 'linux-x64' if args.archive.name.endswith('.tar.gz') else 'win-x64'
        print(f'Verified SHA256: {verify(args.archive, args.version, platform)}')
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError, zipfile.BadZipFile) as exc:
        print(f'Internal release verification failed: {exc}', file=sys.stderr)
        sys.exit(1)
