# RP1 HAL

This repository is the workspace for RP1 firmware HAL work. It contains the
shared ABI definitions, a minimal runtime, a small HAL facade, entry-point
macro support, build-time RP1 note generation, and a minimal firmware example.

The opt-in `freertos-amp-proc1-gpio-irq` candidate consumes a generated GPIO9
PROC1 interrupt grant and a typed input into the finite local C1 demo. It adds
software-force delivery/acknowledgement status and owned-IRQ retirement; its
ELF must use separate explicit IRQ admission. It is not physical sensor or
5kHz end-to-end qualification. Current evidence and ownership are in the
[AMP acceptance matrix](/home/hotaru/rpi-cm5-hack-amp-work/docs/amp-acceptance-matrix.md)
and [IRQ contract](/home/hotaru/rpi-cm5-hack-amp-work/docs/amp-gpio-irq-commissioning.md).

P1A9/T keeps S's command/IRQ/ownership contract and skips u64 division for the
normal period path; the guard countdown still checks every1000 executed cycles.
Proc1 host6, ABI/HAL109 and strict ELF/DT/parser tests pass. T is staged only:
the current managed exec environment has no serial hardware and refuses sudo.
Hardware is BLOCKED_HARDWARE, performance NOT_TESTED; S remains the last verified
deployment. [T report](/home/hotaru/rpi-cm5-hack-amp-work/reports/amp-period-cost-20261009.md).

U adds `bulk::observation::Collector`: one local queue consumer and unique
Duplex window, four words/pass, one96B OBS1 record, exact FREE before reuse.
Held buffers never dequeue or touch DDR; source drop-newest is visible.
111 host tests and a Cortex-M3 object probe pass. It is not wired into the
OpenAMP endpoint, which still sends commissioning patterns. Production ELF is
byte-identical to T; U adds no runtime or hardware qualification.
[U report](/home/hotaru/rpi-cm5-hack-amp-work/reports/amp-observation-collector-20261009.md).

P1A8/S captures QUERY_CONTROL time after the coherent C1 receipt copy and replies
with mode3; timer failure/regression retires the endpoint and requests C1 stop.
109 host tests and strict Cortex-M3/DT checks pass. Two loaded cohorts,
closures and live ownership audits pass scoped with the same15 files/live DT.
All26113/25994 software forces completed, but4686/4681 periodic slots missed,max377/383us:
SAFE_NEGATIVE, no physical sensor/5kHz e2e. C1's27 bounded function bodies retain
R instructions and peripheral scope. The following R note is historical.

P1A7 source adds lazy local trace construction and zero unmeasured edge timestamps;
P1A6/Q is the retained recovery tuple. P1A7 passes109 host tests and one loaded
software-force/DDR/RPMsg/UART1/GPIO cohort. Its repeat fails strict expiry timing:
the response admission timestamp precedes the concurrent receipt copy. This is
an observation-time contract defect, not proof of early C1 expiry. Both closures
pass. Missed periodic slots4662/4708, max409/383us remain SAFE_NEGATIVE. First
boot's BAR regression is retained. No physical sensor or performance PASS.

The proc0 FreeRTOS warm IRQ-owner build candidate is documented in
[BC01 runtime](docs/rtos-warm-persistent.md). Its eight-generation commissioning
feature is not yet a hardware-verified release; the older minimal introduction
below does not describe that RTOS feature.

## `rp1-abi`

`crates/rp1-abi` defines the initial fixed `.note.rp1` ABI shared by RP1
firmware images and the CM5 RP1 bootloader PoC. It is `#![no_std]` and currently
contains only the boot note layout and owner bitmap device constants.

## Minimal Firmware

`examples/minimal` builds a `thumbv7m-none-eabi` no-std firmware ELF and uses
`rp1-build` to generate an RP1 boot note from `examples/minimal/rp1.toml`.

Recommended build flow:

```sh
nix develop
cargo run -p cargo-rp1 -- build --example minimal
```

This builds the example, attaches `.note.rp1`, checks the minimal bootloader
compatibility rules, and writes:

```text
target/rp1/release/RP1.elf
```

The minimal example now exercises the first HAL API surface:

- `Peripherals` owns `gpio`.
- `gpio.pin::<0>().into_output()` type-checks as an owned GPIO output pin.
- the loop calls `ConfiguredPin::toggle()` and `mailbox::poll()`.

These are skeleton APIs. GPIO configuration and output methods are currently
no-op because the RP1 GPIO register offsets have not yet been verified against
the RP1 datasheet and Linux DTB. The mailbox module is also a version responder
skeleton only: it exposes the non-PIO firmware version shape and reports
`GET_FEATURE("PIO ")` as unsupported, but it is not connected to an RP1 mailbox
shared buffer yet.

Manual build fallback:

```sh
nix develop -c cargo build -p rp1-example-minimal --release --target thumbv7m-none-eabi
```

Attach the generated note to the ELF:

```sh
NOTE_BIN=$(find target/thumbv7m-none-eabi/release/build/rp1-example-minimal-* -name rp1_note.bin | head -n1)
tools/attach-rp1-note.sh \
  target/thumbv7m-none-eabi/release/rp1-example-minimal \
  "$NOTE_BIN" \
  target/thumbv7m-none-eabi/release/rp1-example-minimal-note.elf
```

Or build and attach the note in one step:

```sh
nix develop -c tools/build-minimal-note.sh
```

The firmware linker places all loadable segments in RP1 SRAM starting at
`0x20000000`, so `RP1-bootloader-PoC` can materialize the ELF into a contiguous
bootstrap image.

Earlier minimal ELF builds kept `.bss` in a separate `PT_LOAD` at `0x20040000`.
That address is outside the PoC RP1 materializer window, which starts at
`0x20000000` and is capped at the RP1 bootstrap scratch size, so the bootloader
reported `Rp1ImageInvalid`. The runtime linker script now keeps `.vector_table`,
`.text`, `.data`, and `.bss` in the same 64 KiB RP1 SRAM image.

The minimal owner bitmap generated from `examples/minimal/rp1.toml` is:

```text
owner_rp1      = 0x343
owner_linux    = 0x3c
owner_disabled = 0x80
mailbox         = 0x1
version_kind    = 0
```

To smoke-test the ELF with `RP1-bootloader-PoC`, copy the note-attached ELF to
the TFTP root as `RP1.elf` and boot a TFTP-enabled PoC image:

```sh
cp target/rp1/release/RP1.elf /opt/rpi-cm5-hack/tftpboot/RP1.elf
```

Expected UART markers:

```text
[RP1NOTE] valid: owner_rp1=0x343 owner_linux=0x3c owner_disabled=0x80 mailbox=0x1 version_kind=0
[RP1ELF] load_base=0x20000000 image_len=... entry=0x20000069 stack=0x100030d0
[RP1BOOT] proc0 started
```

## Development Shell

Enter the Nix shell before building RP1 firmware code:

```sh
nix develop
```

The shell provides:

- nightly Rust
- `rust-src`
- `llvm-tools-preview`
- `thumbv7m-none-eabi`
- `cargo-binutils`
- `llvm-objcopy` / `rust-objcopy`
- `readelf`
- `xxd`
- `dtc`

The standard build command is:

```sh
cargo build --workspace
```

Build the firmware target explicitly when producing RP1 images:

```sh
cargo build -p rp1-example-minimal --release --target thumbv7m-none-eabi
```

The shell also provides a convenience wrapper:

```sh
build-rp1-hal
```

`build-rp1-hal` runs the same target build through the Nix-provided toolchain.
