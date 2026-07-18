use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

#[cfg(feature = "debug-stub")]
use core::cmp;

use rp1_abi::debug::{self, DebugMailbox, SnapshotEntry};

static LAST_SEQ: AtomicU32 = AtomicU32::new(0);

#[cfg(feature = "debug-stub")]
const PCIE_CFG_BASE: usize = 0x4010_8000;
#[cfg(feature = "debug-stub")]
const PCIE_DBI_BASE: usize = 0x4010_9000;
#[cfg(feature = "debug-stub")]
const DEBUG_DIAG_ADDR: usize = 0x2000_f800;

#[cfg(feature = "debug-stub")]
const DIAG_MAGIC: u32 = u32::from_le_bytes(*b"P1DG");

const _: () = assert!(core::mem::size_of::<DebugMailbox>() <= debug::MAILBOX_SIZE);

#[cfg(feature = "debug-stub")]
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
    write_u32(&mut mailbox.flags, 0);
    write_u32(&mut mailbox.seq, 0);
    write_u32(&mut mailbox.ack, 0);
    write_u32(&mut mailbox.state, debug::state::RUNNING);
    write_u32(&mut mailbox.stop_reason, debug::stop_reason::NONE);
    write_u32(&mut mailbox.command, debug::command::NONE);
    write_u32(&mut mailbox.arg0, 0);
    write_u32(&mut mailbox.arg1, 0);
    write_u32(&mut mailbox.status, debug::status::OK);
    write_u32(&mut mailbox.data_len, 0);
    LAST_SEQ.store(read_u32(&mailbox.seq), Ordering::Relaxed);
}

#[cfg(feature = "debug-stub")]
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

#[cfg(feature = "debug-stub")]
fn write_diag(index: usize, value: u32) {
    unsafe { ptr::write_volatile((DEBUG_DIAG_ADDR as *mut u32).add(index), value) };
}

#[cfg(feature = "debug-stub")]
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
        debug::command::READ_SNAPSHOT_ALLOWLISTED => read_snapshot_allowlisted(mailbox, seq),
        #[cfg(feature = "debug-stub")]
        debug::command::GET_REGS => {
            snapshot_core_regs(mailbox);
            debug::status::OK
        }
        #[cfg(feature = "debug-stub")]
        debug::command::READ_MEM => read_mem(mailbox),
        #[cfg(feature = "debug-stub")]
        debug::command::WRITE_MEM => write_mem(mailbox),
        #[cfg(feature = "debug-stub")]
        debug::command::HALT => {
            write_u32(&mut mailbox.state, debug::state::STOPPED);
            write_u32(&mut mailbox.stop_reason, debug::stop_reason::HOST_HALT);
            debug::status::OK
        }
        #[cfg(feature = "debug-stub")]
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

fn read_snapshot_allowlisted(mailbox: &mut DebugMailbox, sequence: u32) -> u32 {
    let snapshot_id = read_u32(&mailbox.arg0);
    if read_u32(&mailbox.arg1) != debug::snapshot_request_checksum(sequence, snapshot_id) {
        write_u32(&mut mailbox.data_len, 0);
        return debug::status::BAD_CHECKSUM;
    }

    let entries = match snapshot_id {
        debug::snapshot::CORE_STATUS => core_status_entries(mailbox),
        debug::snapshot::PERIPHERAL_STATUS => peripheral_status_entries(),
        _ => {
            write_snapshot_response(
                mailbox,
                snapshot_id,
                sequence,
                debug::status::BAD_SNAPSHOT_ID,
                &[],
            );
            return debug::status::BAD_SNAPSHOT_ID;
        }
    };
    write_snapshot_response(mailbox, snapshot_id, sequence, debug::status::OK, &entries);
    debug::status::OK
}

fn core_status_entries(mailbox: &DebugMailbox) -> [SnapshotEntry; debug::SNAPSHOT_MAX_ENTRIES] {
    let values = [
        read_u32(&mailbox.magic),
        read_u32(&mailbox.version),
        read_u32(&mailbox.size),
        read_u32(&mailbox.ack),
        read_u32(&mailbox.state),
        read_u32(&mailbox.stop_reason),
        read_u32(&mailbox.command),
        read_u32(&mailbox.status),
    ];
    snapshot_entries(&debug::snapshot::CORE_STATUS_ADDRESSES, values)
}

fn peripheral_status_entries() -> [SnapshotEntry; debug::SNAPSHOT_MAX_ENTRIES] {
    let mut values = [0u32; debug::SNAPSHOT_MAX_ENTRIES];
    for (value, &address) in values
        .iter_mut()
        .zip(debug::snapshot::PERIPHERAL_STATUS_ADDRESSES.iter())
    {
        // SAFETY: the ABI allowlist contains only fixed, side-effect-free status registers.
        *value = unsafe { ptr::read_volatile(address as *const u32) };
    }
    snapshot_entries(&debug::snapshot::PERIPHERAL_STATUS_ADDRESSES, values)
}

fn snapshot_entries(
    addresses: &[u32; debug::SNAPSHOT_MAX_ENTRIES],
    values: [u32; debug::SNAPSHOT_MAX_ENTRIES],
) -> [SnapshotEntry; debug::SNAPSHOT_MAX_ENTRIES] {
    let mut entries = [SnapshotEntry {
        local_address: 0,
        value: 0,
        status: debug::snapshot::ENTRY_OK,
    }; debug::SNAPSHOT_MAX_ENTRIES];
    for ((entry, &address), value) in entries.iter_mut().zip(addresses.iter()).zip(values) {
        entry.local_address = address;
        entry.value = value;
    }
    entries
}

fn write_snapshot_response(
    mailbox: &mut DebugMailbox,
    snapshot_id: u32,
    sequence: u32,
    response_status: u32,
    entries: &[SnapshotEntry],
) {
    let mut encoded = [0u8; debug::SNAPSHOT_RESPONSE_MAX_LEN];
    let len = debug::encode_snapshot_response(
        &mut encoded,
        snapshot_id,
        sequence,
        response_status,
        entries,
    )
    .expect("bounded snapshot response");
    for (index, value) in encoded[..len].iter().copied().enumerate() {
        unsafe { ptr::write_volatile(mailbox.data.as_mut_ptr().add(index), value) };
    }
    write_u32(&mut mailbox.data_len, len as u32);
}

#[cfg(feature = "debug-stub")]
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

#[cfg(feature = "debug-stub")]
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

#[cfg(feature = "debug-stub")]
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

#[cfg(feature = "debug-stub")]
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

#[cfg(feature = "debug-stub")]
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
