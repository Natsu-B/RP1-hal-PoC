# OpenAMP build audit

`tools/build-openamp.py /new/absolute/output` builds the exact clean source
checkouts in `$CM5_HACK_ROOT/tools/openamp-lock.json` (default root
`/opt/rpi-cm5-hack`). `--lock` selects another lock file. It does not fetch,
modify upstream sources, link firmware, or access hardware.

The Cortex-M3 toolchain uses Thumb, software floating point, and function/data
sections. Both static archives are built from upstream CMake. Assertions stay
enabled even with size optimization. The output contains the input lock,
effective options, compiler version, archive/source hashes, disassembly,
architecture attributes, undefined symbols, and `audit.json`.

This is an **upstream Generic/template build, not a qualified RP1 port**.
Template IRQ, cache, sleep and timestamp hooks are incomplete. Its mutexes use
exclusive instructions. Do not link these archives into the existing firmware
and assume that locks or cache/coherency are safe. The eventual port needs one
OpenAMP task owner, bounded Linux-provided queue/buffer mappings, and the proven
DDR ordering/completion hooks. RPMsg and virtqueue implementation remain upstream.

Verified output on 2026-09-29:
`/home/hotaru/rpi-cm5-hack/artifacts/20260929-openamp-resume/build/upstream-02`.
Both archives have ARMv7-M attributes, no compiler warnings, and assertions
enabled. Exclusive instructions remain in 2 libmetal and 19 OpenAMP functions,
including RPMsg endpoint/send/receive paths. These are audit findings, not
runtime failures observed on RP1.

The first hardware gate is recorded in the canonical
`/opt/rpi-cm5-hack/reports/openamp-rpmsg.md`. Actual RPMsg communication remains
unestablished: SRAM polling does not guarantee the synchronous selector/bank
semantics required by standard Linux virtio-mmio. Building libraries does not
close that transport gate.
