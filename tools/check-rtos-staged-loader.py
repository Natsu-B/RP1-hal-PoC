#!/usr/bin/env python3
"""Host contract check only; no target build, memory writes or layout admission.

Pass --elf-checkout, --boot-checkout and a new --out directory on tmpfs.
Dependencies must already be cached: this tool never fetches or edits checkouts.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

def command(args, **kwargs):
    return subprocess.check_output(args, stderr=subprocess.STDOUT, timeout=30, **kwargs).decode().strip()

def checked_source(repo, commit, paths):
    assert command(['git', '-C', str(repo), 'rev-parse', 'HEAD']) == commit
    assert not command(['git', '-C', str(repo), 'status', '--porcelain', '--', *paths])
    return {str(p.relative_to(repo)): hashlib.sha256(p.read_bytes()).hexdigest()
        for name in paths for p in ([repo/name] if (repo/name).is_file() else (repo/name).rglob('*'))
        if p.is_file()}

def main():
    if not __debug__:
        raise SystemExit('checks require assertions; refuse -O/PYTHONOPTIMIZE')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--elf-checkout', type=Path, required=True)
    parser.add_argument('--boot-checkout', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--image', type=Path, help='also materialize this RP1 ELF on the host; never deploy')
    args = parser.parse_args()
    elf, boot, out = args.elf_checkout.resolve(), args.boot_checkout.resolve(), args.out.resolve()
    assert not out.exists() and out.parent.is_dir()
    assert os.stat(out.parent).st_dev == os.stat('/dev/shm').st_dev, 'use bounded tmpfs output'
    assert shutil.disk_usage(out.parent).free >= 512*1024**2
    assert shutil.disk_usage(Path(__file__).resolve()).free >= 160*1024**2
    assert command(['rustup', 'run', 'stable', 'rustc', '--version']) == 'rustc 1.96.0 (ac68faa20 2026-05-25)'
    source = Path(__file__).with_name('rtos-staged-loader.rs')
    assembly = source.with_name('rtos-staged-linker.s')
    script = source.with_name('rtos-staged-linker.x')
    sysroot = Path(command(['rustup', 'run', 'stable', 'rustc', '--print', 'sysroot']))
    lld = sysroot/'lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld'
    assert command(['arm-none-eabi-gcc', '-dumpfullversion']) == '14.2.1'
    lld_version = command([str(lld), '-flavor', 'gnu', '--version'])
    assert lld_version.startswith('LLD 22.1.2 ')
    paths = ['elf', 'typestate', 'typestate_macro', 'Cargo.toml', 'Cargo.lock', '.cargo/config.toml']
    elf_hashes = checked_source(elf, '97da4af74e67d9007decadb3682b53a808fff8c4', paths)
    boot_paths = ['rp1_chainboot_poc/src/rp1_image.rs', 'rp1_chainboot_poc/Cargo.toml']
    boot_hashes = checked_source(boot, '6ead136a9721148ec94e5d9cb7f838ea4162fc43', boot_paths)
    out.mkdir()
    env = dict(os.environ, CARGO_TARGET_DIR=str(out/'target'), TMPDIR=str(out),
        CARGO_BUILD_JOBS='1', CARGO_INCREMENTAL='0', RUSTFLAGS='-C debuginfo=0', CARGO_PROFILE_DEV_DEBUG='0',
        CARGO_PROFILE_DEV_PANIC='unwind',  # Host libtest only, not the RP1 target.
        RP1_BOOT_IMAGE_RS=str(boot/'rp1_chainboot_poc/src/rp1_image.rs'),
        RP1_LINKER_FIXTURE=str(out/'linker-fixture.elf'))
    image = args.image.resolve() if args.image else None
    image_hash = hashlib.sha256(image.read_bytes()).hexdigest() if image else None
    if image:
        env['RP1_RUNTIME_ELF'] = str(image)
    record = dict(classification='HOST_CONTRACT_TEST', hardware=False, layout_admitted=False,
        elf_sources=elf_hashes, boot_sources=boot_hashes,
        test_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),
        runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        fixture_sources={p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in [assembly, script]},
        lld_version=lld_version, lld_sha256=hashlib.sha256(lld.read_bytes()).hexdigest(),
        runtime_elf=str(image) if image else None, runtime_elf_sha256=image_hash,
        host_profile=dict(panic='unwind', debuginfo=0, jobs=1, incremental=False), commands=[])
    commands = [['rustup', 'run', 'stable', 'cargo', 'build', '--locked', '--offline',
        '--manifest-path', str(elf/'elf/Cargo.toml'), '-p', 'elf', '--target', 'x86_64-unknown-linux-gnu'],
        ['arm-none-eabi-gcc', '-mcpu=cortex-m3', '-mthumb', '-mfloat-abi=soft', '-c',
            str(assembly), '-o', str(out/'linker-fixture.o')],
        [str(lld), '-flavor', 'gnu', '-m', 'armelf', '-z', 'max-page-size=4',
            '-T', str(script), str(out/'linker-fixture.o'), '-Map='+str(out/'linker-fixture.map'),
            '-o', str(out/'linker-fixture.elf')]]
    deps = out/'target/x86_64-unknown-linux-gnu/debug/deps'
    for phase in range(6 if image else 5):
        if phase == 3:
            record['fixture_elf_sha256'] = hashlib.sha256((out/'linker-fixture.elf').read_bytes()).hexdigest()
            (out/'linker-fixture.readelf.txt').write_text(command(['arm-none-eabi-readelf', '-W', '-l', '-S', '-s', str(out/'linker-fixture.elf')])+'\n')
            libraries = list(deps.glob('libelf-*.rlib'))
            assert len(libraries) == 1
            commands.append(['rustup', 'run', 'stable', 'rustc', '--edition=2024', '--test', str(source),
                '--extern', 'elf='+str(libraries[0]), '-L', 'dependency='+str(deps),
                '-L', 'dependency='+str(out/'target/debug/deps'),
                '-C', 'debuginfo=0', '-C', 'strip=debuginfo', '-o', str(out/'loader-tests')])
        if phase == 4: commands.append([str(out/'loader-tests'), '--nocapture'])
        if phase == 5: commands.append([str(out/'loader-tests'), '--ignored', '--exact', 'target_image_materializes_shared_only', '--nocapture'])
        argv = commands[phase]
        with (out/f'phase-{phase}.txt').open('xb') as log:
            try:
                code = subprocess.run(argv, cwd=elf, env=env, stdout=log, stderr=subprocess.STDOUT, timeout=180).returncode
            except subprocess.TimeoutExpired:
                code = 124
        record['commands'].append(dict(argv=argv, exit=code))
        (out/'result.json').write_text(json.dumps(record, indent=2)+'\n')
        if code: raise SystemExit(code)
    assert checked_source(elf, '97da4af74e67d9007decadb3682b53a808fff8c4', paths) == elf_hashes
    assert checked_source(boot, '6ead136a9721148ec94e5d9cb7f838ea4162fc43', boot_paths) == boot_hashes
    if image:
        assert hashlib.sha256(image.read_bytes()).hexdigest() == image_hash
    record['status'] = 'PASS'
    (out/'result.json').write_text(json.dumps(record, indent=2)+'\n')
    print(json.dumps(dict(result=str(out/'result.json'), status='HOST PASS', hardware=False)))

if __name__ == '__main__': main()
