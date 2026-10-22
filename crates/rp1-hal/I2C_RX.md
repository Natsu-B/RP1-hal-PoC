# Bounded I2C1 RX foundation

`i2c_rx_state` and `i2c_rx_irq_adapter` promote the existing selected multi-byte
IRQ RX engine into the normal HAL. RX credit limits outstanding commands to
observed depth, STOP is only on the final command, fatal/abort takes precedence
over final-byte completion, and stale/late generations cannot become success.
No dynamic allocation or proc1 exclusive-monitor lock is used.

`RxState::arm(expected)` retains the original proof's exact-payload check.
`arm_read(length)` receives unknown real bytes with the same1..33byte bounds
and all source/geometry/generation/cleanup checks. Inspect `bytes()` only while
holding the same owner/IRQ serialization. These APIs perform no hardware by
themselves; READ/POP permits must be followed by an actual access then its record.

ARM-only host methods enable RX_FULL/STOP/fatal mask0x24f, issue a bounded read
command and guard one FIFO pop. `enable_local_irq_route` uses only the selected
RP1 wrapper+0x108=1 contract with controller disabled/IMR0/ownedNVIC masked;
unknown readbacks reject. It does not discover IRQ numbers or reset the bus.
LocalIRQ8/IPSR24 is distinct from the host interrupt namespace.

The IRQ adapter is **not** a checked lifecycle or FreeRTOS driver. The caller
still owns clock/reset prerequisites, IRQ priority, serialization, deadline,
total/no-progress limits, source-first evidence, checked cleanup/quiet interval,
task notification and buffer lifetime. Do not copy proof-wide PRIMASK/VTOR/NVIC
replacement into a scheduler. `record_cleanup` verifies supplied evidence; it
does not mask, disable, ACK, discard, clear pending or observe physical quiet.

STATIC tests cover geometry/credits/STOP order, late/fatal/timeout/rearm, cleanup
rejection, unknown payloads and exact wrapper readback selection. An ARM compile
is BUILD only. The promoted/new source must receive its own RTOS HW cohort;
historical bare-metal RX2/31/32/33/NACK cohorts do not prove this integration.

`CARGO_INCREMENTAL=0 cargo test --offline --locked -p rp1-hal --target x86_64-unknown-linux-gnu`
and `cargo check --offline --locked -p rp1-hal --target thumbv7m-none-eabi` run
the foundation checks without a hardware session.
