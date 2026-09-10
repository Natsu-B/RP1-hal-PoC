# Proc0 FreeRTOS R1 integration

Status: selected tick/context/task-synchronization HW2/2 on source68e55d54,
not yet a full R1 release; no R2/R3 completion claim. Evidence repository commit
c43964472ec0d3128a61f6fff50007ee9b42e96f records the two-member cohort.

The `freertos-r1` feature reuses this example's selected endpoint-clock,
state1/2/3/5 startup unchanged. Only after LinkUp does it create seven permanent
tasks and start the official FreeRTOS GCC ARM_CM3 port. No runtime PCIe reinit,
SMP, proc1 kernel, dynamic allocation, tickless idle, or Linux dependency.
The older optional ISRAM/DSRAM PCIe initializer is retained without modification;
it is not enabled by this feature.

## Reproduce

Initialize `third-party/FreeRTOS-Kernel` at the committed gitlink. The kernel is
official V11.3.1, commit `3a22924e0a9ddbbc8b0758881c33b3422a5cc20d`, MIT licensed.
Build uses Rust `1.96.0 (ac68faa20 2026-05-25)`, GCC ARM `14.2.1`, target
`thumbv7m-none-eabi`, ARM AAPCS soft-float; C `-Os`, Rust release optimization,
one codegen unit, panic abort. A changed compiler/kernel is explicitly rejected.

```
bash tools/build-freertos-r1.sh /absolute/new/build-directory
```

The script records source/index diffs, ELF, map, symbol/disassembly output and
the executable ELF validator. Do not deploy an ELF rejected by that validator.
The ordinary bootloader places PT_LOAD file bytes at p_paddr=VMA and zeros the
materialized image; Reset additionally zeroes BSS. No separate data LMA exists.
The nonzero DATA_SENTINEL and zero BSS_SENTINEL are checked before scheduling.

## Memory and ownership

| Proc0 local address | Owner / contract |
| --- | --- |
| 20000000..2000dfff | Code, vector, initialized data, BSS, static TCBs/task stacks/queues |
| 2000e000..2000efff | 4KiB boot/ISR MSP, canary-filled before any Rust prologue |
| 2000f000..2000f7ff | Reserved; legacy endpoint marker locations are not overwritten |
| 2000f800..2000faff | R1 telemetry (192 u32 words) |
| 2000fb00..2000fbff | Stack-independent exception record |
| 2000fc00..2000ffff | Reserved legacy debug/official mailbox region, no R1 service |

VTOR[0]=2000f000, also the loader SP. SVC/PendSV/SysTick vectors point directly
to the official naked port functions; no ABI wrapper. PRIGROUP=0, three
implemented priority bits, kernel priority7/e0, max syscall priority5/a0.
R1 owns SysTick only; external peripheral IRQ FromISR tests are R2 work.
No old expected-fault PC-advance handler is used. Faults halt, not recover in
place. The naked fault handler records MSP/PSP/EXC_RETURN/IPSR/CONTROL and fault
registers without a stack; exception frame reads are SRAM-range checked.

Task stacks and all referenced objects have static lifetime. R1 hooks only
write fixed telemetry words; no kernel API, allocator, mutex or output in hooks.
Each task has one writer for its changing words; aligned host u32 reads are
required. A whole telemetry page is not a coherent atomic snapshot.

## Workload and acceptance boundaries

Seven tasks: high-priority monitor4; two non-yielding assembly spinners1;
consumer3, producer2; mutex-low1 and mutex-high3. The spinners preserve and
continuously check literal R4-R11 patterns without a single call/yield. The
monitor sleeps1000 ticks and requires both spinner counters, queue sequence,
and mutex completion to advance. Queue-full/empty/deadline, invalid capacities,
notification+semaphore sequencing and actual priority inheritance are checked.
These assertions describe planned observations, not hardware facts.

SysTick processor-clock ticks are first compared against the established raw
1us timer over five10ms windows. The measured frequency, spread and selected
integer1kHz reload are recorded; >1% spread/stuck counter rejects startup.
This is not a crystal-accuracy guarantee. Tick min/max intervals and elapsed
raw time are recorded independently. No DWT/TENMS-based timing claim.

Initial telemetry (`u32` indices relative f800): 0 RT01 magic;1 version;2 stage;
3/4 fault reason/detail;5 CPU Hz;6/7 calibration min/max;8 ticks;9 switches;
10 last task;11 last tick raw time;12/13 min/max tick interval;14 monitor cycles;
15 consumer completions;17 inheritance count;18 MSP bytes touched;
19/20 initial data/BSS;21 AIRCR;22 reload;23..25 observed SYS control/div/SEL;
26/27 run raw times;28..31 monitor IPSR/CONTROL/PSP/MSP (first task in the7-task image);
32..38 minimum untouched task stack words;39 queue checks;48/49 sent/received;
50 mutex completions. Spinner blocks64 and80 contain counter, initial
IPSR/CONTROL/PSP/MSP and error latch at block+6. Stage5 means the monitor has
completed at least one interval; ffffffff is an assert/panic. fb00 magic RFT1
indicates an exception record. Neither stage5 nor a pair of GPIO edges alone is
the full R1 proof.

Selected formal same-image2/2 and independent external GPIO observation passed.
Remaining: intentional fault/recovery validation, hardware exception-frame/register
distinction beyond the current pattern test, ISR task wakeups, tick-wrap race
tests, and 10boots/30min endurance. SPI/I2C/UART adapters, 200us absolute periodic
work and R3 remain separate work. No servo/IMU/camera measurement is implied.

## Deliberate fault candidates (not unattended firmware)

Set RP1_RTOS_FEATURE=freertos-r1-fault or freertos-r1-panic when invoking the
same build script. They are mutually exclusive opt-in test images, not part
of normal freertos-r1. After five monitor cycles, the first enables architected
UsageFault and executes UDF with known R0-R3/R12 values; the second calls the
ordinary Rust panic path. Both halt for reserved-record inspection, never
skip the fault instruction, restart in place, or write RP1 reset/POWER bits.
Hardware/restart validation is separate from building these candidates.

## TIMER FromISR candidate

RP1_RTOS_FEATURE=freertos-r1-timer-irq adds slot7 (task ID8), priority5, to the
same seven-task workload. It owns previously proven TIMER0 ALARM0/IRQ26 only,
with NVIC priority6/c0 and the existing vector ABI; legacy setup routines that
change VTOR/global masks are not called. The official SysTick remains1kHz.
Each real alarm masks/acks its source, publishes generation/timestamps, calls
notification_give_from_isr, and requests the official port yield. The next
switch must select task8; the task blocks for completion, never timer-polls.
The initial absolute deadline period is20ms, not a200us/5kHz acceptance claim.

Telemetry96..127: IRQ/wake/error counts96..98; IPSR99, priority100; initial ISR
PRIMASK/BASEPRI102/103; IRQentry/end/task timestamps104..106; last/max IRQ-end
to-task latency107/108; max ISR body109; deadline110/max lateness111;
requested/IRQ generation112/113; missed schedule114; active119;
next-switch latch120/ID121; task8 free stack123; first task8 context124..127.
Whole host snapshots remain non-atomic. Finite timeout disables source/route
and panics; it does not write unproven ARMED disarm bits or claim in-place recovery.
