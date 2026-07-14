use core::cmp;
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

use rp1_abi::debug::{self, DebugMailbox};

static LAST_SEQ: AtomicU32 = AtomicU32::new(0);

const PCIE_CFG_BASE: usize = 0x4010_8000;
const PCIE_DBI_BASE: usize = 0x4010_9000;
const DEBUG_DIAG_ADDR: usize = 0x2000_f800;

const DIAG_MAGIC: u32 = u32::from_le_bytes(*b"P1DG");

const _: () = assert!(core::mem::size_of::<DebugMailbox>() <= debug::MAILBOX_SIZE);

pub fn init() {
    init_mailbox();
    snapshot_pcie_diag(0x40, 0, 0);
}

pub fn init_mailbox() {
    let mailbox = mailbox_mut();
    write_u32(&mut mailbox.magic, debug::MAGIC);
    write_u32(&mut mailbox.version, debug::VERSION);
    write_u32(
        &mut mailbox.size,
        core::mem::size_of::<DebugMailbox>() as u32,
    );
    write_u32(&mut mailbox.state, debug::state::RUNNING);
    write_u32(&mut mailbox.stop_reason, debug::stop_reason::NONE);
    write_u32(&mut mailbox.command, debug::command::NONE);
    write_u32(&mut mailbox.status, debug::status::OK);
    LAST_SEQ.store(read_u32(&mailbox.seq), Ordering::Relaxed);
}

fn snapshot_pcie_diag(phase: u32, register: u32, value: u32) {
    write_diag(0, DIAG_MAGIC);
    write_diag(1, phase);
    write_diag(2, register);
    write_diag(3, value);
    write_diag(4, mmio_read32(PCIE_CFG_BASE + 0x004));
    write_diag(5, mmio_read32(PCIE_CFG_BASE + 0x194));
    write_diag(6, mmio_read32(PCIE_CFG_BASE + 0x1a4));
    write_diag(7, mmio_read32(PCIE_CFG_BASE + 0x1ac));
    write_diag(8, mmio_read32(PCIE_DBI_BASE + 0x004));
}

fn write_diag(index: usize, value: u32) {
    unsafe { ptr::write_volatile((DEBUG_DIAG_ADDR as *mut u32).add(index), value) };
}

fn mmio_read32(addr: usize) -> u32 {
    unsafe { ptr::read_volatile(addr as *const u32) }
}

pub fn poll() {
    let mailbox = mailbox_mut();
    let seq = read_u32(&mailbox.seq);
    if seq == LAST_SEQ.load(Ordering::Relaxed) {
        return;
    }

    let command = read_u32(&mailbox.command);
    let status = match command {
        debug::command::NONE => debug::status::OK,
        debug::command::PING => debug::status::OK,
        debug::command::GET_REGS => {
            snapshot_core_regs(mailbox);
            debug::status::OK
        }
        debug::command::READ_MEM => read_mem(mailbox),
        debug::command::WRITE_MEM => write_mem(mailbox),
        debug::command::HALT => {
            write_u32(&mut mailbox.state, debug::state::STOPPED);
            write_u32(&mut mailbox.stop_reason, debug::stop_reason::HOST_HALT);
            debug::status::OK
        }
        debug::command::CONTINUE => {
            write_u32(&mut mailbox.state, debug::state::RUNNING);
            write_u32(&mut mailbox.stop_reason, debug::stop_reason::NONE);
            debug::status::OK
        }
        _ => debug::status::BAD_COMMAND,
    };

    write_u32(&mut mailbox.status, status);
    write_u32(&mut mailbox.command, debug::command::NONE);
    write_u32(&mut mailbox.ack, seq);
    LAST_SEQ.store(seq, Ordering::Relaxed);
}

pub fn fault() -> ! {
    let mailbox = mailbox_mut();
    snapshot_core_regs(mailbox);
    write_u32(&mut mailbox.state, debug::state::FAULTED);
    write_u32(&mut mailbox.stop_reason, debug::stop_reason::EXCEPTION);
    loop {
        poll();
        core::hint::spin_loop();
    }
}

pub fn panic() -> ! {
    let mailbox = mailbox_mut();
    snapshot_core_regs(mailbox);
    write_u32(&mut mailbox.state, debug::state::FAULTED);
    write_u32(&mut mailbox.stop_reason, debug::stop_reason::PANIC);
    loop {
        poll();
        core::hint::spin_loop();
    }
}

fn read_mem(mailbox: &mut DebugMailbox) -> u32 {
    let addr = read_u32(&mailbox.arg0) as usize;
    let len = cmp::min(read_u32(&mailbox.arg1) as usize, debug::MAILBOX_DATA_LEN);
    for idx in 0..len {
        let value = unsafe { ptr::read_volatile((addr + idx) as *const u8) };
        unsafe { ptr::write_volatile(mailbox.data.as_mut_ptr().add(idx), value) };
    }
    write_u32(&mut mailbox.data_len, len as u32);
    debug::status::OK
}

fn write_mem(mailbox: &mut DebugMailbox) -> u32 {
    let addr = read_u32(&mailbox.arg0) as usize;
    let len = read_u32(&mailbox.arg1) as usize;
    if len > debug::MAILBOX_DATA_LEN {
        return debug::status::BAD_LENGTH;
    }
    for idx in 0..len {
        let value = unsafe { ptr::read_volatile(mailbox.data.as_ptr().add(idx)) };
        unsafe { ptr::write_volatile((addr + idx) as *mut u8, value) };
    }
    debug::status::OK
}

fn snapshot_core_regs(mailbox: &mut DebugMailbox) {
    let mut regs = [0u32; debug::MAILBOX_REG_COUNT];
    unsafe {
        core::arch::asm!(
            "mov {sp_out}, sp",
            "mov {lr_out}, lr",
            "mrs {xpsr_out}, xpsr",
            sp_out = lateout(reg) regs[13],
            lr_out = lateout(reg) regs[14],
            xpsr_out = lateout(reg) regs[16],
            options(nostack, preserves_flags),
        );
    }
    regs[15] = snapshot_core_regs as *const () as usize as u32;
    for (dst, src) in mailbox.regs.iter_mut().zip(regs) {
        write_u32(dst, src);
    }
}

fn mailbox_mut() -> &'static mut DebugMailbox {
    unsafe { &mut *(debug::MAILBOX_ADDR as *mut DebugMailbox) }
}

fn read_u32(src: &u32) -> u32 {
    unsafe { ptr::read_volatile(src) }
}

fn write_u32(dst: &mut u32, value: u32) {
    unsafe { ptr::write_volatile(dst, value) }
}
