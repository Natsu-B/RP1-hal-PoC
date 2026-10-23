//! Post-completion diagnostic only; protocol and disassembly review are in docs/doorbell-protocol.md.
unsafe extern "C" { pub(super) fn AmpProc1DoorbellStart() -> !; }
core::arch::global_asm!(include_str!("doorbell.S"));
