#!/usr/bin/env python3
"""Validate a supplied final DTB, never a hard-coded historical snapshot.

Uses dtc's own decoder (DTB -> DTS -> YAML) and PyYAML. A structural PASS is
not hardware/build admission: physical rates, SRAM/ELF placement, camera wiring
and the deployed kernel must also be verified independently.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import yaml
from clock_profile import DEFAULT, load


class DtcLoader(yaml.SafeLoader):
    pass


for tag in ("!u8", "!u16", "!u32", "!u64"):
    DtcLoader.add_constructor(tag, lambda loader, node: loader.construct_sequence(node))


def decode(path):
    dts = subprocess.run(["dtc", "-I", "dtb", "-O", "dts", str(path)],
                         capture_output=True, check=True).stdout
    result = subprocess.run(["dtc", "-I", "dts", "-O", "yaml", "-"], input=dts,
                            capture_output=True, check=True)
    roots = yaml.load(result.stdout, Loader=DtcLoader)
    if len(roots) != 1 or not isinstance(roots[0], dict):
        raise ValueError("expected one DT root")
    return roots[0]


def cells(value):
    if value is None:
        return []
    if not isinstance(value, list):
        raise ValueError("expected cells")
    out = []
    for part in value:
        out.extend(cells(part) if isinstance(part, list) else [part])
    if not all(type(x) is int and 0 <= x < 2**32 for x in out):
        raise ValueError("invalid u32 cells")
    return out


def strings(value):
    return value if isinstance(value, list) and all(isinstance(x, str) for x in value) else []


def scalar(node, key, default=None):
    if key not in node:
        return default
    seq = cells(node[key])
    if len(seq) != 1:
        raise ValueError("expected one cell: " + key)
    return seq[0]


def number(seq):
    result = 0
    for n in seq:
        result = (result << 32) | n
    return result


class Tree:
    def __init__(self, root):
        self.nodes, self.parents, self.enabled, self.phandles = {}, {}, {}, {}
        self.errors = []
        def visit(n, path, parent, on):
            status = strings(n.get("status", ["okay"]))
            on = on and status in (["okay"], ["ok"])
            self.nodes[path], self.parents[path], self.enabled[path] = n, parent, on
            for prop in ("phandle", "linux,phandle"):
                if prop in n:
                    ph = scalar(n, prop)
                    if not ph or ph == 0xffffffff or self.phandles.get(ph, path) != path:
                        self.errors.append(f"{path}: invalid/duplicate phandle")
                    self.phandles[ph] = path
            for key, value in n.items():
                if isinstance(value, dict):
                    visit(value, path.rstrip("/") + "/" + key, path, on)
        visit(root, "/", None, True)

    def rp1(self, path):
        while path:
            if path.rsplit("/", 1)[-1].split("@")[0] == "rp1":
                return True
            if "raspberrypi,rp1" in strings(self.nodes[path].get("compatible")):
                return True
            path = self.parents[path]
        return False

    def refs(self, path, prop, count_prop, zero=False):
        seq, i, out = cells(self.nodes[path].get(prop)), 0, []
        while i < len(seq):
            ph = seq[i]
            i += 1
            if ph == 0 and zero:
                out.append((None, []))
                continue
            if ph not in self.phandles:
                raise ValueError(f"{path}: unresolved {prop} phandle {ph}")
            target = self.phandles[ph]
            count = scalar(self.nodes[target], count_prop)
            if count is None or count > 16 or i + count > len(seq):
                raise ValueError(f"{path}: invalid {count_prop} at {target}")
            out.append((target, seq[i:i + count]))
            i += count
        return out

    def regs(self, path):
        parent = self.nodes[self.parents[path]]
        ac, sc = scalar(parent, "#address-cells", 2), scalar(parent, "#size-cells", 1)
        if not 1 <= ac <= 3 or not 1 <= sc <= 2:
            raise ValueError(f"{path}: unsupported region cell widths {ac}/{sc}")
        seq = cells(self.nodes[path].get("reg"))
        if not seq or len(seq) % (ac + sc):
            raise ValueError(f"{path}: invalid region reg")
        return [(number(seq[i:i + ac]), number(seq[i + ac:i + ac + sc]))
                for i in range(0, len(seq), ac + sc)]

    def physical(self, bus, address, size):
        # Generic ranges translation, including the PCI high/space cells.
        # Missing ranges is NOT silently treated as an identity mapping.
        while bus != "/":
            n = self.nodes[bus]
            parent = self.parents[bus]
            if "ranges" not in n:
                raise ValueError(f"{bus}: missing address translation")
            if n["ranges"] is not True:
                seq = cells(n["ranges"])
                ac = scalar(n, "#address-cells", 2)
                pc = scalar(self.nodes[parent], "#address-cells", 2)
                sc = scalar(n, "#size-cells", 1)
                width = ac + pc + sc
                if not width or len(seq) % width:
                    raise ValueError(f"{bus}: malformed ranges")
                found = []
                for i in range(0, len(seq), width):
                    child = number(seq[i:i + ac])
                    cpu = number(seq[i + ac:i + ac + pc])
                    length = number(seq[i + ac + pc:i + width])
                    if child <= address and address + size <= child + length:
                        found.append(cpu + address - child)
                if len(found) != 1:
                    raise ValueError(f"{bus}: region has no unique containing range")
                address = found[0]
            bus = parent
        return address


def validate(root, profile, require_camera=False):
    t = Tree(root)
    failures = list(t.errors)
    refs, disabled, pin_users, regions = [], [], {}, []
    pin_functions = {}
    scmi_ids = {c["scmi_id"] for c in profile["clock"] if c["mode"] == "scmi"}
    fixed = {"rp1-profile-" + c["name"]: c for c in profile["clock"] if c["mode"] == "fixed"}
    fixed_paths = {}
    protocol_paths = []
    mailbox_users = {}
    uart_paths, camera_paths = [], []

    def fail(text):
        failures.append(text)

    for path, n in t.nodes.items():
        on = t.enabled[path]
        compat = strings(n.get("compatible"))
        leaf = path.rsplit("/", 1)[-1]
        if not on:
            disabled.append(path)
        if on and "raspberrypi,rp1-clocks" in compat:
            fail(path + ": direct clk-rp1 provider enabled")
        if t.rp1(path) and on:
            if leaf in profile["firmware_nodes"]:
                fail(path + ": firmware-owned peripheral enabled in Linux")
            if leaf == "serial@34000":
                uart_paths.append(path)
            if "raspberrypi,rp1-cfe" in compat:
                camera_paths.append(path)
        for name in strings(n.get("clock-output-names")):
            if name in fixed:
                c = fixed[name]
                if (not on or compat != ["fixed-clock"] or
                        scalar(n, "#clock-cells") != 0 or scalar(n, "clock-frequency") != c["rate_hz"]):
                    fail(path + ": fixed clock/profile mismatch")
                if c["name"] in fixed_paths:
                    fail(path + ": duplicate profile clock")
                fixed_paths[c["name"]] = path
        if on and leaf == "protocol@14" and "#clock-cells" in n:
            parent = t.parents[path]
            if "arm,scmi" in strings(t.nodes[parent].get("compatible")):
                protocol_paths.append(path)
                if scalar(n, "reg") != 0x14 or scalar(n, "#clock-cells") != 1:
                    fail(path + ": invalid SCMI clock protocol")
        for prop in ("clocks", "assigned-clocks", "assigned-clock-parents", "resets", "mboxes"):
            kind = "clock" if "clock" in prop else "reset" if prop == "resets" else "mbox"
            try:
                parsed = t.refs(path, prop, "#" + kind + "-cells", zero=prop.startswith("assigned-"))
                for target, args in parsed:
                    if target is None:
                        continue
                    refs.append({"consumer": path, "enabled": on, "property": prop,
                                 "provider": target, "args": args})
                    if on and prop == "mboxes":
                        key = (target, tuple(args))
                        if key in mailbox_users and mailbox_users[key] != path:
                            fail(f"{path}: mailbox channel also owned by {mailbox_users[key]}")
                        mailbox_users[key] = path
                    if on and not t.enabled[target]:
                        fail(f"{path}: disabled {prop} provider {target}")
                    if on and "raspberrypi,rp1-clocks" in strings(t.nodes[target].get("compatible")):
                        fail(f"{path}: {prop} still references clk-rp1")
                    if on and target.endswith("/protocol@14"):
                        if len(args) != 1 or args[0] not in scmi_ids:
                            fail(f"{path}: SCMI clock ID mismatch")
                if on and prop == "assigned-clocks":
                    rates = cells(n.get("assigned-clock-rates"))
                    if len(rates) > len(parsed):
                        fail(path + ": excess assigned clock rates")
                    for (target, args), rate in zip(parsed, rates):
                        if target and rate and "fixed-clock" in strings(t.nodes[target].get("compatible")):
                            if scalar(t.nodes[target], "clock-frequency") != rate:
                                fail(path + ": assigned fixed-clock rate mismatch")
                        if target and rate and target.endswith("/protocol@14") and len(args) == 1:
                            allowed = next((c["allowed_rates"] for c in profile["clock"]
                                            if c.get("scmi_id") == args[0]), [])
                            if rate not in allowed:
                                fail(path + ": assigned SCMI rate outside profile")
            except ValueError as exc:
                # Malformed disabled references are recorded too, not silently ignored.
                if on:
                    fail(str(exc))
                else:
                    refs.append({"consumer": path, "enabled": False, "property": prop, "error": str(exc)})
        if on:
            used = set()
            functions = set()
            for prop in n:
                if re.fullmatch(r"pinctrl-\d+", prop):
                    for ph in cells(n[prop]):
                        target = t.phandles.get(ph)
                        if target is None:
                            fail(path + ": unresolved pinctrl")
                            continue
                        if not t.rp1(target):
                            continue
                        for p, group in t.nodes.items():
                            if p == target or p.startswith(target + "/"):
                                for pin in strings(group.get("pins")):
                                    match = re.fullmatch(r"gpio(\d+)", pin)
                                    if match:
                                        used.add(int(match[1]))
                                    else:
                                        fail(p + ": unrecognized RP1 pin name")
                                used.update(cells(group.get("brcm,pins")))
                                if "pins" in group or "brcm,pins" in group:
                                    fn = strings(group.get("function"))
                                    functions.update(fn or ["UNKNOWN"])
            pin_functions[path] = sorted(functions)
            # GPIO consumers are owners too, even without a pinctrl state.
            # A hog encodes parent-controller specifiers WITHOUT a phandle.
            if "gpio-hog" in n:
                parent = t.parents[path]
                if t.rp1(parent):
                    width = scalar(t.nodes[parent], "#gpio-cells")
                    spec = cells(n.get("gpios"))
                    if width is None or width < 1 or width > 16 or not spec or len(spec) % width:
                        fail(path + ": invalid RP1 GPIO hog")
                    else:
                        used.update(spec[::width])
            else:
                for prop in n:
                    if prop == "gpios" or prop.endswith(("-gpios", "-gpio")):
                        try:
                            for provider, args in t.refs(path, prop, "#gpio-cells", zero=True):
                                if provider is not None and t.rp1(provider):
                                    if not t.enabled[provider] or not args:
                                        fail(path + ": unavailable RP1 GPIO provider")
                                    else:
                                        used.add(args[0])
                        except ValueError as exc:
                            fail(str(exc))
            if used:
                pin_users[path] = sorted(used)
                overlap = used.intersection(profile["firmware_pins"])
                if overlap:
                    fail(f"{path}: uses firmware pins {sorted(overlap)}")
            for prop in ("memory-region", "shmem", "remote-endpoint"):
                for ph in cells(n.get(prop)):
                    target = t.phandles.get(ph)
                    if target is None or not t.enabled[target]:
                        fail(f"{path}: unresolved/disabled {prop}")
                    elif prop == "remote-endpoint":
                        back = cells(t.nodes[target].get(prop))
                        own = scalar(n, "phandle", scalar(n, "linux,phandle"))
                        if back != [own]:
                            fail(path + ": camera graph link is not reciprocal")
        parent = t.parents[path]
        is_region = (parent == "/reserved-memory" or "arm,scmi-shmem" in compat or
                     (parent and "mmio-sram" in strings(t.nodes[parent].get("compatible"))))
        if on and is_region:
            try:
                for start, size in t.regs(path):
                    if not size:
                        raise ValueError(path + ": zero-sized shared/reserved region")
                    cpu = t.physical(parent, start, size)
                    if cpu + size > 2**64:
                        raise ValueError(path + ": region overflow")
                    regions.append({"node": path, "cpu_start": cpu, "size": size})
            except ValueError as exc:
                fail(str(exc))
    for i, a in enumerate(regions):
        for b in regions[i + 1:]:
            if a["cpu_start"] < b["cpu_start"] + b["size"] and b["cpu_start"] < a["cpu_start"] + a["size"]:
                fail(f'shared/reserved overlap: {a["node"]} and {b["node"]}')
    owners = list(pin_users.items())
    for i, (a, pins) in enumerate(owners):
        for b, other in owners[i + 1:]:
            overlap = set(pins).intersection(other)
            if overlap:
                fail(f"pin ownership overlap {sorted(overlap)}: {a} and {b}")
    marker = t.nodes.get("/rp1_clock_profile", {})
    if strings(marker.get("profile-sha256")) != [profile["sha256"]]:
        fail("DT profile SHA mismatch/missing")
    for c in profile["clock"]:
        if c["mode"] == "fixed" and c["name"] not in fixed_paths:
            fail("missing fixed clock: " + c["name"])
    if len(protocol_paths) != 1:
        fail("exactly one enabled profile SCMI clock protocol required")
    else:
        scmi = t.parents[protocol_paths[0]]
        try:
            channels = t.refs(scmi, "mboxes", "#mbox-cells")
            if len(channels) != 1 or channels[0][1] != [profile["mailbox_channel"]]:
                fail(scmi + ": profile requires one bidirectional mailbox channel")
            elif "raspberrypi,rp1-mbox" not in strings(t.nodes[channels[0][0]].get("compatible")):
                fail(scmi + ": SCMI mailbox is not RP1")
            shmem = cells(t.nodes[scmi].get("shmem"))
            if len(shmem) != 1 or shmem[0] not in t.phandles:
                fail(scmi + ": expected one shmem")
            else:
                shared = t.phandles[shmem[0]]
                if "arm,scmi-shmem" not in strings(t.nodes[shared].get("compatible")):
                    fail(shared + ": not SCMI shared memory")
                allocations = [r for r in regions if r["node"] == shared]
                if len(allocations) != 1 or allocations[0]["size"] != 256 or allocations[0]["cpu_start"] % 4:
                    fail(shared + ": expected aligned 256-byte SCMI region")
        except ValueError as exc:
            fail(str(exc))
    if len(uart_paths) != 1:
        fail("exactly one enabled RP1 UART1 required")
    else:
        path = uart_paths[0]
        n = t.nodes[path]
        if "arm,pl011-axi" not in strings(n.get("compatible")):
            fail(path + ": requires reviewed PL011 AXI driver")
        if pin_users.get(path) != [0, 1] or "uart-has-rtscts" in n:
            fail(path + ": UART1 must use TX/RX GPIO0/1 only, no CTS/RTS")
        if pin_functions.get(path) != ["uart1"]:
            fail(path + ": UART1 pin function must be uart1")
        try:
            clocks = t.refs(path, "clocks", "#clock-cells")
            if not clocks or clocks[0] != (fixed_paths.get("uart"), []):
                fail(path + ": functional clock must be profile fixed UART clock")
        except ValueError as exc:
            fail(str(exc))
    if require_camera and not camera_paths:
        fail("camera required but no enabled RP1 CFE")
    for path in camera_paths:
        leaf = path.rsplit("/", 1)[-1]
        cfg = {"csi@110000": "mipi0_cfg", "csi@128000": "mipi1_cfg"}.get(leaf)
        try:
            clocks = t.refs(path, "clocks", "#clock-cells")
            if not cfg or not clocks or clocks[0] != (fixed_paths.get(cfg), []):
                fail(path + ": CFE CFG clock must match selected fixed profile")
        except ValueError as exc:
            fail(str(exc))
        if not any(p.startswith(path + "/") and t.enabled[p] and "remote-endpoint" in n
                   for p, n in t.nodes.items()):
            fail(path + ": enabled CFE has no sensor graph")
    return {"classification": "STATIC", "result": "FAIL" if failures else "PASS",
            "scope": "candidate DTB structure only; not deployment admission or HW proof",
            "profile_sha256": profile["sha256"], "failures": sorted(set(failures)),
            "enabled_nodes": [p for p in t.nodes if t.enabled[p]], "disabled_nodes": disabled,
            "references": refs, "pin_owners": pin_users, "regions": regions,
            "camera_nodes": camera_paths,
            "remaining_admission": ["live kernel/config identity", "firmware ELF SRAM placement/guards",
                                    "physical clock readback", "camera wiring", "full mailbox IRQ roundtrip"]}


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("dtb", type=Path)
    ap.add_argument("--profile", type=Path, default=DEFAULT)
    ap.add_argument("--require-camera", action="store_true")
    ap.add_argument("--output", type=Path)
    a = ap.parse_args()
    try:
        result = validate(decode(a.dtb), load(a.profile), a.require_camera)
    except (ValueError, KeyError, TypeError, subprocess.CalledProcessError, yaml.YAMLError) as exc:
        result = {"classification": "STATIC", "result": "FAIL", "failures": [str(exc)]}
    result["dtb_sha256"] = hashlib.sha256(a.dtb.read_bytes()).hexdigest()
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if a.output:
        a.output.write_text(text)
        print(result["result"] + ": " + str(len(result.get("failures", []))) + " failures; " + str(a.output))
    else:
        print(text, end="")
    raise SystemExit(0 if result["result"] == "PASS" else 1)


if __name__ == "__main__":
    main()
