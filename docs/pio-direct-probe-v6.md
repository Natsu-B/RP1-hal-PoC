# C1 direct PIO probe v6 (G9, build only)

Feature combination: `freertos-amp-observation` plus
`freertos-amp-proc1-profile freertos-amp-proc1-local-stack freertos-amp-proc1-no-hooks`.
Runs on C1 before READY. This is a diagnostic, with hardware validation still OPEN.

Register contract: Raspberry Pi utils `proc_pio.h`, revision
`65bad738fb9b40d19659f210705fd629db4fc7b0`, inventoried in
`/opt/rpi-cm5-hack/artifacts/20260905-023759-rp1-pio-complete-register-map-static/`.
The official CLKDIV.INT description specifies `sysclk/int`: INT=1, FRAC=0 gives
the current clk_sys frequency. The probe leaves the clock tree unchanged.

## Ownership and exact writes

Linux PIO must be disabled. Read RESETS CTRL0 0x40014000 and DONE0 0x40014018.
Refuse if CTRL0 bit27 is already clear: instruction memory is write-only, so
an existing program cannot be preserved. Otherwise release only bit27 using
0x40017000=0x08000000, then poll DONE0 bit27 at most 100000 times. If it never
sets, restore with 0x40016000=0x08000000 without accessing PIO. Bit27 is
PIO_APBS; its relationship to the local PIO core remains a hardware hypothesis.

At local PIO base 0xf0000000, refuse unless CTRL[11:0]=0, FLEVEL=0, and
FSTAT=0x0f000f00. Save SM0 CLKDIV/EXECCTRL/SHIFTCTRL/PINCTRL/PC.

| Address | Probe write |
| --- | --- |
| 0xf00000e0 SM0_PINCTRL | 0 (no side-set) |
| 0xf00000cc SM0_CLKDIV | 0x00010000 (clk_sys / 1) |
| 0xf00000d0 SM0_EXECCTRL | 0x00003000 (wrap 0..3) |
| 0xf00000d4 SM0_SHIFTCTRL | 0 (manual push, no join) |
| 0xf000004c INSTR_MEM0 | 0xe035 (`set x,21`) |
| 0xf0000050 INSTR_MEM1 | 0xa0c1 (`mov isr,x`) |
| 0xf0000054 INSTR_MEM2 | 0x8020 (`push block`) |
| 0xf0000058 INSTR_MEM3 | 0x0003 (`jmp 3`) |
| 0xf0000000 CTRL | 0x110 (SM0/divider restart), then 1 (enable SM0 only) |
| 0xf00000dc SM0_INSTR | 0 (`jmp 0`, while disabled) |

Enable only after register/PC readback matches. Poll FSTAT.RXEMPTY0 at most
100000 reads, disable SM0, capture FSTAT/FLEVEL/SM0_ADDR, then pop RXF0
0xf0000024 only if nonempty. Expect 21, FLEVEL=0x10 and PC=3. The program
contains no GPIO or IRQ instructions, and no pinmux/pad registers are written.

Cleanup disables all initially idle SMs via CTRL=0, injects `set x,0` (0xe020),
restarts SM0/divider (CTRL=0x110), flushes SM0 FIFOs via SHIFTCTRL=0x80000000,
restores saved SHIFTCTRL/CLKDIV/EXECCTRL, injects `jmp saved_pc`, restores
PINCTRL, and zeros the four instruction slots. Verify readable registers/PC
and empty FIFOs before reasserting reset bit27 through its SET alias. Nothing
touches SM1..3 configuration, processor IRQ enables, GPIO, pads, or DMA.
Unknown pre-existing internal state cannot be reconstructed; the held-reset
ownership prerequisite and requested zeroing are essential to this experiment.

## Result ABI and capture

PRF1 VERSION=6; live STATE=2. Results use LOCAL words 16..31 at
`0x2000f600+4*i`, host BAR2 `0x1f0040f600+4*i`.

