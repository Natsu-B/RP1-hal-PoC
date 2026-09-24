#!/usr/bin/env python3
"""Export the final linker reservation, not the historical fb00 address.

Result has RP1-local and BAR2-offset domains only. Resolve the retained/current
DT bus translation independently before producing a deployable shmem node.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("elf", type=Path)
    ap.add_argument("--output", type=Path, required=True)
    a = ap.parse_args()
    symbols = {}
    for line in subprocess.check_output(["arm-none-eabi-nm", "-n", str(a.elf)], text=True).splitlines():
        fields = line.split()
        if len(fields) == 3:
            symbols[fields[2]] = int(fields[0], 16)
    start, end = symbols["__scmi_shmem_start"], symbols["__scmi_shmem_end"]
    if not (0x20000000 <= symbols["__ebss"] <= start < end <= symbols["__image_end"] <= symbols["__app_limit"] <= 0x2000e000):
        raise SystemExit("FAIL: SCMI/image/reserved memory bounds")
    if end - start != 256 or start % 64:
        raise SystemExit("FAIL: SCMI size/alignment")
    section = subprocess.check_output(["arm-none-eabi-readelf", "-SW", str(a.elf)], text=True)
    rows = [line.split() for line in section.splitlines() if ".scmi_shmem" in line]
    if len(rows) != 1 or "NOBITS" not in rows[0]:
        raise SystemExit("FAIL: expected one NOLOAD/NOBITS reservation")
    result = {"classification": "BUILD", "result": "PASS", "elf_sha256": hashlib.sha256(a.elf.read_bytes()).hexdigest(),
              "local_start": start, "local_end": end, "bar2_offset": start - 0x20000000,
              "size": end - start, "image_end": symbols["__image_end"],
              "app_limit": symbols["__app_limit"],
              "remaining": "DT address translation/ELF matching and full IRQ transport not yet admitted"}
    a.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"BUILD layout PASS: local {start:#x}..{end:#x}, BAR2 offset {start - 0x20000000:#x}")


if __name__ == "__main__":
    main()
