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
ADC, RP1 GEM and DMA now use profile-defined fixed-clock mappings with the reviewed
driver-compatible strings and clock-name order. Their enabled status is preserved;
the validator rejects disabling them to make the clock-reference check pass. Other
binding IDs remain UNCLASSIFIED. USB has no explicit clock references in this base
DT; its inherited physical dependencies are not thereby proved or removed.

The profile now declares nine of the current binding's47IDs: eight FIXED and one
locked SCMI discovery clock. SYS200/DMA100/ADC50/ETH125/TSU50MHz are required physical
holds, not a newly implemented clock programmer. The current Linux CCF census reports
DMA100MHz but MIPI CFG50MHz; desired MIPI25MHz MUST fail physical admission until an
exact safe setup/readback is implemented. CCF rate reports are not waveform proofs.

Compile the actual supplied base and generated fragments together:

```sh
python3 tools/build_linux_dtb.py --base board-base.dtb \
  --firmware-elf RP1.elf --out /path/to/new-dtb-build
```

This uses a single DTS translation unit, not a flattened overlay: `/delete-property/`
in a flattened overlay cannot remove an existing base property. Existing labels are
resolved through the base `__symbols__` table to absolute paths; generated local
SCMI/fixed labels remain local. Unknown/colliding references fail. The base must have
the reviewed symbols, and the actual firmware fingerprint must match the profile.

Provider assignments are removed, UART0/SPI0/I2C1 and the unavailable channel0 service
are disabled, UART1 is enabled, old `rp1_fw_shmem` is disabled together with its only
consumer. No unrelated base-node availability change is accepted. Final validation
uses the compiled DTB and actual linked ELF, including exact256B SRAM placement.
Output contains the original private live-DT data; retain it privately unless reviewed.
Structural PASS is still not boot/deployment admission or proof of physical holds.

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

### Cold read-only runtime (candidate IRQ57)

The optional `freertos-scmi-readonly` feature connects the existing dispatcher
to vector73 of the normal RTOS vector table. It does not relocate VTOR or reset
sibling peripheral vectors. `freertos-scmi-readonly-mixed` also includes the
existing R2 SPI/I2C/UART workload. Use the normal pinned build helper:

```sh
RP1_RTOS_FEATURE=freertos-scmi-readonly tools/build-freertos-r1.sh /new/r1-scmi
RP1_RTOS_FEATURE=freertos-scmi-readonly-mixed tools/build-freertos-r1.sh /new/mixed-scmi
```

Successful selected builds deliberately exit3: BUILD PASS, hardware admission
REFUSED until separate review/commissioning. The helper preserves RTOS config,
family optimization/LTO, official kernel, stack budget and ELF validators. Raw
`cargo build` default flags are not interchangeable with this pinned build.

Plain R1 (no R2 SPI/I2C/UART/mixed sibling) first performs a separate cold clock
prerequisite as the first main action, before GPIO/reset/endpoint/PCIe writes
and CPU calibration. Two equal
snapshots must show the PLL reset defaults `1/3f/0/0/77000/80010000`, UART clock
`0/1/1`, and asserted/not-DONE PLL_SYS and UART0 resets. Any mismatch, including
running `77010` or already-correct `51010`, rejects before any clock/reset write.
Existing reset-release/core-lock and UART clock-before-reset-DONE helpers are
reused. Only the prior active-DMAC proof's selected `PRIM=51000`, DSB/readback,
`PRIM=51010`, DSB/readback sequence is added; no DMA operation is called. Final
PLL `80000001/4/20/0/51010/80010000` and UART `10000840/1/1` are read back.
Failures halt; there is no runtime retune/retry or new recovery contract. This
is BUILD/HOST coverage, not new hardware proof or external frequency accuracy.
R2/mixed keeps its existing `77010` initialization and cannot satisfy the exact
100MHz SCMI admission; this change does not qualify that separate cohort.
The host check covers the actual startup body and Cargo feature/call integration,
not only protocol backends seeded at the desired tuple.

SCMI protocol preparation requires cold proc0/PRIMASK1, correct vector, inactive/unowned IRQ57,
zero PROC_EVENTS and exact enabled SCMI APB tuple. Only this IRQ pending/priority/
enable is changed (priority0xc0); no global unmask, VTOR/AIRCR/BASEPRI/clock write.
Scheduler owns the PRIMASK transition. No extra task, heap or shared exclusive
primitive is introduced. The ISR services one bounded request and records actual
IPSR, event/active/pending bits, responses/notifications, error and raw timer times.
Unknown/unhandled source masks IRQ57 without clearing another channel. Warm
restart combinations are refused pending Linux quiescence/epoch coordination.

