# Standard Linux control-plane foundation

Status: STATIC/HOST and selected BUILD only. No new SCMI IRQ roundtrip, UART
coexistence, camera, DDR or OpenAMP hardware result is claimed by this change.
Existing RTOS proofs remain on their original source/artifact identities.

## Reproduce

Dependencies: Python 3.11+, PyYAML, dtc, Rust (the RTOS build pins 1.96.0),
GCC ARM 14.2.1. No additional Python library is needed for TOML.

```sh
python3 tools/clock_profile.py
python3 tools/clock_profile.py --check
python3 tools/test_clock_profile.py
python3 tools/test_scmi_clock.py
python3 tools/validate_linux_dtb.py candidate-final.dtb --output validation.json
# Match the candidate to the ACTUAL linked firmware, not a supplied JSON claim:
python3 tools/validate_linux_dtb.py candidate-final.dtb --firmware-elf RP1.elf --output linked-validation.json
# Add --require-camera for the camera/integration admission stage.
```

`profiles/rp1-clock-profile.toml` generates Rust policy, fixed-clock definitions,
SCMI clock IDs, consumer ownership fragment and validator input. UART/CFE
physical rates are requirements, not measured values. Other clock IDs are
UNCLASSIFIED until their consumers are reviewed, not implicitly private.
`clock_profile.py --binding path/to/include/dt-bindings/clock/rp1.h --check`
checks selected IDs and inventories the actual binding file.

`profiles/generated/clocks.dtsi` needs an independently placed `rp1_scmi`
transport node. `ownership.dtsi` targets Raspberry Pi `rp1.dtsi` labels. It
disables the direct clock provider, UART0/SPI0/I2C1 and the unprovided channel0
firmware service; UART1 uses fixed 50 MHz and GPIO0/1 without CTS/RTS. CFE clock
references change but CFE **is not enabled** and no sensor/wiring is invented.
Other enabled consumers must be migrated: this fragment alone is NOT bootable
on the historical complete board DT. The validator intentionally rejects it.

## Kernel

Use the external `profiles/linux-standard.config`, not C patches. Initial
source authority is the retained Raspberry Pi 6.12.75 tree. Generate a separate
output config using `bcm2712_defconfig`, merge via `scripts/kconfig/merge_config.sh`,
run `olddefconfig`, then:

```sh
python3 tools/check_linux_config.py path/to/output/.config
```

This checks the **resolved** symbols including COMMON_CLK_RP1=n. Config success
is not an Image/modules build, boot, final-DTB admission or RP1 remoteproc attach.
REMOTEPROC/RPMSG/VIRTIO configuration alone does not create an RP1 transport.
Custom Linux drivers are outside this architecture. Maintained-next-branch
build compatibility remains OPEN.

## Firmware

`scmi_clock` is an allocation-free Base/Clock 2.0 dispatcher. No Reset protocol
is advertised. It supports the reviewed discovery, rate_get, discrete locked
rate discovery, and opt-in vote/config_set plus exact-rate no-op rate_set.
Notifications/extended/async protocol features are not advertised. The backend
must return actual clock state; a rate mismatch returns HARDWARE_ERROR rather
than the desired profile rate. Unsupported rates/IDs/flags are rejected.

Physical requirement is boot_required OR rp1_required OR linux_required. A
logical Linux vote commits only after successful physical readback. The initial
`ReadOnlyUartApb` backend reads only the established PLL_SYS tuple; it never writes
clocks even in vote-test mode. It does not accept alternative PLL tuples or
automatically reinitialize PLL_SYS after Linux boot. New typed gate/rate/reset
backends require separate admission and evidence.

`clock_adopt` checks a stable UART CTRL/DIV/SEL snapshot for nominal XOSC 50 MHz,
DIV1, enable/status and observed SEL=1. Existing 115200 baud uses IBRD/FBRD27/8.
`Uart0::init_uart_with_existing_clock` performs no MMIO writes on mismatch; on
success it initializes only the owned UART/pins. Caller still owns APB/reset,
quiescence and the stable clock lifetime. It is not a general clock solver.

