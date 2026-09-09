#!/usr/bin/env python3
"""将解包后的当前候选四个真实 crate 装配到全新消费者目录；不改全局 Cargo 配置。"""
import argparse
import hashlib
import json
import re
import shutil
import tarfile
import tempfile
import tomllib
from pathlib import Path

CRATES = ('uix', 'uix-derive', 'uix-lang-compiler', 'uix-lang-runtime')


def assemble(candidate, output, portable=False):
    candidate, output = candidate.resolve(), output.resolve()
    if output.exists():
        raise ValueError(f'consumer output already exists: {output}')
    manifest = json.loads((candidate / 'CANDIDATE.json').read_text(encoding='utf-8'))
    version = manifest['version']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+', version):
        raise ValueError('invalid candidate version')
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.uix-consumer-', dir=output.parent) as tmp:
        stage = Path(tmp)
        registries = set()
        for name in CRATES:
            filename = f'{name}-{version}.crate'
            data = (candidate / filename).read_bytes()
            if hashlib.sha256(data).hexdigest() != manifest['payload_sha256'][filename]:
                raise ValueError(f'candidate crate digest mismatch: {filename}')
            root = f'{name}-{version}'
            with tarfile.open(candidate / filename, 'r:gz') as archive:
                seen = set()
                for member in archive.getmembers():
                    parts = member.name.split('/')
                    if (not member.isfile() or not parts or parts[0] != root or len(parts) < 2
                            or any(p in ('', '.', '..') for p in parts)
                            or '\\' in member.name or ':' in member.name
                            or member.name.casefold() in seen):
                        raise ValueError(f'unsafe crate member: {member.name}')
                    seen.add(member.name.casefold())
                    dest = stage / 'crates' / member.name
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    with archive.extractfile(member) as src, dest.open('wb') as dst:
                        shutil.copyfileobj(src, dst)
            metadata = tomllib.loads((stage / 'crates' / root / 'Cargo.toml').read_text())
            if metadata['package']['name'] != name or metadata['package']['version'] != version:
                raise ValueError(f'crate identity mismatch: {filename}')
            def visit(value):
                if isinstance(value, dict):
                    for key, val in value.items():
                        if key == 'registry-index':
                            registries.add(val)
                        visit(val)
                elif isinstance(value, list):
                    for val in value:
                        visit(val)
            visit(metadata)
        dep = f'path = "crates/uix-{version}"'
        if portable:
            dep += ', default-features = false, features = ["uix-dynamic"]'
        text = '[package]\nname = "uix-candidate-consumer"\nversion = "0.1.0"\nedition = "2024"\n\n[workspace]\n\n[dependencies]\nuix = { ' + dep + ' }\n'
        for registry in sorted(registries):
            text += '\n[patch.' + json.dumps(registry) + ']\n'
            for name in CRATES:
                text += f'{name} = {{ path = "crates/{name}-{version}" }}\n'
        (stage / 'Cargo.toml').write_text(text, encoding='utf-8')
        (stage / 'src').mkdir()
        (stage / 'src/main.rs').write_text('fn main() {}\n', encoding='utf-8')
        # 同文件系统提交完整目录；失败不改已存在消费者。
        stage.rename(output)
    return output


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--portable', action='store_true', help='关闭 UIX 默认 feature，仅启用 uix-dynamic')
    args = parser.parse_args()
    try:
        print(assemble(args.candidate, args.output, args.portable))
    except (OSError, ValueError, KeyError, tarfile.TarError) as exc:
        parser.exit(1, f'Candidate assembly failed: {exc}\n')
