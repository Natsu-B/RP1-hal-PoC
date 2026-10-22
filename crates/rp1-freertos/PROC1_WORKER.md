# Proc0 FreeRTOS with a peripheral-free proc1 worker

Opt-in example feature `freertos-r3-proc1-worker`; BUILD candidate, HW OPEN.
Proc0 retains the seven R1 preemption/context/queue/notification/mutex tasks;
task8 owns one finite proc1 transaction stream. Proc1 has no FreeRTOS kernel,
peripheral, SysTick or IRQ ownership. This is not SMP or OpenAMP.

```
RP1_RTOS_FEATURE=freertos-r3-proc1-worker \
  tools/build-freertos-r1.sh /absolute/new/build-directory
python3 -B tools/test-freertos-elf.py /absolute/new/build-directory/RP1.elf
python3 -B tools/check-freertos-proc1.py
```

The official kernel/toolchain pins and normal startup/PCIe initialization remain
unchanged. Proc0 starts the owner only after scheduler startup. No pointer into
the boot/MSP stack is retained. Static task pool remains2560 words; the seven
normal tasks plus512-word owner use2304 words. MSP stays2000e000..2000f000.

The normal `rp1-rt` linker includes dedicated symbol-relocatable proc1 sections
only when selected. App/code/PT_LOAD must remain below2000e000. Proc1 has a
512-byte-aligned80-entry vector, separate initialized word/BSS word, lifecycle,
5-word request,12-word response,fault slot and2048-byte stack with two8-byte
guards. These sections are outside `__sbss..__ebss`; no reinitialization of the
proc0 kernel on proc1 launch or cooperative RESUME. Existing PCIe local SRAM
layout is untouched. The checker rejects external calls/tail branches from
proc1 code, exclusive instructions and allocator/unwind helpers.

Launch reuses the selected P1R1 contract: check proc0 identity and held proc1
reset before touching proc1 storage/scratch; write known magic/SP/encoded entry;
recheck and deassert only known direct bit31 once. No reset/retry/reclaim of a
live or timed-out worker. Cross-core exchange uses aligned volatile u32 words,
one outstanding token and DMB publication/token-last; no Rust shared references
or cross-core kernel/atomic locks. Proc1 WFE always rechecks its token after SEV.

Proc0 response observation blocks for1 tick between checks, at most200 checks
and100ms raw-time deadline per wait. Deadline is checked before accepting a
response. This is bounded delayed observation, **not** proc1 interrupt-driven
task notification. WORK/PAUSE/RESUME, stale/duplicate/invalid requests across
three epochs reuse the twelve-transaction model. RESUME resets only two
proc1 application words. Terminal PAUSE keeps the worker alive without further
requests; the owner checks guards/fault state once per second.

RT01 telemetry words96..159 contain P1R1 ABI **v2** (word1=00400002).
The old mailbox at2000fc00 is not used by this adapter. V2 word56 records a
2000-tick post-service delay, not the old foreground4096-iteration loop;
word20 counts finite owner observations. The response trace/canary/epoch fields
retain their meaning. Terminal word63 is published last and remains immutable.
Words160..163: owner IPSR/CONTROL/PSP/MSP;164..167: before/after tick/switch;
168: continuing owner heartbeat;169: owner minimum free stack words;
170..171: raw-low service interval;172: observed proc1 stack bytes used.
R1 telemetry and reserved fault area184..255 remain separate.

Require the same-image R1 validator **and** proc1 validator, exact selected ELF
identity, external GPIO progression and normal recovery. Host synthetic tests
are not HW evidence. Hardware reset/restart, proc1 fault recovery/peripheral
IRQs, second RTOS kernel and Linux DDR remain separate OPEN requirements.
