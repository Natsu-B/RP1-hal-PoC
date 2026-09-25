#!/usr/bin/env python3
"""Compile actual no_std protocol modules as host tests; no MMIO."""
from pathlib import Path
import subprocess
import tempfile
import tomllib

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

    # Exercise the real cold prerequisite body, including no-write rejection.
    subprocess.run(["rustc", "--edition=2024", "--test", "-D", "warnings",
                    str(root / "examples/minimal/src/scmi_cold_clock.rs"),
                    "-o", str(binary)], check=True)
    subprocess.run([str(binary)], check=True)

    # Compile the actual startup cfg/calls against the Cargo feature closure.
    # Backend tests seeded at 0x51010 cannot detect a missing startup call.
    features = tomllib.loads((root / "examples/minimal/Cargo.toml").read_text())["features"]
    def closure(selected):
        enabled = set()
        def visit(name):
            if name not in enabled and name in features:
                enabled.add(name)
                for dependency in features[name]: visit(dependency)
        for name in selected: visit(name)
        return enabled
    main = (root / "examples/minimal/src/main.rs").read_text()
    start = main.index("fn main(mut p: Peripherals)")
    call = main.index("scmi_cold_clock::prepare()", start)
    clock = main.index('    #[cfg(feature = "endpoint-clock-only")]', call)
    cold_call = main[main.rfind("    #[cfg", start, call):main.index(";", call)+1]
    reset = main.index('    #[cfg(all(feature = "pll-sys-core-lock-only"', call)
    startup = cold_call + "\n" + main[reset:clock]
    assert startup.count("scmi_cold_clock::prepare()") == 1
    assert main.count("scmi_cold_clock::prepare()") == 1
    assert call < main.index("let mut gpio22 = p.gpio.pin::<22>().into_output()", start)
    assert call < main.index("enable_endpoint_clock_bit26(&mut gpio22)", start)
    assert call < main.index("pre_state1_reset_clock_boundary()", start) < main.index("freertos_r1::run(gpio22)", start)
    runtime = (root / "examples/minimal/src/freertos_r1.rs").read_text()
    assert runtime.index("let hz = calibrate_cpu_hz()") < runtime.index("scmi::prepare()") < runtime.index("os::start(hz)")
    for selected, expected in [
        (["freertos-scmi-readonly"], "cold"),
        (["freertos-r1"], "none"),
        (["freertos-r2-spi"], "legacy"),
        (["freertos-r2-i2c-nack"], "legacy"),
        (["freertos-r2-uart"], "legacy"),
        (["freertos-scmi-readonly-mixed"], "legacy"),
        *[(["freertos-scmi-readonly", sibling], "legacy") for sibling in
          ("freertos-r2-spi", "freertos-r2-i2c-nack", "freertos-r2-uart")],
    ]:
        enabled = closure(selected)
        if expected == "cold":
            assert {"pll-sys-core-lock-only", "uart0-functional-clock-before-reset-done", "freertos-r1"} <= enabled
        test.write_text('''
static mut CALL: &str = "none";
mod scmi_cold_clock { pub fn prepare() -> Result<(), ()> { unsafe { super::CALL = "cold"; } Ok(()) } }
fn release_pll_sys_reset_bit29() -> Result<(), ()> { unsafe { CALL = "legacy"; } Ok(()) }
fn pulse_width(_: &mut (), _: u32) {}
fn quiet_stop() -> ! { panic!("unexpected stop") }
fn main() {
    let mut gpio22 = ();
%s
    assert_eq!(unsafe { CALL }, "%s");
}
''' % (startup, expected))
        flags = [arg for feature in sorted(enabled) for arg in ("--cfg", f'feature="{feature}"')]
        subprocess.run(["rustc", "--edition=2024", "-A", "warnings", *flags,
                        str(test), "-o", str(binary)], check=True)
        subprocess.run([str(binary)], check=True)
    print("PASS: actual cold startup body and 9 Cargo feature/startup integration combinations; NOT HW")