`scmi_mailbox` implements bounded one-request publication and preserves header/
token, orders response payload before FREE and FREE before HOST_EVENTS. Its
mutable state has one proc0 owner; no AtomicU32/shared exclusive primitive is
used. Its MMIO adapter touches only the channel1 event bits and its assigned
SRAM. It does not set up NVIC, mask interrupts or claim IRQ57 is proven.
Host fake-I/O tests check publication order, not PCIe ordering/IRQ delivery.

## SRAM and build admission

Old fb00..fbff SCMI storage collides with the current RTOS fault ABI and must
not be used. `rp1-hal/scmi-clock` reserves 256 bytes in a dedicated NOLOAD
`.scmi_shmem`, outside BSS/warm-data and below __app_limit. The old ABI is intact.
The linker checks size/alignment/limits. Initialize this channel explicitly
only while Linux is quiesced. Warm restart/SCMI reconnection is NOT yet admitted.

```sh
cargo build -p rp1-example-minimal --release --target thumbv7m-none-eabi \
  --locked --features freertos-r1,rp1-hal/scmi-clock
python3 tools/scmi_elf_layout.py path/to/rp1-example-minimal --output layout.json
# Optional: derive the SCMI transport fragment from the board's base DTB:
python3 tools/scmi_elf_layout.py path/to/rp1-example-minimal --output layout.json \
  --dtb board-base.dtb --dtsi scmi-transport.dtsi
```

This selected build verifies the reservation fits with R1; it does **not** wire
or run the server IRQ, prove full mixed/warm image capacity, or enable a Linux
deployment. Derive DT placement from the final linked ELF, never from the
address of a previous build. `layout.json` separates M3-local and BAR2-offset;
CPU-physical/DT-bus translation must be checked against the final board DT.

The optional transport generator walks the supplied DT `ranges` and identifies
the 64KiB RP1 SRAM at system address `0xc040400000`. It keeps M3 local
`0x20000000`, BAR2 offset and Linux CPU physical address separate. It emits a
source include using absolute DT node paths, not a guessed host physical address.
Combine this with generated clocks/ownership and reviewed remaining consumer
changes, then compile the **final** DTB and validate it with `--firmware-elf`.
This does not modify the input DT or create a complete board configuration.

The firmware now retains its profile SHA in `.rp1_clock_profile`; the linker
requires exactly 64 bytes. ELF validation reads those bytes and checks the SCMI
section's exact address/size/type and absence of allocated-section overlap.
DT validation compares the actual ELF SHA, linked profile SHA, translated
SCMI address and 256B reservation. Old images without a fingerprint are rejected
for this new paired-image check; their old hardware proof is not invalidated.

Initially all SRAM except the one SCMI slot is firmware-owned. An enabled Linux
SRAM reservation in that remainder is rejected, including the historical
channel0 `shmem@ff00`. Disable/remove that Linux reservation only together with
its unprovided `rp1_firmware` service; adding another shared service requires an
explicit allocator update. Runtime BAR assignment, clock readback and IRQ
delivery still require hardware evidence even after this structural check passes.

DT compiler metadata is not a hardware node, and a child named `clocks` is not
a clock-reference property. Dynamic reserved pools (for example Linux CMA)
are recorded as requested sizes/allocation envelopes, not invented fixed
addresses. Their actual kernel-selected allocations remain a live admission
check. Static reserved regions still undergo exact overlap checks.

## Remaining gates

1. Observable known-good boot/current kernel/config/DT and physical camera pins.
2. Migrate every enabled clock consumer; validate final DT and SRAM vs ELF.
3. Standard SCMI Base/rate_get -> actual M3 IRQ -> response -> host IRQ, 2/2.
4. Fixed-clock UART0/UART1 hardware coexistence, then camera, votes and timesync.
5. Exact M3 outbound DDR contract before bounded DDR writes or DDR OpenAMP.

The DT validator's PASS is explicitly structural only. It checks enabled and
disabled references separately, pin ownership, profile IDs/rates, mailbox reuse,
translated reserved/shmem overlaps and reciprocal graph links. Firmware-internal
allocations cannot be inferred from a DTB alone. Physical camera identity,
live register values, compiled kernel functionality, driver runtime writers,
transport restart epochs and host write-effect isolation need separate evidence.
