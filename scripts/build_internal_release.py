#!/usr/bin/env python3
"""从干净源码和显式 Git 文档提交构建内部候选；普通失败保留原容器。"""
import argparse
import gzip
import json
import os
import platform as host_platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path
from internal_release_contract import (CRATES, DOCS, DOC_PREFIX, VERSION, COMMIT, TARGETS,
                                       MANIFEST_NAME, archive_name, binary_name, payload, hash_file)
from verify_internal_release import verify

ROOT = Path(__file__).resolve().parent.parent


def run(args, cwd=ROOT, capture=False):
    print('+ ' + ' '.join(map(str, args)), file=sys.stderr, flush=True)
    return subprocess.run(list(map(str, args)), cwd=cwd, check=True,
                          stdout=subprocess.PIPE if capture else None).stdout


def metadata(manifest, full=False, target=None):
    args = ['cargo', 'metadata', '--format-version', '1', '--locked', '--offline', '--manifest-path', manifest]
    if not full:
        args += ['--no-deps']
    if target:
        args += ['--filter-platform', target]
    return json.loads(run(args, capture=True))


def source_identity(expected_version):
    root = metadata(ROOT / 'Cargo.toml')
    demo = metadata(ROOT / 'demo/Cargo.toml')
    packages = {p['name']: p for p in root['packages']}
    version = packages['uix-app']['version']
    if not VERSION.fullmatch(version) or (expected_version and version != expected_version):
        raise ValueError(f'version mismatch: source {version}, requested {expected_version}')
    for name in CRATES:
        if packages[name]['version'] != version or not packages[name]['description']:
            raise ValueError(f'first-party metadata mismatch: {name}')
        for dep in packages[name]['dependencies']:
            if dep['name'] in CRATES and dep['kind'] != 'dev' and dep['req'] != '^' + version:
                raise ValueError(f'first-party dependency version mismatch: {name} -> {dep["name"]}')
    demos = [p for p in demo['packages'] if p['name'] == 'uix-lang-demo']
    if len(demos) != 1 or demos[0]['version'] != version:
        raise ValueError('demo/source version mismatch')
    return version, root, demo


def export_docs(repo, revision, stage):
    if not COMMIT.fullmatch(revision):
        raise ValueError('docs revision must be a full 40-character commit')
    commit = run(['git', '-C', repo, 'rev-parse', revision + '^{commit}'], capture=True).decode().strip()
    if commit != revision:
        raise ValueError('docs revision is not the specified commit')
    hashes = {}
    for name in DOCS:
        data = run(['git', '-C', repo, 'show', f'{revision}:{DOC_PREFIX}/{name}'], capture=True)
        if not data.strip():
            raise ValueError(f'empty required document: {name}')
        (stage / name).write_bytes(data)
        hashes[name] = hash_file(stage / name)
    return {'commit': revision, 'prefix': DOC_PREFIX, 'sha256': hashes}


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + '\n', encoding='utf-8')


def rust_notices(meta, artifacts, output):
    packages = {p['id']: p for p in meta['packages']}
    collected = []
    for package_id in sorted(artifacts):
        p = packages[package_id]
        if not p['source']:
            continue
        root = Path(p['manifest_path']).parent
        files = []
        declared = root / p['license_file'] if p.get('license_file') else None
        for path in sorted(root.rglob('*')):
            if path.is_file() and (path == declared or path.name.upper().startswith(
                    ('LICENSE', 'LICENCE', 'COPYING', 'NOTICE', 'COPYRIGHT'))):
                # 按原包文本逐字节解码；不裁剪行尾或凭 SPDX 名称编造许可证。
                text = path.read_bytes().decode('utf-8')
                if text.strip():
                    files.append({'path': path.relative_to(root).as_posix(), 'sha256': hash_file(path), 'text': text})
        if not files:
            raise ValueError(f'no packaged license/notice texts: {p["name"]} {p["version"]}')
        collected.append({'id': package_id, 'name': p['name'], 'version': p['version'],
                          'source': p['source'], 'license_expression': p['license'], 'files': files})
    if not collected:
        raise ValueError('no Rust third-party build artifacts recorded')
    sysroot = Path(run(['rustc', '--print', 'sysroot'], capture=True).decode().strip())
    rust_docs = sysroot / 'share/doc/rust'
    library_copyright = rust_docs / 'COPYRIGHT-library.html'
    if not library_copyright.is_file() or not (rust_docs / 'licenses').is_dir():
        raise ValueError('Rust toolchain standard-library copyright/license materials are missing')
    std_files = []
    for path in [library_copyright, *sorted((rust_docs / 'licenses').glob('*.txt'))]:
        text = path.read_bytes().decode('utf-8')
        if not text.strip():
            raise ValueError(f'empty Rust toolchain license: {path.name}')
        std_files.append({'path': path.relative_to(rust_docs).as_posix(), 'text': text, 'sha256': hash_file(path)})
    write_json(output, {'schema': 1, 'rust_standard_library': {'scope': 'Toolchain-shipped complete standard-library copyright inventory and referenced license directory; conservative, not target-level reachability', 'files': std_files}, 'scope': 'Cargo compiler-artifact package set for the demo build; includes build/proc-macro dependencies, not a claim of exact linker-symbol reachability', 'packages': collected})
    return [p['id'] for p in collected]


