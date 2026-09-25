#!/usr/bin/env python3
"""Bounded standard arm64 Image build. No source patches, modules or deployment.

First configure; inspect config.diff, then explicitly admit its hash with --image.
Output/records are retained on failure. Caller owns archiving/cleanup of build output.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

from check_linux_config import config

TOOLS = Path(__file__).resolve().parent
GIB = 1024 ** 3


def sha(path):
    with open(path, 'rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()


def capacity(memory, output_free, record_free):
    return memory >= 2 * GIB and output_free >= GIB and record_free >= 1_500_000_000


def resources(out, record):
    info = dict(line.split(':', 1) for line in Path('/proc/meminfo').read_text().splitlines())
    return [int(info['MemAvailable'].split()[0]) * 1024,
            shutil.disk_usage(out).free, shutil.disk_usage(record).free]


def run(cmd, out, record, env, name, seconds):
    if not capacity(*resources(out, record)):
        raise RuntimeError('resource admission refused')
    start = time.monotonic()
    reason = None
    with (record/(name+'.log')).open('xb') as log, (record/(name+'-resources.jsonl')).open('x') as samples:
        child = subprocess.Popen(cmd, stdout=log, stderr=subprocess.STDOUT,
                                 env=env, start_new_session=True)
        try:
            while child.poll() is None:
                elapsed = time.monotonic() - start
                sample = resources(out, record)
                samples.write(json.dumps([round(elapsed, 3), *sample])+'\n')
                samples.flush()
                if not capacity(*sample):
                    reason = 'resource floor'
                    break
                if elapsed > seconds:
                    reason = 'time limit'
                    break
                try:
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    pass
        finally:
            # Kill the entire dedicated build group on interruption or guard exit;
            # don't leave compiler children consuming RAM after parent make exits.
            if child.poll() is None or reason:
                try:
                    os.killpg(child.pid, signal.SIGTERM)
                    child.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    pass
                except ProcessLookupError:
                    pass
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait()
    result = dict(command=cmd, exit=child.returncode, guard=reason,
                  seconds=round(time.monotonic()-start, 3))
    (record/(name+'.json')).write_text(json.dumps(result, indent=2)+'\n')
    if reason or child.returncode:
        raise RuntimeError(f'{name} failed: {result}')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source', type=Path, required=True)
    p.add_argument('--config', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--record', type=Path, required=True)
    p.add_argument('--cc', default='/usr/bin/aarch64-linux-gnu-gcc-14')
    p.add_argument('--cross-compile', default='/usr/bin/aarch64-linux-gnu-')
    p.add_argument('--image', metavar='REVIEWED_CONFIG_SHA256')
    p.add_argument('--jobs', type=int, choices=(1, 2), default=2)
    args = p.parse_args()
    source, out, record = (x.resolve() for x in (args.source, args.out, args.record))
    if source == out or source in out.parents or out in source.parents:
        raise ValueError('use an independent out-of-tree build directory')
    out.mkdir(parents=True, exist_ok=bool(args.image))
    record.mkdir(parents=True, exist_ok=bool(args.image))
    (out/'tmp').mkdir(exist_ok=True)
    env = dict(os.environ, TMPDIR=str(out/'tmp'), LC_ALL='C', SOURCE_DATE_EPOCH='1773244446',
               KBUILD_BUILD_TIMESTAMP='@1773244446', KBUILD_BUILD_VERSION='1',
               KBUILD_BUILD_USER='cm5', KBUILD_BUILD_HOST='builder')
    # Ignore caller toolchain/config overrides; only the recorded explicit inputs apply.
    for key in ('KCONFIG_CONFIG', 'KCONFIG_ALLCONFIG', 'KBUILD_OUTPUT', 'KBUILD_SRC',
                'KBUILD_EXTMOD', 'KBUILD_KCONFIG', 'MAKEFLAGS', 'MFLAGS', 'KCFLAGS',
                'KAFLAGS', 'KCPPFLAGS', 'LLVM', 'LLVM_IAS', 'CFLAGS_KERNEL', 'LDFLAGS_vmlinux'):
        env.pop(key, None)
    make = ['/usr/bin/make', '-C', str(source), 'O='+str(out), 'ARCH=arm64',
            'CROSS_COMPILE='+args.cross_compile, 'CC='+args.cc,
            'HOSTCC=/usr/bin/gcc', 'HOSTCXX=/usr/bin/g++']
    identity = dict(source=str(source), makefile_sha256=sha(source/'Makefile'),
                    base_config_sha256=sha(args.config), compiler=args.cc,
                    compiler_sha256=sha(args.cc), cross_compile=args.cross_compile,
                    fragment_sha256=sha(TOOLS.parent/'profiles/linux-standard.config'))
    if not args.image:
        shutil.copyfile(args.config, out/'.config')
        run(make+['olddefconfig'], out, record, env, 'configure', 180)
        diff = subprocess.run([str(source/'scripts/diffconfig'), str(args.config), str(out/'.config')],
                              capture_output=True, check=True)
        (record/'config.diff').write_bytes(diff.stdout)
        shutil.copyfile(out/'.config', record/'resolved.config')
        identity['resolved_config_sha256'] = sha(out/'.config')
        (record/'identity.json').write_text(json.dumps(identity, indent=2)+'\n')
        print('CONFIG ONLY; review config.diff and admit '+identity['resolved_config_sha256'])
    else:
        prior = json.loads((record/'identity.json').read_text())
        if any(prior.get(k) != v for k, v in identity.items()):
            raise ValueError('source/config/tool identity changed')
        if args.image != sha(out/'.config') or args.image != prior['resolved_config_sha256']:
            raise ValueError('reviewed config hash mismatch')
        actual = config(out/'.config')
        requested = config(TOOLS.parent/'profiles/linux-standard.config')
        if any(actual.get(k) != v for k, v in requested.items()):
            raise ValueError('standard config fragment mismatch')
        run(make+['-j'+str(args.jobs), 'Image'], out, record, env, 'image', 7200)
        if sha(out/'.config') != args.image:
            raise ValueError('build changed admitted config')
        products = ['arch/arm64/boot/Image', 'System.map', 'vmlinux.symvers']
        hashes = {f:sha(out/f) for f in products}
        (record/'products.json').write_text(json.dumps(hashes, indent=2)+'\n')
        print('BUILD Image PASS; no modules/deployment/boot proof; '+hashes[products[0]])


if __name__ == '__main__':
    main()
