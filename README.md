# RP1 HAL

This repository is the workspace for RP1 firmware HAL work. It contains the
shared ABI definitions, a minimal runtime, a small HAL facade, entry-point
macro support, build-time RP1 note generation, and a minimal firmware example.

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

Build and attach the note in one step:

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
