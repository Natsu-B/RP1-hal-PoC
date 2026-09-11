# UART0 task adapter

Build candidate: `RP1_RTOS_FEATURE=freertos-r2-uart tools/build-freertos-r1.sh /new/output`.
This selects the existing pinned official FreeRTOS GCC ARM_CM3 proc0 port,
Rust size optimization, static task8 (priority5,512words) and direct local
IRQ25/vector41 (logical priority6). No Linux, proc1, clock mediation or servo
claim. Hardware acceptance is separate from the build.

UART0 uses the existing50MHz XOSC/div1, PL01127/8 divisor and GPIO14/15 contract.
PLL_SYS primary phase and known UART0 reset release occur before scheduler start;
the task adapter never changes clocks/reset/VTOR/PRIMASK/BASEPRI.

`Driver::exchange(prompt,rx,timeout_ticks)` owns one UART and notification0.
RX arms before prompt TX; task waits for real IRQ notifications/deadline. A64B
driver-owned ring separates ISR storage from caller buffers. At most32FIFO words
are serviced per interrupt, with a finite entry/no-progress budget. Deadlines
are nonzero and less than2^31ticks. Error, timeout and cancellation all pass
checked RXE-off/drain/error-retention/ACK/quiet cleanup before buffer ownership
returns. Generation is published last and withdrawn first; exhaustion halts
instead of reusing an ancient cancellation tag. Delayed residual traffic cannot
silently complete the next request. Buffer copies and state borrows are under
IRQ25 mask and end before notification waits or unmasking.

`write_all` uses FIFO attempts and task delays, also checking serial BUSY idle.
It requires an earlier checked exchange, not just construction. It preserves
the last exchange receipt. No recovery is claimed from Drop. Cleanup failure
is halt-only and requires the established board recovery path.

The example sends READY, receives19actual peer bytes including sequence, compares
them and canaries, sends OK, then repeats once. Receipts at telemetry136/160 each
hold19words plus20payload/sentinel bytes. Header128 is RU01; task context124..127;
canaries184/185; finalclock186..189; priority190; completed generation191. Fault
record192 onward remains separate. `cleanup_elapsed_us` and `quiet_samples`
sum preflight and final cleanup (each>=4ms quiet); elapsed includes them.

Run `python3 tools/test-uart-rtos.py` for actual extracted Rust state/predicate
checks. These prove neither MMIO timing nor physical RX. Hardware timeout,
active cancel, physical overflow/error recovery, continuous RX and combined
SPI/I2C/UART same-image load are still separate required cohorts.
