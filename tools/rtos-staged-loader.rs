//! Host-only fixtures against the selected, unchanged loader source.
//! This does not execute ARM code or admit a runtime/linker memory layout.
#![allow(dead_code)]

// Only platform error/log plumbing is replaced; rp1_image.rs is included intact.
#[derive(Debug)]
pub enum BootError { AddressOverflow, Rp1ImageCrcMismatch, Rp1ImageInvalid, Rp1ImageTooLarge }
#[macro_export]
macro_rules! logln { ($($arg:tt)*) => { println!($($arg)*) }; }
mod selected { include!(env!("RP1_BOOT_IMAGE_RS")); }

const BASE: u32 = 0x2000_0000;
const STAGE: u32 = BASE + 0x8000;
const LOCAL: u32 = 0x1000_1000;
const ENTRY: u32 = BASE + 0x141;
const PAYLOAD: [u8; 16] = [0x5a; 16]; // A data pattern, never executable proof.
type Phdr = [u32; 8];
const LINKED: &[u8] = include_bytes!(env!("RP1_LINKER_FIXTURE"));

fn headers() -> [Phdr; 3] {
    [
        [1, 0x100, BASE, BASE, 8, 0x200, 5, 4],
        [1, 0x140, LOCAL, STAGE, 16, 16, 5, 4],
        // PT_NULL: future BSS is not uploaded as an overlapping PT_LOAD.
        // Merely ignoring this header does NOT initialize runtime BSS.
        [0, 0, STAGE, STAGE, 0, 0x1000, 6, 4],
    ]
}

fn fixture(h: [Phdr; 3], entry: u32) -> Vec<u8> {
    let mut b = vec![0; 0x180];
    b[..7].copy_from_slice(b"\x7fELF\x01\x01\x01");
    for (at, value) in [(16, 2u16), (18, 40), (40, 52), (42, 32), (44, 3)] {
        b[at..at+2].copy_from_slice(&value.to_le_bytes());
    }
    for (at, value) in [(20, 1u32), (24, entry), (28, 52)] {
        b[at..at+4].copy_from_slice(&value.to_le_bytes());
    }
    for (i, words) in h.iter().enumerate() {
        for (j, value) in words.iter().enumerate() {
            let at = 52 + 32*i + 4*j;
            b[at..at+4].copy_from_slice(&value.to_le_bytes());
        }
    }
    b[0x100..0x104].copy_from_slice(&(BASE+0xf000).to_le_bytes());
    b[0x104..0x108].copy_from_slice(&ENTRY.to_le_bytes());
    b[0x140..0x150].copy_from_slice(&PAYLOAD);
    b
}

fn rejects(h: [Phdr; 3], entry: u32) {
    let mut scratch = vec![0xa5; 0x10000];
    assert!(selected::build_from_rp1_elf(&fixture(h, entry), &mut scratch, 0).is_err());
}

#[test]
fn physical_staging_accepted_but_no_local_memory_write_is_performed() {
    let bytes = fixture(headers(), ENTRY);
    let info = selected::inspect_rp1_elf(&bytes).unwrap();
    assert_eq!(info.load_count, 2);
    assert_eq!(info.loads()[1].vaddr, LOCAL);
    assert_eq!(info.loads()[1].paddr, STAGE);
    let mut scratch = vec![0xa5; 0x10000];
    let image = selected::build_from_rp1_elf(&bytes, &mut scratch, 0).unwrap();
    assert_eq!(image.load_addr, BASE);
    assert_eq!(image.entry, ENTRY);
    assert_eq!(image.stack, BASE+0xf000);
    assert_eq!(image.payload.len(), 0x8010);
    assert_eq!(&image.payload[0x8000..0x8010], &PAYLOAD);
    assert!(image.payload[8..0x8000].iter().all(|&b| b == 0));
    // The uploader receives shared bytes only. A separate runtime copy is OPEN.
}

#[test]
fn bss_load_overlap_rejected_in_either_header_order() {
    let mut h = headers(); h[2][0] = 1;
    rejects(h, ENTRY);
    h.swap(1, 2);
    rejects(h, ENTRY);
}

#[test]
fn direct_local_physical_load_and_out_of_window_loads_rejected() {
    for pa in [LOCAL, BASE-16, BASE+0x10000, BASE+0xffff, u32::MAX-7] {
        let mut h = headers(); h[1][3] = pa;
        rejects(h, ENTRY);
    }
}

#[test]
fn local_entry_is_not_a_valid_bootstrap_entry() {
    rejects(headers(), LOCAL|1);
}

#[test]
fn malformed_size_file_range_and_alignment_rejected() {
    for (field, value) in [(4, 17), (1, 0x178), (2, LOCAL+2), (7, 3)] {
        let mut h = headers(); h[1][field] = value;
        rejects(h, ENTRY);
    }
    let mut h = headers(); h[1][5] = 0x10000;
    rejects(h, ENTRY);
}