| Word | Value |
| --- | --- |
| 16 / 17 / 18 | RESET before / DONE before / DONE after release |
| 19 / 20 / 21 | PIO CTRL / FSTAT / FLEVEL before |
| 22 | SM0 CLKDIV readback |
| 23 / 24 / 25 | FSTAT / FLEVEL / PC after stopping SM0 |
| 26 / 27 | RX value (0xffffffff if unread) / FSTAT after pop |
| 28 / 29 | RESET / DONE after restoration |
| 30 | `PIO1` = 0x314f4950, published last |
| 31 | Status and cleanup flags |

Status low byte: 0 success, 1 already released, 2 reset timeout, 3 busy/nonempty,
4 config readback failure, 5 RX timeout, 6 wrong value/level/PC.
0x100 flags register/PC/FIFO cleanup failure; 0x200 flags reset restore failure.
Do not treat PRF1 live alone as a passing PIO probe. Unreached fields remain 0.

`tools/read-pio-profile.sh` copies the previous v5 reader's ranges unchanged;
only the main session may execute it on target Linux. It reads all 32 scratch
words, the 64 profile words, proc1 status and two RTOS snapshots. Decode on host:

```sh
python3 tools/decode-pio-profile.py /path/to/uart10.raw
python3 tools/decode-pio-profile.py --self-test
```

The decoder rejects old versions, missing/mixed/duplicate/incomplete records,
failure status, and inconsistent success data. It uses the latest read block.
v6 omits obsolete memory timing snippets to fit the unchanged 0x600-byte region;
PRF1 still provides the existing bounded timer calibration (word 8 cycles/us).

## Code-generation review and checks

Review performed before updating either checker pin, recorded in
`/opt/rpi-cm5-hack/logs/20261023-001240-codex-pio-probe/disassembly-review.md`.
The profile section is 0x59c bytes. `lib_probes`/`probe` add no indirect calls
or branches; stores resolve only to the listed MMIO, BASE/LOCAL, and stack.
The C1 body is instruction-equivalent to v5 after relocation/outline normalization.
Existing C0 `endpoint_config_finish` remains in the same sealed profile section.

Only mode `('1','1')` pins change:

- Profile section: `57f882072093f653e22f305757c76003fa531da5bef23e6704a2eb63b5e1f3e1`.
- C1 body: `06d632e8045b1f6dcfb314eabaa72dbb24842b23a6a00de38c95aa50bc6b88be`.

PIO literals are limited to 0xf0000004 inside this no-hooks probe; reset literals
to 0x40014018/0x40016000/0x40017000. The unused G2 exclusive-instruction
exception is removed; exclusive instructions are rejected everywhere.

Build logs: `/opt/rpi-cm5-hack/logs/20261023-001240-codex-pio-probe/`.
`fw-01` exceeded the code budget by 284 bytes; `fw-02` linked and was rejected
by the obsolete profile pin; `fw-03` passed every required build check (exit 3
with `AMP_proc1_build=PASS`). fw-02 and fw-03 ELF bytes are identical.

- Firmware SHA256: `73529c332a70ff340eb69d3a1d7a8caf8af748158b48571a6828683c95007161`.
- SCMI handler remains 0x20000d89 (Thumb pointer).
- Loader `loader-01` built successfully (exit 0), with kernel SHA256
  `254c04cb9c97a2dec4a9072539bc0fa31c506a1ca41772a9a76eefe8c28d13a3`.
  The 32-byte firmware seal occurs exactly once in the kernel at offset 0x2f420.
- Production X rebuilt without extra features to unchanged SHA256
  `2a6b897be9094e6087e797ef5939ec7a9a57a28149123967062e7707290ccec6`.
- Required ELF, vector, codegen, stack, remap and mutation checks passed.
- Host model runs the actual Rust probe body with 11 success/refusal/timeout/
  cleanup injections; decoder self-tests use synthetic captures. These do not
  establish hardware behavior. Failed attempts are retained in `failures.md`.

The main session must check DONE release, FIFO value/level/PC, cleanup flags,
reset restoration, and the full Z2 cohort. No hardware or deployment was performed.
