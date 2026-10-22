# Proc0 I2C IRQ adapter — selected IRQ foundation

`rp1-freertos/i2c1-irq` provides a single-owner, notification-blocking receiver
using the existing bounded RX engine and actual local IRQ8/vector24. It does not
install a global IRQ mask, VTOR, reset writer, or cross-core lock. FromISR runs at
logical priority6; official FreeRTOS handles the wake and PendSV request.

Own the peripheral, pins2/3, clocks and IRQ8 exclusively. Construct in a proc0
task after the known PLL/reset/startup path. Supply buffers of1..33bytes and a
nonzero timeout below half the32-bit tick range. The ISR only touches driver-owned
storage, never the caller buffer. After terminal status the task withdraws the
generation, disables IRQ/controller, retains fatal evidence before selective ACK,
checks disabled/FIFO/pending state, and samples quiet over at least4ms with1tick
sleeps. Only then copy the actual prefix to the caller and permit rearm. Failed
cleanup halts for recovery; it is not a successful Drop-based recovery.

Quiet evidence includes sample count and maximum gap: it is not continuous
observation. No success may hide a fatal cause seen during cleanup. A new request
is published generation-last; cancellation checks and notification are one C
critical-section transaction. Kernel `higher_priority_wakes` distinguishes a
blocked task being woken from merely having an IRQ before the wait.

`freertos-r2-i2c-nack` is the first8-task integration workload: two reads to the
known unassigned0x2e with the present ESP32 slave at0x2d, requiring retained address
NACK, actual IRQ wake, unchanged caller buffer/canaries and checked rearm. It must
not call the current ESP32's protected0x2d callback outside its READYACK protocol.
This workload does not prove successful payload RX, timeout/cancel HW behavior,
arbitrary-slave NACK recovery, or combined SPI/I2C/UART operation.

Build: `RP1_RTOS_FEATURE=freertos-r2-i2c-nack tools/build-freertos-r1.sh /new/out`.
Run host tests: `cargo test -p rp1-hal --target x86_64-unknown-linux-gnu`,
`python3 tools/test-irq-publication.py`, `python3 tools/test-isr-notify.py`, and
`python3 tools/test-spi-cancel.py`. Preserve ELF/vector/memory admission checks.
Keep STATIC/BUILD checks separate from the selected hardware cohorts below.

## Current peer workload

`freertos-r2-i2c-peer` uses the same driver/task slot and current ESP32 native
READYACK service. It takes R1 startup's existing typed GPIO9 input-pull-up handle,
checking its configured register/reset state; it never drives that wire. GPIO22 belongs
to this worker during the handshake, then transfers to the monitor after the
final HIGH dwell. Seven other kernel-test tasks continue throughout.

Host observes13tick READY pulse, arms one absolute5s peer lease and stops the
slave. LOW1 grants a NACK read, then17tick CLEAN1 permits separator HIGH. The
19tick READY2 permits the peer's existing I2C0 reset/restart/two-byte preload.
LOW2 grants the IRQ-blocking read of31,4e;23tick DONE2 permits release; final
HIGH produces29tick DONE then monitor ownership. Both receipts require an
actual higher-priority task wake, checked cleanup and guarded buffer contents.
The HIGH/LOW dwell is sampled at1tick intervals for at least2ms, not continuous
observation. No lease is renewed and no callback is requested outside this flow.

Build with `RP1_RTOS_FEATURE=freertos-r2-i2c-peer tools/build-freertos-r1.sh /new/out`.
This workload requires the native host protocol/terminal-evidence join, not
just seeing final HIGH (which could be fail-close). On2026-09-11, selected source
f2d0116816d8753f35fd5fbc509c071e2c5d5c80/image113e9427ba99f84a7ecd48c6753d65e1bae1ca0ad9c5fd0225bf9a88a760e77d
passed formal2/2: one NACK IRQ wake then real31,4e RX with2IRQs/one blocked-task
wake, checked cleanup/canary and continuing kernel-test tasks. Payload ISR maxima
301/302us, IRQ-end→task52/48us, observed MSP<=912B, task unused190/512words.
Earlier cold-input admission failed because common R1 startup already configured
the pin; reuse that handle, as the current source does. The failure and separate
normal recovery remain distinct. No ESP or Linux image changes occurred.
These bounded cohorts do not prove arbitrary-slave recovery, hard physical lease
latency, I2C timeout/cancel, integrated R2,30min operation or an IMU acquisition rate.