#[test]
fn rtos_reserved_memory_is_not_protected_by_generic_loader_policy() {
    // Deliberately accepted by this generic64KiB loader, forbidden by the
    // current RTOS ELF validator. Do not replace the latter with this test.
    let mut h = headers(); h[1][3] = BASE+0xf000;
    let bytes = fixture(h, ENTRY);
    let mut scratch = vec![0xa5; 0x10000];
    let image = selected::build_from_rp1_elf(&bytes, &mut scratch, 0).unwrap();
    assert_eq!(image.payload.len(), 0xf010);
    assert_eq!(&image.payload[0xf000..0xf010], &PAYLOAD);
}

#[test]
fn actual_linker_elf_materializes_shared_only_and_ignores_local_stack() {
    let info = selected::inspect_rp1_elf(LINKED).unwrap();
    assert_eq!(info.load_count, 1);
    assert_eq!(info.phnum, 2);
    assert_eq!(info.vector0_sp, BASE+0xf000);
    let shared = &info.loads()[0];
    assert_eq!(shared.vaddr, shared.paddr);
    assert_eq!(shared.paddr, BASE);
    assert!(shared.memsz > shared.filesz);
    assert!(info.loads().iter().all(|load| load.paddr >= BASE
        && load.paddr.checked_add(load.memsz).unwrap() <= BASE+0xe000));
    let payload = selected::elf_load_file_bytes(LINKED, shared).unwrap();
    let mut scratch = vec![0xa5; 0x10000];
    let image = selected::build_from_rp1_elf(LINKED, &mut scratch, 0).unwrap();
    assert_eq!(image.entry, info.vector1_reset);
    assert_eq!(image.entry & 1, 1);
    assert_eq!(image.payload.len(), shared.memsz as usize);
    assert_eq!(&image.payload[..payload.len()], payload);
    assert!(image.payload[payload.len()..].iter().all(|b| *b == 0));
}

#[test]
fn actual_linker_local_stack_cannot_be_reclassified_as_a_load() {
    let mut bytes = LINKED.to_vec();
    let phoff = u32::from_le_bytes(bytes[28..32].try_into().unwrap()) as usize;
    let phsize = u16::from_le_bytes(bytes[42..44].try_into().unwrap()) as usize;
    let phnum = u16::from_le_bytes(bytes[44..46].try_into().unwrap()) as usize;
    let mut changed = 0;
    for n in 0..phnum {
        let at = phoff+n*phsize;
        if u32::from_le_bytes(bytes[at..at+4].try_into().unwrap()) == 0 {
            assert_eq!(&bytes[at+8..at+12], &0x10003800u32.to_le_bytes());
            assert_eq!(&bytes[at+12..at+16], &0x10003800u32.to_le_bytes());
            assert_eq!(&bytes[at+16..at+20], &0u32.to_le_bytes());
            assert_eq!(&bytes[at+20..at+24], &2048u32.to_le_bytes());
            bytes[at..at+4].copy_from_slice(&1u32.to_le_bytes());
            changed += 1;
        }
    }
    assert_eq!(changed, 1);
    let mut scratch = vec![0xa5; 0x10000];
    assert!(selected::build_from_rp1_elf(&bytes, &mut scratch, 0).is_err());
}

#[test]
#[ignore = "requires runner --image; not a hardware test"]
fn target_image_materializes_shared_only() {
    let bytes = std::fs::read(std::env::var("RP1_RUNTIME_ELF").unwrap()).unwrap();
    let info = selected::inspect_rp1_elf(&bytes).unwrap();
    assert_eq!(info.vector0_sp, BASE+0xf000);
    let mut scratch = vec![0xa5; 0x10000];
    let image = selected::build_from_rp1_elf(&bytes, &mut scratch, 0).unwrap();
    assert_eq!(image.entry, info.vector1_reset);
    assert_eq!(image.stack, BASE+0xf000);
    assert!(image.payload.len() <= 0xe000);
    for load in info.loads() {
        assert_eq!(load.vaddr, load.paddr);
        assert!(BASE <= load.paddr && load.paddr + load.memsz <= BASE+0xe000);
        let start = (load.paddr-BASE) as usize;
        let payload = selected::elf_load_file_bytes(&bytes, load).unwrap();
        assert_eq!(&image.payload[start..start+payload.len()], payload);
        assert!(image.payload[start+payload.len()..start+load.memsz as usize].iter().all(|b| *b == 0));
    }
    println!("host materialized {} shared bytes; entry={:#x}; no proc-local upload", image.payload.len(), image.entry);
}
