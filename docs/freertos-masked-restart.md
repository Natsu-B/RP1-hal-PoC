# Selected masked-pending watchdog restart

Opt in with `RP1_RTOS_FEATURE=freertos-r3-watchdog-kernel-restart-masked`
and run `bash tools/build-freertos-r1.sh OUT`. This preserves the strict WDT8/9
features and official FreeRTOS ARM_CM3 port; no Linux modification is involved.

The AS experiment independently captured guard E23 twice: NVIC bank1 pending
bit21, with all interrupt enables and active bits zero. The new policy accepts
only this disabled/inactive pending bit (or zero), never enables or acknowledges
its unidentified peripheral source, and checks its disabled/inactive state on
every warm monitor pass. All other startup guards remain fail-closed.

Arm DUI0552A section4.2.2 defines disabled interrupts as unable to activate even
when pending. That is the contract used here, not a claim identifying IRQ53.
https://documentation-service.arm.com/static/5ea823e69931941038df1af5

The WDT9 reader and packet format are unchanged. An E packet is a failed restart,
not success. C is emitted only after five fresh monitor passes with the existing
kernel/context/data checks. Fresh RTOS start is not PCIe reinitialization,
peripheral recovery, autonomous health feeding, or full R3 completion.

`check-masked-restart-elf.py --self-test ELF` pins the reviewed linked image and
uses existing ELF/vector/probe checks. A changed image requires new review; a
matching hash alone is not hardware evidence. The normal build also executes
the pure masked-pending acceptance/rejection tests.
