# Mixed I2C notification acceptance

`notification_give_from_isr_woken()` returns `Result<bool, Error>`. The boolean
is FreeRTOS's `pxHigherPriorityTaskWoken`, not notification delivery success.
The pinned kernel `3a22924e` increments the notification value before checking
whether the recipient was blocked and outranks the interrupted task
(`tasks.c:8353..8406`). The bridge returns this scheduling hint after calling
`portYIELD_FROM_ISR`; it does not discard a notification when the hint is zero.

The mixed I2C owner must therefore not assert a positive hint for every read.
Equal-priority concurrent owners and completion before blocking are valid cases.
The driver API, IRQ handler, priority table and kernel remain unchanged.
Per-request acceptance retains generation, IRQ8/IPSR24, exact payload/tail,
canaries, fatal/abort/discard checks and checked cleanup before DONE/ACK.
The host validator requires a monotonic hint counter, bounded by request count,
and at least one higher-priority wake during the complete workload. It does not
claim that every transaction caused an immediate preemption.

Caller audit: UART overflow already permits zero hints when receipt precedes
blocking; standalone I2C/UART examples with deliberately isolated high-priority
waiters retain their stronger selected wakeup tests. The mixed SPI/UART worker
only accumulates hints. No blanket weakening of controlled IRQ-wakeup proofs.

Run `tools/test-i2c-mixed-pair.py`, `tools/test-freertos-i2c-mixed-pair.py` and
`tools/test-freertos-i2c-mixed-stream.py`. These check actual frame/queue code and
numeric acceptance of sparse hints, rejecting zero aggregate evidence,
overflow/regression and existing payload/deadline/canary faults. Hardware
commissioning/repeat of this changed image remains required.
