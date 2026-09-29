#!/usr/bin/env python3
"""Build/audit pinned upstream archives; not an RP1 platform qualification.

No upstream edits, downloads, hardware access, or firmware deployment.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


def output(*args):
    return subprocess.check_output(args, text=True, stderr=subprocess.STDOUT)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path, help="new absolute build directory")
    parser.add_argument("--rp1", action="store_true", help="use single-owner mutex and ordering/cache hooks")
    parser.add_argument("--lock", type=Path, default=Path(os.environ.get(
        "CM5_HACK_ROOT", "/opt/rpi-cm5-hack")) / "tools/openamp-lock.json")
    args = parser.parse_args()
    if not args.out.is_absolute() or args.out.exists():
        parser.error("out must be a new absolute directory")
    lock = json.loads(args.lock.read_text())
    if lock["schema"] != 1:
        parser.error("unsupported lock schema")
    for name in ("libmetal", "openamp"):
        item = lock[name]
        source = Path(item["checkout"])
        head = output("git", "-C", str(source), "rev-parse", "HEAD").strip()
        if head != item["commit"]:
            parser.error(f"{name}: source HEAD does not match lock")
        if output("git", "-C", str(source), "status", "--porcelain", "--untracked-files=all").strip():
            parser.error(f"{name}: checkout is not clean")
    repo = Path(__file__).resolve().parents[1]
    toolchain = repo / "openamp/cortex-m3.cmake"
    args.out.mkdir(parents=True)
    prefix = args.out / "install"
    (args.out / "input-lock.json").write_bytes(args.lock.read_bytes())
    (args.out / "toolchain.cmake").write_bytes(toolchain.read_bytes())
    configured = {}
    with (args.out / "build.txt").open("w") as log:
        def run(command):
            log.write(json.dumps(command) + "\n")
            log.flush()
            subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)

        for name in ("libmetal", "openamp"):
            item = lock[name]
            build = args.out / name
            options = dict(item["build_options"])
            # Keep upstream assertions: MinSizeRel normally defines NDEBUG.
            options.update(CMAKE_BUILD_TYPE="MinSizeRel", MACHINE="template",
                           CMAKE_C_FLAGS_MINSIZEREL="-Os",
                           CMAKE_EXPORT_COMPILE_COMMANDS=True)
            if name == "openamp":
                options.update(LIBMETAL_INCLUDE_DIR=str(prefix / "include"),
                               LIBMETAL_LIB=str(prefix / "lib/libmetal.a"))
                if args.rp1:
                    shutil.copyfile(repo/'openamp/metal-mutex.h', prefix/'include/metal/system/generic/mutex.h')
                    shutil.copyfile(repo/'openamp/metal-sleep.h', prefix/'include/metal/system/generic/sleep.h')
                    options['WITH_DCACHE'] = True
            configured[name] = options
            run(["cmake", "-S", item["checkout"], "-B", str(build), "-G", "Ninja",
                 f"-DCMAKE_TOOLCHAIN_FILE={toolchain}",
                 f"-DCMAKE_INSTALL_PREFIX={prefix}",
                 *[f"-D{k}={'ON' if v is True else 'OFF' if v is False else v}"
                   for k, v in options.items()]])
            run(["cmake", "--build", str(build), "--parallel", "2"])
            run(["cmake", "--install", str(build)])
            commands = json.loads((build / "compile_commands.json").read_text())
            if any("-DNDEBUG" in entry["command"] for entry in commands):
                raise RuntimeError(f"{name}: assertions must stay enabled")
    report = {
        "status": "ARCHIVES_BUILT_PLATFORM_NOT_QUALIFIED",
        "scope": "Upstream Generic/template build only; not linked into RP1 firmware",
        "compiler": output("arm-none-eabi-gcc", "--version").splitlines()[0],
        "lock_sha256": sha(args.lock), "toolchain_sha256": sha(toolchain),
        "builder_sha256": sha(Path(__file__)),
        "source_commits": {n: lock[n]["commit"] for n in ("libmetal", "openamp")},
        "effective_build_options": configured,
        "rp1_port": args.rp1,
        "mutex_header_sha256": sha(repo/'openamp/metal-mutex.h') if args.rp1 else None,
        "archives": {},
        "required_before_firmware_link": [
            "Replace Generic/template no-op IRQ/cache/sleep/time hooks with qualified RP1 hooks",
            "Single OpenAMP task ownership; no shared-memory exclusive locks",
            "Bound all mappings to verified Linux queues and buffers",
            "Preserve DDR posted-write completion at vring publication/notification",
            "Resolve synchronous VirtIO register service before queue admission",
        ],
    }
    for name in ("metal", "open_amp"):
        archive = prefix / f"lib/lib{name}.a"
        asm = output("arm-none-eabi-objdump", "-d", str(archive))
        (args.out / f"{name}.disasm").write_text(asm)
        attrs = output("arm-none-eabi-readelf", "-A", str(archive))
        (args.out / f"{name}.attributes.txt").write_text(attrs)
        if "Tag_CPU_arch: v7" not in attrs or "Tag_CPU_arch_profile: Microcontroller" not in attrs:
            raise RuntimeError(f"{name}: not Cortex-M3 architecture")
        undefined = output("arm-none-eabi-nm", "-u", str(archive))
        (args.out / f"{name}.undefined.txt").write_text(undefined)
        current = ""
        exclusive = set()
        for line in asm.splitlines():
            match = re.match(r"[0-9a-f]+ <([^>]+)>:", line)
            if match:
                current = match[1]
            if re.search(r"\b(?:ldrex|strex)[bh]?\b", line):
                exclusive.add(current)
        report["archives"][name] = {
            "path": str(archive), "sha256": sha(archive),
            "exclusive_instruction_functions": sorted(exclusive),
            "atomic_helpers": sorted(set(re.findall(r"\b(__atomic_\w+)", undefined))),
        }
    (args.out / "audit.json").write_text(json.dumps(report, indent=2) + "\n")
    print(args.out / "audit.json")


if __name__ == "__main__":
    main()
