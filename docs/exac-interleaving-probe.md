# EXAC v7 candidate disassembly review (2026-10-23)

Build only. No device files or hardware were accessed. Runtime outcomes remain unverified.
This review was written before changing the ELF admission pins.

Candidate: `20261023-012010-codex-exac2-probe/fw-01/RP1.elf` under the experiment
log root, ELF SHA256 `ac3e3c6add5b13803d6dfc9fd6cf06bbdecaedf687747caf1cefa0be7b864070`.
The first build compiled and failed admission at the old exclusive-count assertion.
Disassembly was inspected with `arm-none-eabi-objdump -d -M reg-names-raw`.

Reviewed no-hooks mode (`1`,`1`) pins:

| Region | SHA256 |
| --- | --- |
| `.amp_proc1_profile_text` (1444 bytes, 0x2000f000..0x2000f5a4) | `3002f662c9bdf729e9cdc71b993d5761adf66b9aeea3e484d2b9cd360fa84b9a` |
| `AmpProc1Body` checker instruction text | `072738b6156a5c1b793f9772a5df3e61c7ff7b3c429a9c591fb7d92c4fd4ee91` |

`AmpProc1Body` normalized instructions match predecessor fw-09 exactly after resolving
relocated branch targets and removing instruction encodings, symbol hashes and outline
numbers. Its 608-byte local reservation and saved-register prologue remain unchanged.
The profile section still fits the existing 0x600-byte limit; no memory budget is enlarged.

`lib_probes` (0x2000f234..0x2000f3f4):

- Reads CTRL0 through DONE0-24; the only reset store is mask 64 to 0x40017000
  at 0x2000f254. EXAC accesses follow the DONE bit6 and both owner-encoding gates.
- EXAC literal pool contains only word6=0x40400300 and word7=0x40400380. Stores
  use those two words, shared result/mailbox SRAM, or the local stack. No watchdog,
  clock, GPIO, other EXAC word, or additional MMIO write is introduced.
- Reset wait 0x2000f26a..0x2000f27a decrements a 1025-initialized counter (1024
  polls) and checks unsigned raw-timer elapsed >9999. The three-trial outer loop
  is bounded by command<=3. Ack wait 0x2000f37a..0x2000f38c decrements a
  100001-initialized counter and checks elapsed >9999. Wrapping subtraction is used.
- Exactly two static LDREX, two STREX and one CLREX instructions. The baseline pair
  is 0x2000f2ba/0x2000f2be. The trial LDREX at 0x2000f356 precedes the command
  store at 0x2000f366. Until STREX at 0x2000f3a8 there is no intervening LDREX,
  CLREX, function call, indirect branch, or interrupt enable; only direct bounded
  branches, register operations, normal SRAM/timer accesses and DMB occur.
- Success records status/final and advances a three-bit completion mask. Timeout
  records trial/outcome=4 and bypasses STREX. Both paths withdraw command at
  0x2000f2e2, DSB/ISB, CLREX, store 15 to both words, DSB/ISB, record cleanup.
- C1 prologue saves 36 bytes; `lib_probes` is a leaf. No new indirect branch exists.

`lib_probes_c0` (0x2000f3f4..0x2000f4b0):

- Command is range-checked to 1..3 before address arithmetic or EXAC access, and
  ack>=command returns. Target can be only word7 or (command3) word6. Result row
  stores resolve to 0x2000f68c..0x2000f6e0 with eight-word row stride.
- Exactly one static LDREX/STREX pair, zero CLREX. Command2 accesses word7 with
  pair at 0x2000f43c/0x2000f440. Commands1/3 use normal LDR/STR at 0x2000f45a/5c.
- Each path has MRS PRIMASK / CPSID i, just its access and DMB, then MSR PRIMASK
  (0x2000f436..48 or 0x2000f454..62). No loop, wait, call or results publication
  occurs while this new mask is held. Saved mask register is distinct from inputs
  and output registers, including the STREX status register.
- After reading results, command mismatch releases the selected word with 15 at
  0x2000f48a, then DSB/ISB precedes ack. This handles a late access after timeout
  withdrawal. It does not prove an indefinitely stalled C0 has finished cleanup.
- C0 saves 32 bytes; it is a leaf with only `bx lr` as an indirect return.

`lib_probes_init` (0x2000f4b0..0x2000f4d4) clears exactly 128 bytes at
0x2000f680..0x2000f700, with +4 increment and offset==128 termination, before C1
release. No exclusive instruction or MMIO access occurs there.

`probe` retains its SysTick-idle gate, CSR/RVR/CVR writes, 64/32-word bounded
record clears and raw-timer/50000000-iteration calibration bound. Relative to
fw-09 its source change is VERSION 6 to 7; the direct `lib_probes` branch relocates.
The sealed diagnostic contains no new indirect calls. Existing IRQ/helper checks
and stack admission remain enabled; their full checks must pass on the final build.

The only new peripheral address admitted by the checker is 0x40400300 in the
already sealed C1 lib_probes scope. Exclusive instructions remain prohibited
outside the two exact reviewed functions; their counts become C1=(2,2,1), C0=(1,1,0).
SCMI handler remains at 0x20000d88 (Thumb vector 0x20000d89).

Main-session hardware checks: completed PRF1/v7 + EXA7, DONE bit6, baseline
(15,0,1), all three acknowledgements, expected C1 statuses (1,1,0), C0 pair
status 0, C1 final values (0,0,1), both cleanup values 15, no timeout/refusal,
and the complete existing Z2 cohort audit. A different control result is measured
evidence to investigate, not a reason to relabel the run successful.
