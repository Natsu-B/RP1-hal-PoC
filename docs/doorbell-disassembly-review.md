# Doorbell disassembly review before checker admission

Candidate: task output `fw-03/RP1.elf`, built 2026-10-23. Build exit 1 is
expected at this stage: the old checker rejects the newly file-backed inactive
stack reservation. This is not a successful checked build.

Reviewed `arm-none-eabi-objdump -d -M reg-names-raw`, ELF sections/segments,
the entry/park/session blocks and a normalized comparison against relaunch
`fw-06`. No hardware was accessed.

* `.amp_proc1_stack`: PROGBITS/AX, 2064 bytes, 0x2000bf40..0x2000c74f.
  Low/high guards remain at 0x2000bf40/0x2000c748. Code starts at 0x2000bf48;
  the copied handler is 0x2000c0f8..0x2000c15f (104 bytes, including pool).
  Padding after 0x2000c160 is unreachable. The existing guard initialization
  remains; the 512-word shared-stack fill is removed only in this diagnostic.
  AmpProc1Entry still switches MSP to 0x10002800 before entering Rust.
* `park`: original stop/output checks and status stores precede CMP state,2.
  Only the equality branch calls AmpProc1DoorbellStart at 0x2000bf48. All
  other states retain DSB/WFE. No change to the control loop is introduced.
* Setup: zero loop is exactly 16 words at 0x2000f600..0x2000f63f; handler copy
  is 26 aligned words to 0x10000000..0x10000067; vector copy is 80 words to
  0x10002800..0x1000293f, then only word16 is replaced with 0x10000001.
  PIC proof: source and destination are four-byte aligned; every handler PC
  literal points inside the copied 104 bytes; every non-return branch stays
  within that block. There are no calls or callee-saved-register writes in it.
* New peripheral stores: only 0x400d2110=0x200 (INTE SET), 0x400d3110=0x200
  (INTE CLR), 0x400d3114=0x200 (INTF CLR). TIMER low, INTS, GPIO9 CTRL are
  reads only. There is no GPIO data/CTRL store in the new code. New core-local
  stores are ISER0/ICER0/ICPR0=1, VTOR=0x2000be00 or 0x10002800, code/vector
  copies and record words within the declared ranges. No watchdog or reset
  write, DDR access, RPMsg call or new peripheral grant is introduced.
* Handle: first non-literal data read is TIMER low; DB[6] store precedes exact
  INTS==0x200 check. Normal path is INTF CLR, DSB, ack increment/store, DSB,
  compare against 10,000,000. Fault paths use only DB[7/8/15], PRIMASK and
  ICER0, then BX LR. BX LR is the normal call/exception return, not a new
  indirect call. The only new dispatch through a runtime address is IRQ0's
  VTOR entry, set to the fixed local Thumb address.
* Poll/WFI modes keep PRIMASK set. ISR mode uses a masked WFI/unmask/check
  interlock as documented in the protocol. All mode selection uses immediate
  compares and direct branches. Unknown/stop/limit/spurious paths restore
  VTOR, clear owned INTE/INTF, disable and clear IRQ0, publish state 2, WFE.
  Setup loops are bounded; only responder idle/service and final WFE loops
  are intentionally unbounded. Limit is on handled events, not elapsed time.
* AmpProc1Body's normalized instruction/literal sequence is unchanged; only
  BL encodings/targets moved by eight bytes after park gained its branch.
  Every profile function is unchanged after relocation normalization. In the
  existing endpoint_config_finish, three literal pointers moved +8 with
  their original targets (0x2000992c/3c/78 → 0x20009934/44/80). These are the
  only normalized deltas in the profile section. IRQ text shrinks by 20 bytes
  after removal of the unused shared fill; GPIO IRQ helpers remain identical
  under the existing sealed-X comparison.

Pins admitted only for (local_stack,no_hooks)=(1,1):

| Object | SHA256 |
| --- | --- |
| AmpProc1Body opcode rendering | 501a9c131bed3096fe93f0e23c5ddb268fbdeb7cb91475dc5b9af36cb30fbaa0 |
| Profile section bytes | e7a150437ee21275bf8b3cf7e015233e44bf8018f353b849086a9e77a1ed0d3f |
| Inactive shared-stack section bytes | 0cc39bd1ec9db6259203df70f0dc8dcba5af040c2f737bd3867daafb72227a68 |
| AmpProc1DoorbellStart opcode rendering | 26e47b5b2a68fe6f5f3513ff0cb768b76d814d8a5752b66e6860ba48e75d0c63 |
| AmpProc1DoorbellIrq opcode rendering | f50dc120eb94adc7193c3ea819ec5f41e09ab125c34cb41eec672ad78f4d7c59 |

The whole source section pin includes all literal pools, guards and padding;
per-function pins also protect synthetic disassembly mutation tests. Other
feature profiles retain their original pins and stack section type. Hardware
latency, real interrupt delivery, stop/limit behaviour and cohort success
remain unverified until the main session runs them.
