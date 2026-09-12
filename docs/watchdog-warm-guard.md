# WDT9 warm-start guard diagnostics

Opt-in feature `freertos-r3-watchdog-warm-guard` inherits WDT8's fresh-kernel
experiment and keeps every admission predicate. The ARM cookie, startup record,
checked watchdog disable, cold-data token, shadow checksum and BSS reset stay.
WDT9/WQ09/QA09 distinguish the host/firmware protocol; default modes stay opt-in.

One type-E packet reports a rejected guard: type4/nonce16/code8/XOR4.
The code is NOT watchdog REASON. E is terminal and cannot mean fresh-kernel PASS.
Codes: 1=data prepare, 2=reason, 3=IPSR, 4=PRIMASK, 5=ICTR;
10/11/12=bank0 enable/pending/active, 20/21/22=bank1;
23=bank1 pending exactly00200000 with both banks enabled/active zero;
30=SysTick CTRL,31=ICSR,32=CFSR,33=HFSR,34=shadow info (codes hexadecimal
except the identical single-digit values).

The pre-BSS reporter validates WENT+nonce/checksum, masks IRQs, uses inherited
GPIO22 SYS_RIO_OUT at400e0000 and bounded raw timer reads at400ac028 only.
It neither obtains the HAL singleton (which writes BSS) nor calls RTOS APIs.
Missing/invalid cookie, earlier capture failure or failed GPIO/timer retention
remains silence, not a diagnosis. Hold loops stop after10M samples; failures
lower GPIO and halt. One E replaces C: capacity192 is not enlarged.

Type C remains the unchanged source-gated five fresh monitor-pass witness.
Neither C nor E externally samples all counters, proves PCIe/peripheral recovery,
or completes R3. No Linux kernel/config/source/module change.

Build with the normal `tools/build-freertos-r1.sh`, setting
`RP1_RTOS_FEATURE=freertos-r3-watchdog-warm-guard`.
The official kernel/port and guarded Rust/GCC versions remain pinned.
`check-warm-guard-elf.py` uses an exact reviewed candidate image hash and
the existing generic ELF/probe checks; source or compiler changes require new
review, not automatic re-blessing. Host `test-warm-guard.py` covers live
one-based ESP sequence numbers, complete frames and refusal cases.