def make_archive(stage, path, version, platform):
    names = payload(version, platform)
    (stage / MANIFEST_NAME).write_text(''.join(f'{hash_file(stage / n)}  {n}\n' for n in names), encoding='ascii')
    if platform == 'linux-x64':
        with path.open('wb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0, compresslevel=9) as zipped:
            with tarfile.open(fileobj=zipped, mode='w', format=tarfile.USTAR_FORMAT) as archive:
                for name in names + (MANIFEST_NAME,):
                    info = tarfile.TarInfo(name)
                    info.size = (stage / name).stat().st_size
                    info.mode = 0o755 if name == binary_name(platform) else 0o644
                    with (stage / name).open('rb') as stream:
                        archive.addfile(info, stream)
    else:
        with zipfile.ZipFile(path, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for name in names + (MANIFEST_NAME,):
                info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                info.create_system = 3
                info.external_attr = 0o100644 << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                archive.writestr(info, (stage / name).read_bytes(), compresslevel=9)


def build(args):
    # 版本/文档门禁优先于昂贵构建和旧制品替换。
    version, root_meta, demo_meta = source_identity(args.version)
    demo_target = Path(demo_meta['target_directory']).resolve()
    if not demo_target.is_relative_to(ROOT) or demo_target == ROOT:
        raise ValueError('demo target directory must be inside source repository')
    source_commit = run(['git', 'rev-parse', 'HEAD'], capture=True).decode().strip()
    if run(['git', 'status', '--porcelain=v1', '--untracked-files=all'], capture=True).strip():
        raise ValueError('internal release requires a clean worktree')
    target_root = Path(root_meta['target_directory']).resolve()
    if not target_root.is_relative_to(ROOT) or target_root == ROOT:
        raise ValueError('candidate target directory must be inside source repository')
    system, machine = host_platform.system(), host_platform.machine().lower()
    if machine not in ('x86_64', 'amd64'):
        raise ValueError('candidate build requires an x64 host')
    if (args.platform == 'linux-x64' and system != 'Linux') or (args.cross_windows and system != 'Linux'):
        raise ValueError('this entry requires a Linux host')
    if args.platform == 'win-x64' and not args.cross_windows and system != 'Windows':
        raise ValueError('native Windows entry requires a Windows host')
    target = TARGETS[args.platform]
    output = target_root / 'internal-release'
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.candidate-', dir=output) as tmp:
        tmp = Path(tmp)
        stage = tmp / 'stage'
        stage.mkdir()
        docs = export_docs(args.docs_repo, args.docs_revision, stage)
        locks = {}
        (stage / 'inputs').mkdir()
        for source, name in ((ROOT / 'Cargo.lock', 'Cargo.lock'), (ROOT / 'demo/Cargo.lock', 'demo-Cargo.lock')):
            shutil.copyfile(source, stage / 'inputs' / name)
            locks[name] = hash_file(source)
        meta = metadata(ROOT / 'demo/Cargo.toml', full=True, target=target)
        driver = ['cargo']
        tools = {'cargo': run(['cargo', '--version'], capture=True).decode().strip(),
                 'rustc': run(['rustc', '-vV'], capture=True).decode().strip(),
                 'python': sys.version, 'host': system + '/' + machine}
        if args.cross_windows:
            tools['cargo-xwin'] = run(['cargo', 'xwin', '--version'], capture=True).decode().strip()
            if tools['cargo-xwin'] not in ('cargo-xwin 0.23.1', 'cargo-xwin-xwin 0.23.1'):
                raise ValueError('Windows cross build requires cargo-xwin 0.23.1')
            driver += ['xwin']
        command = driver + ['build', '--release', '--locked', '--offline', '--manifest-path', str(ROOT / 'demo/Cargo.toml'),
                            '--bin', 'uix-lang-demo', '--target', target, '--message-format=json-render-diagnostics']
        log_root = target_root / 'internal-release-logs'
        log_root.mkdir(exist_ok=True)
        json_log = log_root / f'{args.platform}-cargo-build.jsonl'
        artifacts = set()
        executable = None
        print('+ ' + ' '.join(command), file=sys.stderr, flush=True)
        with json_log.open('wb') as log:
            process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE)
            for line in process.stdout:
                log.write(line)
                message = json.loads(line)
                if message.get('reason') == 'compiler-artifact':
                    artifacts.add(message['package_id'])
                    if message['target']['name'] == 'uix-lang-demo' and message.get('executable'):
                        executable = Path(message['executable'])
            if process.wait() != 0:
                raise ValueError(f'demo build failed; see {json_log}')
        if executable is None:
            raise ValueError('demo build produced no executable artifact')
        built_packages = rust_notices(meta, artifacts, stage / 'RUST_THIRD_PARTY_NOTICES.json')
        package_cmd = ['cargo', 'package', '--registry', 'gitea', '--locked', '--offline', '--no-verify', '--target-dir', tmp / 'cargo']
        for name in CRATES:
            package_cmd += ['-p', name]
        run(package_cmd)
        for name in CRATES:
            filename = f'{name}-{version}.crate'
            shutil.copyfile(tmp / 'cargo/package' / filename, stage / filename)
        for source, name in ((executable, binary_name(args.platform)), (ROOT / 'LICENSE', 'LICENSE'),
                             (ROOT / 'assets/images/demo.png', 'assets/images/demo.png'),
                             (ROOT / 'scripts/assemble_candidate.py', 'assemble_candidate.py')):
            (stage / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, stage / name)
        for source, name in ((ROOT / 'Cargo.lock', 'Cargo.lock'), (ROOT / 'demo/Cargo.lock', 'demo-Cargo.lock')):
            if hash_file(source) != locks[name]:
                raise ValueError(f'lockfile changed during build: {name}')
        if source_commit != run(['git', 'rev-parse', 'HEAD'], capture=True).decode().strip() or run(
                ['git', 'status', '--porcelain=v1', '--untracked-files=all'], capture=True).strip():
            raise ValueError('source changed during candidate build')
        features = {node['id']: node['features'] for node in meta['resolve']['nodes'] if node['id'] in artifacts}
        write_json(stage / 'CANDIDATE.json', {
            'schema': 1, 'version': version, 'platform': args.platform, 'target': target,
            'distribution': 'internal-candidate-not-published', 'source': {'commit': source_commit}, 'docs': docs,
            'first_party_crates': list(CRATES), 'locks': locks, 'tools': tools,
            'demo_features': features, 'demo_build_command': command, 'rust_build_packages': built_packages,
            'excluded': ['CLI/LSP binaries', 'Registry publication', 'unmeasured platforms and features'],
            'payload_sha256': {name: hash_file(stage / name) for name in payload(version, args.platform) if name != 'CANDIDATE.json'}})
        archive = tmp / archive_name(version, args.platform)
        make_archive(stage, archive, version, args.platform)
        digest = verify(archive, version, args.platform)
        final = output / archive.name
        # 唯一正式输出在所有检查后替换；不宣称断电持久化或多文件事务。
        os.replace(archive, final)
        print(f'Artifact: {final}\nSHA256: {digest}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--docs-repo', type=Path, required=True)
    parser.add_argument('--docs-revision', required=True)
    parser.add_argument('--version', help='可选预期版本；省略时采用经过闭包一致性校验的 Cargo 元数据')
    parser.add_argument('--platform', choices=TARGETS, required=True)
    parser.add_argument('--cross-windows', action='store_true')
    args = parser.parse_args()
    try:
        if args.cross_windows and args.platform != 'win-x64':
            raise ValueError('--cross-windows requires win-x64')
        build(args)
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError, tarfile.TarError) as exc:
        print(f'Internal release failed: {exc}', file=sys.stderr)
        sys.exit(1)
