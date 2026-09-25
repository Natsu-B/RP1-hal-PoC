#!/usr/bin/env python3
"""Export the final linker reservation, not the historical fb00 address.

Optionally derive a transport fragment from an explicit input DTB. This checks
address/layout consistency only, not live PCI enumeration or IRQ delivery.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

# RP1 SRAM identity; Linux CPU/PCI addresses are derived from the supplied DT.
LOCAL_BASE = 0x20000000
SYSTEM_BASE = 0xc040400000
SRAM_SIZE = 0x10000


def read_layout(elf):
    elf = Path(elf)
    raw = elf.read_bytes()
    if raw[:7] != b'\x7fELF\x01\x01\x01' or raw[18:20] != b'\x28\x00':
        raise ValueError("expected little-endian ARM ELF32")
    symbols = {}
    for line in subprocess.check_output(["arm-none-eabi-nm", "-n", str(elf)], text=True).splitlines():
        fields = line.split()
        if len(fields) == 3:
            symbols[fields[2]] = int(fields[0], 16)
    start, end = symbols["__scmi_shmem_start"], symbols["__scmi_shmem_end"]
    if not (LOCAL_BASE <= symbols["__ebss"] <= start < end <= symbols["__image_end"] <= symbols["__app_limit"] <= 0x2000e000):
        raise ValueError("SCMI/image/reserved memory bounds")
    if end - start != 256 or start % 64:
        raise ValueError("SCMI size/alignment")
    section = subprocess.check_output(["arm-none-eabi-readelf", "-SW", str(elf)], text=True)
    rows = [m.group(1).split() for line in section.splitlines()
            if (m := re.match(r"\s*\[\s*\d+\]\s+(\S+\s+.*)", line))]
    shared = [r for r in rows if r[0] == ".scmi_shmem"]
    if (len(shared) != 1 or shared[0][1] != "NOBITS" or
            int(shared[0][2], 16) != start or int(shared[0][4], 16) != end-start or
            "A" not in shared[0][6] or "W" not in shared[0][6]):
        raise ValueError("expected matching allocated/writable NOLOAD SCMI section")
    for r in rows:
        if len(r) < 7 or "A" not in r[6] or r[0] == ".scmi_shmem":
            continue
        address, size = int(r[2], 16), int(r[4], 16)
        if size and address < end and start < address + size:
            raise ValueError("SCMI overlaps allocated section " + r[0])
    profiles = [r for r in rows if r[0] == ".rp1_clock_profile"]
    if len(profiles) != 1 or profiles[0][1] != "PROGBITS" or "A" not in profiles[0][6]:
        raise ValueError("expected one linked profile fingerprint")
    p = profiles[0]
    position, size = int(p[3], 16), int(p[4], 16)
    digest = raw[position:position + size]
    if size != 64 or not re.fullmatch(b"[0-9a-f]{64}", digest):
        raise ValueError("invalid linked profile fingerprint")
    return {"classification": "BUILD", "result": "PASS", "elf_sha256": hashlib.sha256(raw).hexdigest(),
              "profile_sha256": digest.decode(),
              "local_start": start, "local_end": end, "bar2_offset": start - LOCAL_BASE,
              "size": end - start, "image_end": symbols["__image_end"],
              "app_limit": symbols["__app_limit"],
              "remaining": "Final DT matching, physical mapping and full IRQ transport need separate validation"}


def sram_mapping(tree):
    matches = [p for p, n in tree.nodes.items() if tree.enabled[p] and tree.rp1(p)
               and "mmio-sram" in n.get("compatible", [])]
    if len(matches) != 1:
        raise ValueError("expected exactly one enabled RP1 SRAM")
    path = matches[0]
    if any(tree.nodes[path].get(k) != [[1]] for k in ('#address-cells', '#size-cells')):
        raise ValueError("SCMI generator requires SRAM child address/size cells 1/1")
    if tree.regs(path) != [(SYSTEM_BASE, SRAM_SIZE)]:
        raise ValueError("RP1 SRAM reg must identify system 0xc040400000/64KiB")
    cpu = tree.physical(tree.parents[path], SYSTEM_BASE, SRAM_SIZE)
    if tree.physical(path, 0, SRAM_SIZE) != cpu:
        raise ValueError("SRAM child ranges disagree with parent reg")
    return {"node": path, "system_start": SYSTEM_BASE, "cpu_start": cpu, "size": SRAM_SIZE}


def transport_dtsi(layout, tree, profile):
    if layout['profile_sha256'] != profile['sha256']:
        raise ValueError("linked ELF profile SHA mismatch")
    mapping = sram_mapping(tree)
    mailboxes = [p for p, n in tree.nodes.items() if tree.rp1(p)
                 and "raspberrypi,rp1-mbox" in n.get("compatible", [])]
    if len(mailboxes) != 1:
        raise ValueError("expected one RP1 mailbox controller")
    if "/firmware/scmi" in tree.nodes or "/rp1_firmware_layout" in tree.nodes:
        raise ValueError("input already has SCMI/layout nodes; use the unmodified base DTB")
    offset = layout['bar2_offset']
    return f'''/* Generated from ELF {layout['elf_sha256']}; not HW admission. */
&{{{mapping['node']}}} {{
    rp1_scmi_shmem: scmi@{offset:x} {{
        compatible = "arm,scmi-shmem";
        reg = <0x{offset:x} 0x{layout['size']:x}>;
    }};
}};
&{{{mailboxes[0]}}} {{ status = "okay"; }};
/ {{
    rp1_firmware_layout {{ elf-sha256 = "{layout['elf_sha256']}"; }};
    firmware {{
        rp1_scmi: scmi {{
            compatible = "arm,scmi";
            #address-cells = <1>;
            #size-cells = <0>;
            mboxes = <&{{{mailboxes[0]}}} {profile['mailbox_channel']}>;
            shmem = <&rp1_scmi_shmem>;
        }};
    }};
}};
'''


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("elf", type=Path)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--dtb", type=Path, help="unmodified base DTB for transport placement")
    ap.add_argument("--dtsi", type=Path, help="generated transport fragment (requires --dtb)")
    ap.add_argument("--profile", type=Path)
    a = ap.parse_args()
    if bool(a.dtb) != bool(a.dtsi):
        ap.error("--dtb and --dtsi are required together")
    try:
        result = read_layout(a.elf)
        if a.dtb:
            from clock_profile import DEFAULT, load
            from validate_linux_dtb import Tree, decode
            tree = Tree(decode(a.dtb))
            if tree.errors:
                raise ValueError("; ".join(tree.errors))
            profile = load(a.profile or DEFAULT)
            fragment = transport_dtsi(result, tree, profile)
            result.update(sram=sram_mapping(tree), input_dtb_sha256=hashlib.sha256(a.dtb.read_bytes()).hexdigest())
            a.dtsi.write_text(fragment)
    except (ValueError, KeyError, subprocess.CalledProcessError) as exc:
        raise SystemExit("FAIL: " + str(exc)) from exc
    a.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"BUILD layout PASS: local {result['local_start']:#x}..{result['local_end']:#x}, BAR2 offset {result['bar2_offset']:#x}")


if __name__ == "__main__":
    main()
