# BC01 warm IRQ owner (build candidate)

Status: STATIC/BUILD, hardware OPEN. This normal source feature extends the
two-generation warm combined owner to **eight** checked generations. It is not
an indefinite service, a second watchdog arm/feed policy, or a full R3 result.

## Reproduce

Initialize `third-party/FreeRTOS-Kernel` at the repository gitlink: official
FreeRTOS V11.3.1, commit `3a22924e0a9ddbbc8b0758881c33b3422a5cc20d`,
`portable/GCC/ARM_CM3`. The build rejects a different/modified kernel.

Pinned tools: Rust1.96.0 `ac68faa20`, `thumbv7m-none-eabi`; ARM GCC14.2.1,
`-mcpu=cortex-m3 -mthumb -mfloat-abi=soft -Os`. Rust uses Oz/fat LTO/one codegen
unit. `build.rs` rejects changed compiler identities and hashes individual
installed Cortex-M3 newlib memcpy/memset/AEABI objects; no complete libc,
allocator, syscalls, custom memory primitive, kernel patch or extra dependency.

```sh
RP1_RTOS_FEATURE=freertos-r3-watchdog-warm-persistent \
  bash tools/build-freertos-r1.sh /absolute/new/build-output
```

The helper currently ends with exit3 **after** foundation BUILD checks: new
compiled-envelope/hardware admission is deliberately separate. It runs pure
owner-state tests, exact calibration comparisons, pinned-memory QEMU tests and
ELF/vector/layout checks. No hardware is touched by this command.

Link12/13 candidate SHA256:
`abb0192767f849ccbaaaecf0c60d408084d84451192ad46a9c034dbfb1fa854a`.
text45208B/data16B/BSS12004B (11976B BSS +28B warm data shadow),
shadow end0x2000df8c:116B below reserved MSP0x2000e000..0x2000f000.
Seven R1 tasks, owner512words, pool2304words, idle128words and MSP4KiB remain.
Proc0 only; proc1 does not operate this kernel. Dynamic allocation/tickless
remain disabled. Existing expected-fault, context, stack and fresh R1 guards
are retained. No Linux kernel/config/module/userspace change is involved.

## Owner and observation contract

One priority5 owner uses existing SPI0/I2C1/UART0 adapters. SPI0 IRQ19/IPSR35,
I2C1 IRQ8/IPSR24 and UART0 IRQ25/IPSR41 use priority0xc0 and FromISR wakeups.
Transactions are ordered SPI -> UART -> I2C, **not simultaneous wire traffic**.
Each adapter must withdraw IRQ/buffer ownership and finish checked cleanup
before completion is counted or the caller's buffer can be reused.

Generation1..8 is distinct from alternating SPI payload template1/2. UART
uses exact decimal generation tokens; existing peer I2C odd/even lengths2/31.
After ACK2, the owner hands GPIO22 permanently to the priority4 monitor and
waits for the complete type8 handback before generation3. Host/controller
scripts require the full same-run/type8 trace before sending the next payload.
At generation8 the owner parks; that boundary does not claim ongoing acquisition.

BC01 words126..149 hold three rolling generation-qualified receipts.
150 is owner seqlock;151..155 generation/operation/deadline/completion count/
last-completed tick;156..158 checked IRQ totals;159 type8 handback.
Monitor161..163 reports eligibility/tick/observed owner sequence. Eligibility
is revoked before fresh R1 checks, fault/assert and diagnostic serialization.
It is historical observation, **not an atomic external snapshot or feed grant**.
The matching `tools/check-warm-persistent-record.py` admits only stable terminal
generation8/op8/count24 two-copy records with live R1 progress. Do not substitute
the old AZ six-receipt validator. Tests: `tools/test-warm-persistent-record.py`.

Deadlines, checked IRQ accounting and generation state admit legitimate active
driver/cleanup intervals while requiring other peripherals quiet. Retrying a
seqlock snapshot is bounded to three attempts; failure revokes eligibility.
No unproven cross-core LDREX/STREX lock or new watchdog feed loop is introduced.

## Calibration and next gate

`tick_calibration.rs` preserves floor(cycles*1000000/dt) exactly for24-bit
cycles and admitted dt10000..11000us, using bounded32-bit quotient/remainder
arithmetic. Five individually admitted samples<=500MHz sum<=2.5e9. Sampling,
raw-timer source, rounding, interval/frequency/spread guards are unchanged.
55,578,434 host comparisons and rejected-input checks are model evidence;
they do not establish absolute clock accuracy or replace the RP1 measurement.

Before deployment: independent compiled startup/critical-envelope review,
fixed source/build/harness inputs, current TFTP/ESP identity and recovery lock,
known-good preflight, commissioning, same-artifact formal2/2 and final baseline.
Use the existing CM5 control/cohort workflow in the evidence repository; this
document authorizes no generic MMIO, unknown reset/POWER write or image copy.
Long warm soak, health-based watchdog feeding, ADC, DDR/OpenAMP and complete
protected Linux sharing remain separate OPEN conditions.
