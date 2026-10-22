# Opt-in 200 MHz clk_sys startup

`rp1_hal::clk_sys::switch(Source::PllSys200Mhz)` selects the already locked
PLL_SYS primary output; `Source::Xosc` selects nominal 50 MHz XOSC again.
The result is the nominal frequency or a typed `clk_sys::Error`.

Both directions require CLK_SYS_DIV_INT=1. The PLL direction reuses the
existing `SysPllSnapshot::known_apb` admission: CS=0x80000001, PWR=4,
FBDIV_INT=20, FBDIV_FRAC=0, and PRIM /5 /1. PRIM's independent APB phase
gate may be either enabled or disabled. Only CTRL.SRC[1:0] is written;
other CTRL bits, the PLL, and dividers remain intact. Selection is polled at
most 100,000 times for exactly SEL=4 (PLL) or SEL=1 (XOSC). A timeout reports
the last SEL and does not roll back the write. Returning to XOSC does not
require a locked PLL.

Call before either core begins timing-sensitive work, exclude other clock
writers, and recalibrate CPU-based timing afterward. This is a cold startup
API, without a running-scheduler or Linux clock-writer coordination contract.

## Firmware feature

Add `RP1_EXTRA_FEATURES=freertos-clk-sys-200` to the existing
`RP1_RTOS_FEATURE=freertos-amp-observation` build. The feature depends on
`freertos-amp-control` and is absent from production's default feature closure.
`amp_control::run` calls it immediately after `scmi::prepare()`, then repeats
`calibrate_cpu_hz()`, publishes its result in RTOS word 5 (0x2000f814), and
passes that result to `os::start`. Words 6/7 retain the new calibration range.
Switch failure halts before scheduler start through the existing panic path.

The existing 1..500 MHz calibration range admits 200 MHz: a 10 ms sample is
about 2,000,000 cycles, comfortably below SysTick's 24-bit wrap. The unchanged
FreeRTOS bridge accepts the measured rate and derives its 1 ms SysTick reload
from `rp1_freertos_cpu_hz`.

The sealed low `.text` has insufficient space. Only feature builds place the
startup helper and `endpoint_config_finish` in `.clk_sys_init_text` at
0x2000f000..0x2000f2ac (684 bytes), within a linker-enforced 0x600-byte budget
above the fixed downward-growing MSP. The diagnostic proc1 profiler owns the
same gap, so combining the two features is explicitly rejected.

The ELF checker requires explicit `RP1_CLK_SYS_200=1`, set by the build script,
and checks that section's exact bytes and R+X load shape. Relocated C1 IRQ
functions must match the sealed X disassembly after the existing relocation
normalization; command outlined helpers must retain a reviewed byte hash.
The reference ELF must have the sealed X hash. The scheduler stack review is
derived with the existing relocation-only remapper, followed by the unchanged
stack checker and negative tests. No source-change bypass is used.

Two production stack-review source pins were refreshed for cfg-only changes
after proving that feature-off X remains byte-identical. Its ELF hash, tables,
stack pivots, and stack budgets are unchanged. Existing panic line locations
are preserved deliberately; do not reflow the guarded startup/attribute lines.

## Static clock assumptions

| Location | Assumption and consequence |
| --- | --- |
| `crates/rp1-hal/src/uart.rs`, `clock_adopt.rs`, `examples/minimal/src/main.rs` | UART has its own 50 MHz XOSC clock and PL011 divisors 27/8. Switching clk_sys leaves that clock untouched. |
| `crates/rp1-hal/src/i2c.rs` | Counts 781/1186 and SDA hold 593 already assume the maximum 200 MHz clk_sys, keeping nominal SCL at or below 100 kHz over supported rates. |
| `crates/rp1-hal/src/spi.rs`, `spi/receive.rs` | Fixed BAUDR=2000 already targets 100 kHz at 200 MHz; nominal speed rises from 25 to 100 kHz when clk_sys rises from 50 to 200 MHz. |
| `crates/rp1-hal/src/pwm.rs`, PWM proof telemetry in `main.rs` | PWM explicitly configures its independent 50 MHz functional clock. |
| `profiles/rp1-clock-profile.toml`, generated clock tables/DTS, `scmi_clock.rs` | UART stays 50 MHz, APB stays 100 MHz; fixed Linux `sys` was already advertised as 200 MHz. ETH_TSU/ADC's 50 MHz entries are separate clocks. SCMI protocol, rates, votes, and generated tables are unchanged. |
| HAL bounded polling loops | Limits are iteration counts, so their elapsed duration can shorten with a faster CPU. They are not calibrated microsecond deadlines. |
| `freertos_r1.rs`, FreeRTOS bridge/config | Scheduler timing uses measured CPU Hz, not a hardcoded 50 MHz. Raw-timer-based deadlines use the independent 1 MHz timer. |

These are static findings. Peripheral waveforms, actual CPU calibration,
SysTick rate, proc1 slots, and runtime stack high-water remain unmeasured.

## Recorded validation (2026-10-22)

Evidence: `/opt/rpi-cm5-hack/logs/20261022-230259-codex-clk-sys-200-impl/`.

HAL host tests: 100 passed, including six fake-register switch tests.
Feature build: ELF and scheduler stack gates pass, including 85 ELF and
11 codegen mutations plus 16 stack mutations. Worker PSP bound is 1788 bytes
(260 remaining); service PSP bound is 1524 (524 remaining); runtime MSP bound
is 904 bytes. These are static bounds.

ELF SHA256:

- Plain X: `2a6b897be9094e6087e797ef5939ec7a9a57a28149123967062e7707290ccec6`.
- Feature: `9dc126607ea6542808450fcef3a02994067ea7a818a887592e93dd25a8bfdc22`.

Build success for this script is exit 3 with `AMP_proc1_build=PASS` and
`hardware_admission=REFUSED`. Earlier linker, opcode-pin, and source-pin
failures are retained in the evidence directory. No hardware was accessed.
