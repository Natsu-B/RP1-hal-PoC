# Proc0 warm combined owner — static candidate, not hardware admitted

Feature: `freertos-r3-watchdog-warm-combined`. Normal source is
`examples/minimal/src/warm_combined.rs`; it reuses the IRQ adapters in
`crates/rp1-freertos/src/{spi0,i2c1,uart0}.rs` and the selected preparation
contracts in `warm_spi.rs`, `warm_i2c.rs`, and `warm_uart_prepare.rs`.
Separate warm UART/SPI/I2C cohorts do **not** prove this combined image.

## Fixed configuration

- FreeRTOS Kernel `3a22924e0a9ddbbc8b0758881c33b3422a5cc20d`, official
  `portable/GCC/ARM_CM3`; proc0 only, no dynamic allocation or tickless mode.
- Rust `1.96.0 (ac68faa20 2026-05-25)`, `thumbv7m-none-eabi`; GCC14.2.1,
  Cortex-M3 Thumb soft ABI. Candidate RustOz/fatLTO, Ckernel-Os.
- Seven unchanged R1 task stacks plus one512-word, priority5/id8 owner;
 2304-word pool, separate128-word idle stack and4KiB MSP. No stack-budget cut.
- Opt-in `sync-pool-r1`: two queues(capacity limit4), one binary semaphore
  and one mutex. R1 allocates queue0/4,queue1/1,binary0,mutex0. Default pools
  remain4/16/4. Invalid-capacity tests use a valid unused queue slot.
- Opt-in `newlib-memcpy`: only the installed Cortex-M3 newlib `memcpy` and
  `aeabi_memcpy` object members, not full libc. The build pins both member
  hashes and rejects any dependency other than the latter's call to memcpy.
  No allocator, syscalls or custom copy implementation is introduced.
- RTOS naked Reset establishes CCR.UNALIGN_TRP=0/STKALIGN=1 before any Rust
  call, preserving other CCR bits. Copy is for ordinary memory, never MMIO.
- Combined-only counted cold delays explicitly retain the selected AY
 50-YIELD,4-load and64-load loop bodies. Outlining and Oz still change
  surrounding instruction gaps; full cold-path timing equivalence is not claimed.

## Ownership and sequence

Warm restart bypasses cold main/PCIe setup. It performs the established common
PLL preparation once, checked I2C/SPI reset preparation, and UART0 TX-only setup.
One task owns all three drivers, GPIO22 and default notification0; IRQ19/8/25
have encoded priority0xc0 and call only their respective bounded adapters.
The drivers' full completion receipts, generations, buffers and post-cleanup
quiet state are checked before another operation begins.

Twice, in order: external SPI `69963c01/02` → UART0 READY/actual19-byte reply →
I2C1 actual IRP3 payload(length2 then31) → UART0 ACK. The existing ESP0.6.3
SPI/I2C stream peer is a candidate; UART uses the current USB UART connection.
No new equipment, Linux change or new ESP firmware is assumed.

Telemetry124..183 is a new compact AZ01 schema, skipping160(owner stack HWM).
Each stored receipt contains actual payload FNV1a plus IRQ/timing fields; the
full raw receipt is checked locally, not externally dumped. Words96..123 and
184..255 retain their existing watchdog/fault contracts. The typed completion
is type8, not a relabel of older type9/F/D successes. Errors remain diagnostic
halts; neither a GPIO success code alone nor a separate recovery boot proves
the integrated peripheral sequence.

## Reproduce the current static build

Initialize the pinned kernel checkout, then use a fresh output/cache:

```sh
RP1_RTOS_FEATURE=freertos-r3-watchdog-warm-combined \
CARGO_TARGET_DIR=/absolute/new/temporary-cache \
bash tools/build-freertos-r1.sh /absolute/new/output
python3 -B crates/rp1-freertos/tests/sync_pool.py
python3 -B crates/rp1-freertos/tests/newlib_memcpy.py
python3 -B tools/test-warm-combined.py
```

The helper intentionally does not grant deployment admission. The legacy named
watchdog probe is inlined in this image. `check-warm-combined-elf.py` instead
pins the entire reviewed AZ14 ELF and its active77-row critical envelope; any
different image requires another review. The old checker remains unchanged.
Cold-path timing and external peer joins still need commissioning/admission.
Preserve unsuccessful logs/maps as well as fitting ELFs.

The installed QEMU Cortex-M3 copy model covers16,640 ordinary-memory cases and
the expected UNALIGN_TRP-on negative case. This is instruction-level model
evidence only: no RP1 timing/bus/peripheral or source-overread guarantee follows.

| Level | This candidate's boundary |
| --- | --- |
| R1 | Existing cold R1 evidence remains; new combined image not yet run |
| R2 | Existing cold integrated and separate warm bus proofs remain; combined warm HW OPEN |
| R3 | Not complete: watchdog integrated recovery, ADC, DDR/OpenAMP and protected sharing remain separate requirements |

Before hardware: repeat the fixed build; finish compiled/peer/controller checks;
freeze inputs; use the sole hardware lock for known-good preflight,
commissioning, same-image formal2/2 and the agreed separate final baseline.
Do not infer whole-system watchdog recovery or autonomous health feeding.
