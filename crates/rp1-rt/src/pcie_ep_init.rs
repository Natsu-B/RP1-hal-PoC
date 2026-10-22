use core::arch::{asm, global_asm};
use core::ptr;

const LOCAL_ISRAM_BASE: usize = 0x1000_0000;
const SHARED_MARKER_BASE: usize = 0x2000_f000;
const IO_BANK0_BASE: usize = 0x400d_0000;
const PADS_BANK0_BASE: usize = 0x400f_0000;
const UART0_BASE: usize = 0x4003_0000;

const UART_DR: usize = 0x00;
const UART_FR: usize = 0x18;
const UART_IBRD: usize = 0x24;
const UART_FBRD: usize = 0x28;
const UART_LCRH: usize = 0x2c;
const UART_CR: usize = 0x30;
const UART_ICR: usize = 0x44;

const UART_FR_TXFF: u32 = 1 << 5;
const UART_CR_UARTEN: u32 = 1 << 0;
const UART_CR_TXE: u32 = 1 << 8;
const UART_CR_RXE: u32 = 1 << 9;
const UART_LCRH_FEN: u32 = 1 << 4;
const UART_LCRH_WLEN_8: u32 = 3 << 5;

const GPIO_CTRL_F_M_DEFAULT: u32 = 4 << 5;
const GPIO_CTRL_FUNCSEL_UART0: u32 = 4;
const PAD_OD: u32 = 1 << 7;
const PAD_IE: u32 = 1 << 6;
const PAD_DRIVE_4MA: u32 = 1 << 4;
const PAD_SCHMITT: u32 = 1 << 1;

pub fn init() {
    unsafe {
        copy_and_call_relocated_init();
    }
}

unsafe fn copy_and_call_relocated_init() {
    unsafe extern "C" {
        static __rp1_pcie_reloc_start: u8;
        static __rp1_pcie_reloc_end: u8;
    }

    let src = ptr::addr_of!(__rp1_pcie_reloc_start);
    let end = ptr::addr_of!(__rp1_pcie_reloc_end);
    let len = end as usize - src as usize;
    let dst = LOCAL_ISRAM_BASE as *mut u8;

    uart0_init();
    uart0_put_marker(b'N');
    write32(SHARED_MARKER_BASE, 0xe5d1_0001);

    let mut offset = 0;
    while offset < len {
        unsafe {
            dst.add(offset)
                .write_volatile(src.add(offset).read_volatile());
        }
        offset += 1;
    }

    uart0_put_marker(b'B');
    write32(SHARED_MARKER_BASE + 0x004, 0xe5d1_0002);

    unsafe {
        asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
        let entry: extern "C" fn() = core::mem::transmute(LOCAL_ISRAM_BASE | 1);
        entry();
        asm!("dsb sy", "isb sy", options(nostack, preserves_flags));
    }

    uart0_put_marker(b'Y');
    write32(SHARED_MARKER_BASE + 0x020, 0xe5d1_0009);
}

fn write32(addr: usize, value: u32) {
    unsafe { ptr::write_volatile(addr as *mut u32, value) };
}

fn read32(addr: usize) -> u32 {
    unsafe { ptr::read_volatile(addr as *const u32) }
}

fn gpio_ctrl(pin: usize) -> usize {
    IO_BANK0_BASE + 0x004 + pin * 0x8
}

fn gpio_pad(pin: usize) -> usize {
    PADS_BANK0_BASE + 0x004 + pin * 0x4
}

fn uart0_init() {
    write32(gpio_pad(14), PAD_DRIVE_4MA | PAD_SCHMITT);
    write32(gpio_pad(15), PAD_OD | PAD_IE | PAD_DRIVE_4MA | PAD_SCHMITT);
    write32(
        gpio_ctrl(14),
        GPIO_CTRL_F_M_DEFAULT | GPIO_CTRL_FUNCSEL_UART0,
    );
    write32(
        gpio_ctrl(15),
        GPIO_CTRL_F_M_DEFAULT | GPIO_CTRL_FUNCSEL_UART0,
    );

    write32(UART0_BASE + UART_CR, 0);
    write32(UART0_BASE + UART_ICR, 0x7ff);
    write32(UART0_BASE + UART_IBRD, 26);
    write32(UART0_BASE + UART_FBRD, 3);
    write32(UART0_BASE + UART_LCRH, UART_LCRH_WLEN_8 | UART_LCRH_FEN);
    write32(
        UART0_BASE + UART_CR,
        UART_CR_UARTEN | UART_CR_TXE | UART_CR_RXE,
    );
}

