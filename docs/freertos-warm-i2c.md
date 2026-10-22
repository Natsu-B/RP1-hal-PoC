# Selected warm I2C1 owner (AY)

Build-only candidate; hardware acceptance remains separate. Use the existing
official FreeRTOS ARM_CM3, compiler and target pins. The opt-in feature is
`freertos-r3-watchdog-warm-i2c`; default and earlier features are unchanged.
`tools/build-freertos-r1.sh OUTPUT` selects O3/fat LTO, with only the Rust
`rp1-freertos` package at size optimization. The opt-in 2304-word C/Rust pool
removes 256 unused words, not any task stack: seven R1 stacks use1792 words
and the sole I2C owner has512. The independent MSP remains4096 bytes.

The first reviewed ELF is
`1196c6a0685b6800760dcce357ad057db0e2f82b06e93b4089fcaeea105fe775`.
Its reset capture and warm-data handling were inlined by LTO. The checker
does not infer arbitrary inlined initialization semantics. After independently
reviewing this exact image, set `RP1_WARM_I2C_ELF_SHA256` to that SHA and
`RP1_WARM_I2C_REVIEWED_INLINED_RESET=1`; then run the normal build with
`RP1_RTOS_FEATURE=freertos-r3-watchdog-warm-i2c`. A changed image needs new
source/compiled review. The explicit flag alone cannot admit another image.

After checked watchdog expiry/Reset entry, reuse the selected warm UART/PLL
preparation and release only known I2C1 reset bank0bit8. Require the recorded
reset/DONE state, nonclearing identity/body snapshots, quiet IRQ8 and pin
ownership before constructor writes. Initial IMR is48ff; IC_STATUS is recorded,
not assigned an invented reset-value predicate. No cold main/PCIe replay.

The task owns the existing bounded I2C adapter and moves GPIO22 marker
ownership out of the monitor. GPIO9 is the existing ESP32 low/hi-Z grant input.
13/17/19/23/29ms markers sequence first NACK, checked cleanup, ESP I2C0
reset/rearm, actual bytes31/4e through IRQ8/IPSR24 and final release. Native
peer host/ESP deadlines remain4s/5s with one arm, no lease renewal. The first
NACK must not modify the guarded buffer; the second result must be314ec3c3.
Generation, cancellation, pending notification, canaries, bounded cleanup and
post-delay buffer integrity are checked before returning marker ownership.

Telemetry124..183 holds preparation, two16-word receipts, owner watermark,
pulses and IRQ total/IPSR; watchdog96..123 and fault184..255 stay reserved.
Early-entry words are reused only after saving the fresh EPOCH. Type9 requires
both transactions, five R1 progress checks, context/synchronization and every
task/MSP margin. E70..79 are explicit refusal/timeout/revocation diagnostics;
terminal failure masks interrupts and lowers the marker before diagnostic
publication. This is not recovery from arbitrary task corruption.

Private host captures must independently join nonce, exact110 post-ARM edges,
native lease/reset/callback receipts and type9. Extra, missing, overflowed or
post-success diagnostic edges reject. GPIO high alone and a preloaded payload
are not hardware success. NACK recovery is specific to the existing ESP I2C0
reset contract, not arbitrary slave automatic recovery.

OPEN: warm electrical/IRQ payload acceptance, runtime stack high-water marks,
native trace capacity/timing, formal2/2. This is neither warm integrated R2,
in-flight recovery, watchdog health feeding, PCIe recovery nor full R3.