`RP1_SCMI_TELEMETRY` is a separate108-byte private BSS record; its ELF symbol is
the authority, not a fixed address. Cold preparation and each ISR publish odd/
even sequence with compiler/bus barriers. Read sequence,108bytes,sequence; accept
only equal even sequence, magicSCI1/version1. `ready=1` means this transport's
readonly service prepared, NOT all fixed-clock holds or Linux handoff admitted.
An error clears ready and requires recovery, not blind rearm. Handler duration
uses1us raw timer and excludes entry/prologue/tail cost; not complete IRQ latency.

IRQ57 remains a candidate from prior firmware analysis, not a hardware-proven
current route. Earlier Linux request visibility without PROC_EVENTS/IRQ delivery
and endpoint BAR/reset mismatch remains a negative boundary. No endpoint replay
or unknown route writer is added. The standard rp1-mailbox Linux driver uses
mailbox-framework TX-done polling; this must be distinguished from the required
SCMI response IRQ completion (no firmware request polling in this runtime).

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

## Bounded standard Linux Image build

`tools/build_linux_image.py` keeps the source tree unchanged and uses an explicit
out-of-tree directory. Supply the retained standard config, official source,
output and record directories with `--config`, `--source`, `--out`, `--record`.
First invocation only configures. Inspect `config.diff`, then repeat with
`--image REVIEWED_CONFIG_SHA256`. The20-symbol fragment must still match.
Cross-GCC14 is explicit; compiler-driven config changes are not silently accepted.
The driver limits jobs to1/2 and checks MemAvailable2GiB, output filesystem1GiB,
record filesystem1.5GB every5seconds. Failure/time/resource limits terminate the
dedicated build process group and retain output for diagnosis. No cleanup, source
patches, `modules_install`, deployment or boot is performed. Run the small host
guard checks with `python3 tools/test_linux_image.py`.

Image success does not prove module closure, standard RPMsg attach, physical
clock ownership or a successful boot. Retain config/tool identities and selected
products before releasing scratch space; never publish private keys/raw DT data.

## Endpoint transition observer (not recovery)

`RP1_RTOS_FEATURE=freertos-endpoint-uart tools/build-freertos-r1.sh /new/output`
builds the existing plain-R1 UART0 diagnostic, with no new task or PCIe writer.
The existing monitor samples once per roughly one second and caps output at32
changes plus CAP. Sequential selector checks cannot detect ABA. The270-byte
record adds selector-independent MONITOR2/INTR/INTE/INTS plain reads at
`0x401081a4/1a8/1ac/1b4` to the DBI snapshot. INTR is never acknowledged or
masked; the destructive-read LTSSM FIFO at`0x40108124` is excluded.

`python3 tools/check-endpoint-uart.py --self-test` checks the decoder.
Use `tools/check-endpoint-uart.py capture.raw --report observation.json` on a
captured UART stream. It also accepts the earlier206-byte records without
inventing APBS fields. Latched events plus current levels cannot establish their
order, the electrical reset cause, or the official firmware's current state.
New event-only changes are observable even if the DBI selector is ambiguous.

This optional build intentionally exits3 after successful ELF/test validation:
it is not hardware admission. Rebind the final DT, SRAM reader and bootloader to
the new ELF, and review the compiled observer/memory budget before deployment.
The current R1 cold state5 path does not maintain the official PCIe event loop;
replaying that reset sequence from a running RTOS is not yet admitted.

## Remaining gates

1. Current kernel/config/DT census and held-R1 recovery observed separately; camera absent.
2. Generated actual final DT and SRAM vs ELF checked; prove physical holds, inherited
   USB dependencies and standard-kernel availability before deployment.
3. Standard SCMI Base/rate_get -> actual M3 IRQ -> response -> host IRQ, 2/2.
4. Fixed-clock UART0/UART1 hardware coexistence, then camera, votes and timesync.
5. Exact M3 outbound DDR contract before bounded DDR writes or DDR OpenAMP.

The DT validator's PASS is explicitly structural only. It checks enabled and
disabled references separately, pin ownership, profile IDs/rates, mailbox reuse,
translated reserved/shmem overlaps and reciprocal graph links. Firmware-internal
allocations cannot be inferred from a DTB alone. Physical camera identity,
live register values, compiled kernel functionality, driver runtime writers,
transport restart epochs and host write-effect isolation need separate evidence.