fn uart0_put_marker(byte: u8) {
    let mut timeout = 0x10000;
    while (read32(UART0_BASE + UART_FR) & UART_FR_TXFF) != 0 && timeout != 0 {
        timeout -= 1;
    }
    write32(UART0_BASE + UART_DR, byte as u32);
}

global_asm!(
    r#"
    .syntax unified
    .thumb
    .section .text.rp1_pcie_reloc,"ax",%progbits
    .balign 4
    .global __rp1_pcie_reloc_start
    .global __rp1_pcie_reloc_end
    .thumb_func
__rp1_pcie_reloc_start:
    /* marker: relocated entry reached */
    movw r3, #0xf008
    movt r3, #0x2000
    movw r1, #0x0003
    movt r1, #0xe5d1
    str r1, [r3]
    movw r3, #0x0000
    movt r3, #0x4003
    movs r1, #'E'
    str r1, [r3]

    /*
     * E5d1-old-d local DSRAM state model.
     * Clean-room layout mirrors the old firmware's state/timestamp/event area
     * without copying firmware data bytes.
     */
    movw r0, #0x2000
    movt r0, #0x1000
    movs r1, #0
    movs r2, #0x60
0:
    str r1, [r0]
    adds r0, r0, #4
    subs r2, r2, #4
    bne 0b

    movw r0, #0x2000
    movt r0, #0x1000
    movs r1, #1
    str r1, [r0, #0x10]    /* event/state-machine active flag */
    movs r1, #11
    strb r1, [r0, #0x16]   /* old initial state byte */
    movs r1, #1
    strb r1, [r0, #0x18]   /* next state / local latch */

    /* marker: local DSRAM state initialized */
    movw r3, #0xf010
    movt r3, #0x2000
    movw r1, #0x0005
    movt r1, #0xe5d1
    str r1, [r3]
    movw r3, #0x0000
    movt r3, #0x4003
    movs r1, #'S'
    str r1, [r3]

    /*
     * Old reset/platform prelude, clean-room translation of the semantic MMIO
     * sequence around 0x20000388..0x20000406.
     */
    movw r0, #0x4000
    movt r0, #0x4001
    ldr r1, [r0, #0x04]
    orr r1, r1, #0x40000
    str r1, [r0, #0x04]
    ldr r1, [r0]
    orr r1, r1, #0x1000
    str r1, [r0]
    movw r1, #0x0000
    movt r1, #0x0400
    str r1, [r0]
    movs r1, #0
    str r1, [r0, #0x04]
    str r1, [r0, #0x08]

    /*
     * Clean-room selected translation of old 0x400fc000 + 0x20003d20
     * table writes.  These are explicit semantic writes, not copied table data.
     */
    movw r0, #0xc000
    movt r0, #0x400f
    movs r1, #0x40
    str r1, [r0, #0x04]
    str r1, [r0, #0x28]
    str r1, [r0, #0x2c]
    str r1, [r0, #0x30]
    ldr r1, [r0, #0x04]
    bic r1, r1, #0x80
    str r1, [r0, #0x04]
    movw r3, #0x0000
    movt r3, #0x4003
    movs r1, #'P'
    str r1, [r3]

    /*
     * Old fallback/reset setup if 0x401081a4 says the link-side state is not
     * ready.  This keeps E5b's stock-polarity bit26 poll but in old ordering.
     */
    movw r0, #0x81a4
    movt r0, #0x4010
    ldr r1, [r0]
    lsls r1, r1, #11
    bmi 1f

    movw r0, #0x4000
    movt r0, #0x4001
    ldr r1, [r0]
    bic r1, r1, #0x04000000
    str r1, [r0]
    movw r2, #0x86a0
    movt r2, #0x0001
2:
    ldr r1, [r0, #0x18]
    lsls r1, r1, #5
    bmi 3f
    subs r2, r2, #1
    bne 2b
3:
    movw r0, #0x81c8
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #2
    str r1, [r0]
    movw r0, #0x81c0
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #2
    str r1, [r0]
    movw r0, #0x8004
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #4
    str r1, [r0]
    ldr r1, [r0]
    orr r1, r1, #1
    str r1, [r0]
    movw r0, #0x8194
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #8
    str r1, [r0]
    movw r0, #0x8004
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #0x80
    str r1, [r0]
1:
    /* marker: reset/platform prelude done */
    movw r3, #0xf00c
    movt r3, #0x2000
    movw r1, #0x0004
    movt r1, #0xe5d1
    str r1, [r3]

    /*
     * Old state 1: initial PCIe control setup, next state 2.
     */
    movw r0, #0x81ac
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #0x7f
    str r1, [r0]
    movw r0, #0x81c8
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #2
    str r1, [r0]
    movw r0, #0x8194
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #2
    str r1, [r0]
    ldr r1, [r0]
    bic r1, r1, #8
    str r1, [r0]
    movw r0, #0x8004
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #4
    str r1, [r0]
    ldr r1, [r0]
    orr r1, r1, #1
    str r1, [r0]
    movw r0, #0x81ac
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #2
    str r1, [r0]

    movw r0, #0x2000
    movt r0, #0x1000
    movs r1, #2
    strb r1, [r0, #0x16]

    /*
     * Old states 3/4: bounded wait, sideband/control transition, and state 5
     * scheduling.  Exact old timestamp helpers are represented by short waits.
     */
    movw r2, #0x2000
4:
    subs r2, r2, #1
    bne 4b
    movw r0, #0x81c8
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #2
    str r1, [r0]
    movw r0, #0x8004
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #0x80
    str r1, [r0]
    movw r0, #0x81ac
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #1
    str r1, [r0]
    movw r0, #0x2000
    movt r0, #0x1000
    movs r1, #5
    strb r1, [r0, #0x16]

    /*
     * Old state 5: DBI/ATU/window setup.
     */
    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]

    movw r0, #0x970c
    movt r0, #0x4010
    ldr r1, [r0]
    movw r2, #0x00ff
    movt r2, #0xc700
    and r1, r1, r2
    movw r2, #0x3000
    movt r2, #0x2830
    orr r1, r1, r2
    str r1, [r0]

    movw r0, #0x980c
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #0xff
    orr r1, r1, #0x30
    str r1, [r0]

    movw r0, #0x98bc
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #1
    str r1, [r0]

    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]
    movw r0, #0x981c
    movt r0, #0x4010
    movs r1, #6
    str r1, [r0]

    movw r0, #0x90b0
    movt r0, #0x4010
    ldr r1, [r0]
    uxth r1, r1
    movw r2, #0x0000
    movt r2, #0x003c
    orr r1, r1, r2
    str r1, [r0]
    movw r0, #0x90b4
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]
    movw r0, #0x90b8
    movt r0, #0x4010
    movw r1, #0x2000
    str r1, [r0]
    movw r0, #0x9008
    movt r0, #0x4010
    movw r1, #0x0000
    movt r1, #0x0200
    str r1, [r0]

    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #1
    str r1, [r0]
    movw r0, #0x9010
    movt r0, #0x4010
    movw r1, #0x3fff
    str r1, [r0]
    movw r0, #0x9014
    movt r0, #0x4010
    movw r1, #0xffff
    movt r1, #0x003f
    str r1, [r0]
    movw r0, #0x9018
    movt r0, #0x4010
    movw r1, #0xffff
    str r1, [r0]

    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]
    movw r1, #0xfff0
    movt r1, #0xffff
    movw r0, #0x9010
    movt r0, #0x4010
    str r1, [r0]
    movw r0, #0x9014
    movt r0, #0x4010
    str r1, [r0]
    movw r0, #0x9018
    movt r0, #0x4010
    str r1, [r0]

    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #0x23
    str r1, [r0]
    movw r0, #0x9114
    movt r0, #0x4010
    movw r1, #0x0000
    movt r1, #0x4000
    str r1, [r0]
    movw r0, #0x9118
    movt r0, #0x4010
    movs r1, #0xc0
    str r1, [r0]
    movw r0, #0x9100
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]
    movw r0, #0x9104
    movt r0, #0x4010
    movw r1, #0x0100
    movt r1, #0xc000
    str r1, [r0]

    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #0x63
    str r1, [r0]
    movw r0, #0x9114
    movt r0, #0x4010
    movw r1, #0x0000
    movt r1, #0x2000
    str r1, [r0]
    movw r0, #0x9118
    movt r0, #0x4010
    movs r1, #0xc0
    str r1, [r0]
    movw r0, #0x9100
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]
    movw r0, #0x9104
    movt r0, #0x4010
    movw r1, #0x0200
    movt r1, #0xc000
    str r1, [r0]

    movw r0, #0x8000
    movt r0, #0x4010
    movs r1, #0
    str r1, [r0]
    movw r0, #0x98d4
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #0x100
    str r1, [r0]
    movw r0, #0x8188
    movt r0, #0x4010
    movw r1, #0x0000
    movt r1, #0x0100
    str r1, [r0]
    movw r0, #0x98bc
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #1
    str r1, [r0]

    movw r0, #0x8004
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #4
    str r1, [r0]
    ldr r1, [r0]
    bic r1, r1, #1
    str r1, [r0]
    movw r0, #0x81ac
    movt r0, #0x4010
    ldr r1, [r0]
    orr r1, r1, #0x48
    str r1, [r0]

    /* marker: ordered state path through state 5 done */
    movw r3, #0xf014
    movt r3, #0x2000
    movw r1, #0x0006
    movt r1, #0xe5d1
    str r1, [r3]
    movw r3, #0x0000
    movt r3, #0x4003
    movs r1, #'O'
    str r1, [r3]

    /*
     * Old state 11 CFG scan/retrain over 0x40108008..0x40108108.
     */
    movw r0, #0x9004
    movt r0, #0x4010
    ldr r12, [r0]
    and r12, r12, #0x400
    ldr r1, [r0]
    bic r1, r1, #0x400
    str r1, [r0]

    movw r2, #0x8008
    movt r2, #0x4010
    movw r3, #0x8108
    movt r3, #0x4010
5:
    cmp r2, r3
    beq 9f
    ldr r0, [r2]
    lsls r1, r0, #31
    bpl 8f
    orr r1, r0, #8
    str r1, [r2]
    ldr r1, [r2]
    orr r1, r1, #2
    str r1, [r2]
    movw r1, #0x0400
6:
    subs r1, r1, #1
    bne 6b
    ldr r1, [r2]
    bic r1, r1, #2
    str r1, [r2]
    ldr r1, [r2]
    orr r1, r1, #4
    str r1, [r2]
    tst r0, #0x10
    bne 8f
    ldr r1, [r2]
    bic r1, r1, #8
    str r1, [r2]
8:
    adds r2, r2, #4
    b 5b
9:
    movw r0, #0x9004
    movt r0, #0x4010
    ldr r1, [r0]
    bic r1, r1, #0x400
    orr r1, r1, r12
    str r1, [r0]

    /* marker: state 11 CFG scan/retrain done */
    movw r3, #0xf018
    movt r3, #0x2000
    movw r1, #0x0007
    movt r1, #0xe5d1
    str r1, [r3]
    movw r3, #0x0000
    movt r3, #0x4003
    movs r1, #'R'
    str r1, [r3]

    /* marker: before return */
    movw r3, #0xf01c
    movt r3, #0x2000
    movw r1, #0x0008
    movt r1, #0xe5d1
    str r1, [r3]
    movw r3, #0x0000
    movt r3, #0x4003
    movs r1, #'X'
    str r1, [r3]
    bx lr
__rp1_pcie_reloc_end:
    "#,
);
