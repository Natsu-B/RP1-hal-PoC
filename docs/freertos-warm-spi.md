# Selected warm SPI owner (AW)

Build with `RP1_RTOS_FEATURE=freertos-r3-watchdog-warm-spi bash tools/build-freertos-r1.sh OUTPUT`.
The normal build fixes the existing compiler/toolchain/official FreeRTOS pin,
uses O3/fat LTO and verifies the reviewed image identity. This is a BUILD
candidate, not a hardware-verified release.

Cold epoch keeps the watchdog/R1 path. After the checked expiry/Reset entry,
fresh proc0 kernel state owns one SPI0 task in slot7 (512 words); seven R1
tasks remain, total2304/2560 words. No main/PCIe replay or UART data owner.
The existing AV PLL/UART preparation is reused unchanged, then only the known
SPI0 bank1bit10 release and established GPIO/100kHz host contracts are used.
Initial reset/DONE, quiet body, version, route and NVIC guards reject retained
or unknown states before setup. IRQ19/vector35 has priority6 and calls the
existing bounded FromISR adapter; no new scheduler or interrupt dispatcher.

The task blocks for the existing ESP32 low/hi-Z peer; two actual frames
69963c01/02 must complete via IRQ19/IPSR35 with matching generations,
canaries, checked finish, quiet state and stale-cancel refusal. TypeF is emitted
only after both frames, at least five full R1 progress checks and task/MSP
free-stack guards. E60..65 are preparation refusals; E68 is bounded missing
owner; E69 revokes post-F readiness. Late diagnostics use saved fresh EPOCH,
not the cleared/reused early telemetry region. Any post-F suffix fails the
external validator, including a truncated/overflowed diagnostic.

Telemetry124..159 holds AW preparation and two transfer receipts;160 owner
watermark. Watchdog96..123 and fault184..255 remain reserved. ISR/task stack
peaks require hardware measurement. Receive has a50-tick deadline; raw timer
elapsed includes setup/finish/cleanup and is not a strict50ms bound.

Use the existing0.6.0 finite SPI peer only under sole hardware lock, with fresh
ESP backup, app-sector-only update/readback and exact fresh-backup restore.
No post-ACK RP1 MMIO is needed: captured WQ9 handoff schedules peer stimuli;
the independent real peer records and F frame must both pass. Host procedures
and raw evidence stay in the private evidence repository, not this source tree.

OPEN: selected warm hardware state/payload/timing, measured stack peaks and
formal2/2. Not in-flight SPI recovery, warm integrated SPI/I2C/UART, full R3,
PCIe recovery, DMA drain or autonomous watchdog feeding.
