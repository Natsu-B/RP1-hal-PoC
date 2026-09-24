#!/usr/bin/env python3
"""Compile actual no_std protocol modules as host tests; no MMIO."""
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="rp1-scmi-host-") as tmp:
    test = Path(tmp) / "test.rs"
    test.write_text('''#[path = "%s"] mod clock_profile_generated;
#[path = "%s"] mod scmi_clock;
#[path = "%s"] mod scmi_mailbox;
#[path = "%s"] mod clock_adopt;
''' % (root / "crates/rp1-hal/src/clock_profile_generated.rs",
       root / "crates/rp1-hal/src/scmi_clock.rs",
       root / "crates/rp1-hal/src/scmi_mailbox.rs",
       root / "crates/rp1-hal/src/clock_adopt.rs"))
    binary = Path(tmp) / "test"
    subprocess.run(["rustc", "--edition=2024", "--test", "-D", "warnings", "-A", "dead_code",
                    str(test), "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)
