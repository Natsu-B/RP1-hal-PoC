#!/usr/bin/env python3
"""Test only build.rs-pinned newlib memset members on QEMU, never on RP1."""
import hashlib
import os
from pathlib import Path
import re
import shlex
import subprocess
import tempfile

HERE = Path(__file__).resolve().parent
CC = os.environ.get("RP1_FREERTOS_CC", "arm-none-eabi-gcc")
AR = os.environ.get("RP1_FREERTOS_AR", "arm-none-eabi-ar")
FLAGS = ["-mcpu=cortex-m3", "-mthumb", "-mfloat-abi=soft"]


def run(*args, **kwargs):
    return subprocess.run(args, check=True, capture_output=True, timeout=15, **kwargs).stdout


build = (HERE.parent / "build.rs").read_text()
version = re.search(r'const GCC_VERSION: &str = "([^"]+)"', build)[1]
assert run(CC, "-dumpmachine").strip() == b"arm-none-eabi"
assert run(CC, "-dumpfullversion").decode().strip() == version == "14.2.1"
archive = run(CC, *FLAGS, "-print-file-name=libc.a").decode().strip()
members = re.findall(
    r'\(\s*"(memset|aeabi_memset|aeabi_memclr)",\s*"([0-9a-f]{64})",'
    r'\s*"([^"]*)",\s*"([^"]*)"\s*\)', build)
assert {name for name, *_ in members} == {"memset", "aeabi_memset", "aeabi_memclr"}
assert len(members) == 3
assert b"mps2-an385" in run("qemu-system-arm", "-machine", "help")
print(run(CC, "--version").decode().splitlines()[0])
print(run("qemu-system-arm", "--version").decode().splitlines()[0])
print(f"archive={archive}")
with tempfile.TemporaryDirectory(prefix="newlib-memset-") as temporary:
    work = Path(temporary)
    objects = []
    symbols = []
    for name, expected, dependencies, definitions in members:
        member = f"libc_a-{name}.o"
        data = run(AR, "p", archive, member)
        digest = hashlib.sha256(data).hexdigest()
        assert digest == expected, f"changed newlib member: {member}"
        obj = work / member
        obj.write_bytes(data)
        objects.append(str(obj))
        undefined = run("arm-none-eabi-nm", "-u", str(obj)).decode().strip()
        assert undefined == dependencies
        defined = run("arm-none-eabi-nm", "--defined-only", "--extern-only",
                      "--format=posix", str(obj)).decode().splitlines()
        assert " ".join(line.split()[0] for line in defined) == definitions
        assert all(line.split()[1] == "T" for line in defined)
        symbols.extend(definitions.split())
        print(f"{member} sha256={digest} undefined={undefined or 'none'} defined={definitions}")
    linker = work / "test.ld"
    linker.write_text("""ENTRY(reset)
MEMORY { FLASH (rx) : ORIGIN = 0, LENGTH = 4M
         RAM (rwx) : ORIGIN = 0x20000000, LENGTH = 64K }
SECTIONS { .text : { KEEP(*(.vectors)) *(.text*) *(.rodata*) } > FLASH
           .data : { *(.data*) } > RAM
           .bss (NOLOAD) : { *(.bss*) *(COMMON) } > RAM }
""")
    flags = FLAGS + ["-std=c11", "-O2", "-ffreestanding", "-fno-builtin",
                     "-fno-tree-loop-distribute-patterns", "-fno-unwind-tables",
                     "-fno-asynchronous-unwind-tables", "-Wall", "-Wextra", "-Werror"]
    print("compile flags=" + shlex.join(flags))
    for trap in (0, 1):
        test_object, elf = work / "test.o", work / "test.elf"
        run(CC, *flags, f"-DTRAP_MODE={trap}", "-c", str(HERE / "newlib_memset.c"),
            "-o", str(test_object))
        undefined = run("arm-none-eabi-nm", "-u", str(test_object)).decode().split()
        assert undefined == [word for symbol in sorted(symbols) for word in ("U", symbol)]
        run(CC, *FLAGS, "-nostdlib", "-Wl,--build-id=none", f"-Wl,-T,{linker}",
            str(test_object), *objects, "-o", str(elf))
        assert not run("arm-none-eabi-nm", "-u", str(elf)).strip()
        command = ["qemu-system-arm", "-M", "mps2-an385", "-cpu", "cortex-m3", "-nic", "none",
                   "-nographic", "-monitor", "none", "-serial", "none",
                   "-semihosting-config", "enable=on,target=native", "-kernel", str(elf)]
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
        output = result.stdout + result.stderr
        print(f"CCR.UNALIGN_TRP={trap} exit={result.returncode}\n{output}", end="")
        assert result.returncode == 0 and "PASS cases(hex)=00005d70" in output
        assert "TRAP" not in output and "FAIL" not in output
    print("PASS matrix: 23920 cases per trap mode; trap-off and trap-on both passed.")
print("Temporary objects/ELFs removed. Instruction-level model only; no RP1 hardware evidence.")
