#!/usr/bin/env python3
"""Test only build.rs-pinned newlib memcpy members on QEMU, never on RP1."""
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
members = re.findall(r'\(\s*"(memcpy|aeabi_memcpy)",\s*"([0-9a-f]{64})",', build)
assert {name for name, _ in members} == {"memcpy", "aeabi_memcpy"}
assert b"mps2-an385" in run("qemu-system-arm", "-machine", "help")
print(run(CC, "--version").decode().splitlines()[0])
print(run("qemu-system-arm", "--version").decode().splitlines()[0])
print(f"archive={archive}")
with tempfile.TemporaryDirectory(prefix="newlib-memcpy-") as temporary:
    work = Path(temporary)
    objects = []
    for name, expected in members:
        member = f"libc_a-{name}.o"
        data = run(AR, "p", archive, member)
        digest = hashlib.sha256(data).hexdigest()
        assert digest == expected, f"changed newlib member: {member}"
        obj = work / member
        obj.write_bytes(data)
        objects.append(str(obj))
        undefined = run("arm-none-eabi-nm", "-u", str(obj)).decode().strip()
        assert undefined == ("U memcpy" if name == "aeabi_memcpy" else "")
        print(f"{member} sha256={digest} undefined={undefined or 'none'}")
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
        run(CC, *flags, f"-DTRAP_MODE={trap}", "-c", str(HERE / "newlib_memcpy.c"),
            "-o", str(test_object))
        undefined = run("arm-none-eabi-nm", "-u", str(test_object)).decode().split()
        assert undefined == ["U", "__aeabi_memcpy", "U", "__aeabi_memcpy4",
                             "U", "__aeabi_memcpy8", "U", "memcpy"]
        run(CC, *FLAGS, "-nostdlib", "-Wl,--build-id=none", f"-Wl,-T,{linker}",
            str(test_object), *objects, "-o", str(elf))
        assert not run("arm-none-eabi-nm", "-u", str(elf)).strip()
        command = ["qemu-system-arm", "-M", "mps2-an385", "-cpu", "cortex-m3", "-nic", "none",
                   "-nographic", "-monitor", "none", "-serial", "none",
                   "-semihosting-config", "enable=on,target=native", "-kernel", str(elf)]
        result = subprocess.run(command, capture_output=True, text=True, timeout=15)
        output = result.stdout + result.stderr
        print(f"CCR.UNALIGN_TRP={trap} exit={result.returncode}\n{output}", end="")
        if trap == 0:
            assert result.returncode == 0 and "PASS cases(hex)=00004100" in output
        else:
            # Pinned assembly requires unaligned accesses; this is a diagnostic, not a pass.
            assert result.returncode == 2 and "TRAP cfsr(hex)=01000000" in output
            assert "kind,length,src,dst(hex)=00000000 00000002 00000010 00000011" in output
    print("PASS main matrix: 16640 cases; trap-on diagnostic trapped as reported.")
print("Temporary objects/ELFs removed. Instruction-level model only; no RP1 hardware evidence.")
