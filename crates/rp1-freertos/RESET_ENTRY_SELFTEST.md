# Software Reset-entry identity — unverified candidate

Feature `freertos-r3-reset-entry-selftest`, paired observer
`rp1-rtos-reset-entry-selftest`, WDT6/WQ06/QA06. Not watchdog expiry/restart.
Retain the established256us short watchdog probe and explicit disabled receipt.
Host chooses a nonzero16bit diagnostic nonce from the live monitor timestamp;
request/ACK checksums cover it and firmware binds ACK to saved request nonce.
Formal captures must have distinct nonce; this is correlation, not authentication.

After final ACK/current-map NCR gate and host WFE, GPIO22 owner verifies watchdog
still disabled, publishes WARM cookie last and branches to existing Reset with
IRQs disabled. No further WDT/reset/POWER or boot-scratch write. This explicitly
controlled software branch cannot prove a hardware reset domain or autonomous
restart. No post-ACK host RP1 reads.

Reset installs its separate MSP as before, then captures valid WARM before BSS
clear. The4word WENT record retains nonce/raw32bit REASON/checksum at R1 telemetry
136..139=0x2000fa20..fa2f (basef800), outside ISRstack and panic184..191/fault192..255.
The WDT feature excludesproc1; this space is not promised to other debug ABIs.
After BSS/vector initialization, positive capture emits one typeB32bit GPIO
packet and halts BEFORE optional PCIe initialization or application/kernel entry.
Warm.data restoration and peripheral continuity are still OPEN; do not silently
run the kernel again with retained mutable initialized state.

Reporting uses known GPIO22 HAL setup, raw timer400ac028 and read-only watchdog
REASON40154008. No RTOS delay/allocator/mutex/formatting. Per-hold iteration bound
and complete frame/delimiter checks classify timer stalls/missing packet as failure.
Rawreason>255 is refused instead of truncating the transmitted field. The4word
record keeps its full32bit value. Packet typeB starts with ONE to avoid AN's boot
ZERO false start.32bits contain type4,nonce16,reason8,XOR4; not cryptographic.

Six pure model tests plus synthetic full receipt/trace refusal tests are in normal
source. Build via tools/build-freertos-r1.sh, with target in fresh RAM directory.
Before any HW: pin sources/builds, inspect reset-hook ELF and memory/IRQ paths,
independent audit, known-good preflight, commissioning, formal2/2 and separate
normal recovery. No BUILD/HW success is claimed by this candidate documentation.
