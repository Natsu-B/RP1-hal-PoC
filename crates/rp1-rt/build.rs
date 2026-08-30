use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PCIE_EP_INIT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_STACK_LOW");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_SNAPSHOT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V2");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_INIT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_STUB");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let stack_low = env::var_os("CARGO_FEATURE_DEBUG_STACK_LOW").is_some();
    let snapshot = env::var_os("CARGO_FEATURE_DEBUG_SNAPSHOT").is_some();
    let mailbox_layout = env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT").is_some();
    let mailbox_layout_v1 = env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1").is_some();
    let mailbox_layout_v2 = env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V2").is_some();

    if mailbox_layout_v1 && mailbox_layout_v2 {
        panic!("debug-mailbox-layout-v1 and debug-mailbox-layout-v2 are mutually exclusive");
    }

    let app_len = if mailbox_layout_v2 {
        "0xf700"
    } else if stack_low {
        "62K"
    } else {
        "64K"
    };
    let stack_start = if mailbox_layout_v2 {
        "0x2000f700"
    } else if stack_low {
        "0x2000f800"
    } else {
        "ORIGIN(RP1_APP_SRAM) + LENGTH(RP1_APP_SRAM)"
    };
    let dma_region = if mailbox_layout_v2 {
        "  RP1_SHARED_DMA (rwx)   : ORIGIN = 0x2000f700, LENGTH = 0x100\n"
    } else {
        "  RP1_SHARED_DMA (rwx)   : ORIGIN = 0x20010000, LENGTH = 0\n"
    };
    let diag_region = if mailbox_layout_v2 {
        "  RP1_DEBUG_DIAG (rwx)   : ORIGIN = 0x2000f800, LENGTH = 0x100\n"
    } else if snapshot {
        "  RP1_DEBUG_DIAG (rwx)   : ORIGIN = 0x2000f800, LENGTH = 1K\n"
    } else {
        "  RP1_DEBUG_DIAG (rwx)   : ORIGIN = 0x2000f800, LENGTH = 0\n"
    };
    let rpc_region = if mailbox_layout_v2 {
        "  RP1_RPC (rwx)          : ORIGIN = 0x2000f900, LENGTH = 0x200\n"
    } else {
        "  RP1_RPC (rwx)          : ORIGIN = 0x20010000, LENGTH = 0\n"
    };
    let scmi_region = if mailbox_layout_v2 {
        "  RP1_SCMI (rwx)         : ORIGIN = 0x2000fb00, LENGTH = 0x100\n"
    } else {
        "  RP1_SCMI (rwx)         : ORIGIN = 0x20010000, LENGTH = 0\n"
    };
    let stub_region = if mailbox_layout_v2 || mailbox_layout_v1 {
        "  RP1_DEBUG_STUB (rwx)   : ORIGIN = 0x2000fc00, LENGTH = 0x300\n"
    } else if mailbox_layout {
        "  RP1_DEBUG_STUB (rwx)   : ORIGIN = 0x2000fc00, LENGTH = 1K\n"
    } else if stack_low {
        "  RP1_DEBUG_STUB (rwx)   : ORIGIN = 0x2000fc00, LENGTH = 0\n"
    } else {
        "  RP1_DEBUG_STUB (rwx)   : ORIGIN = 0x20010000, LENGTH = 0\n"
    };
    let official_region = if mailbox_layout_v2 {
        "  RP1_OFFICIAL_MBOX (rwx): ORIGIN = 0x2000ff00, LENGTH = 0x100\n"
    } else {
        "  RP1_OFFICIAL_MBOX (rwx): ORIGIN = 0x20010000, LENGTH = 0\n"
    };
    let v2_asserts = if mailbox_layout_v2 {
        r#"ASSERT(LENGTH(RP1_APP_SRAM) == 0xf700, "RP1 shared SRAM v2 app length");
ASSERT(_stack_start == 0x2000f700, "RP1 shared SRAM v2 stack");
ASSERT(ORIGIN(RP1_SHARED_DMA) == 0x2000f700 && LENGTH(RP1_SHARED_DMA) == 0x100, "RP1 shared SRAM v2 DMA");
ASSERT(ORIGIN(RP1_DEBUG_DIAG) == 0x2000f800 && LENGTH(RP1_DEBUG_DIAG) == 0x100, "RP1 shared SRAM v2 diag");
ASSERT(ORIGIN(RP1_RPC) == 0x2000f900 && LENGTH(RP1_RPC) == 0x200, "RP1 shared SRAM v2 RPC");
ASSERT(ORIGIN(RP1_SCMI) == 0x2000fb00 && LENGTH(RP1_SCMI) == 0x100, "RP1 shared SRAM v2 SCMI");
ASSERT(ORIGIN(RP1_DEBUG_STUB) == 0x2000fc00 && LENGTH(RP1_DEBUG_STUB) == 0x300, "RP1 shared SRAM v2 D1RP");
ASSERT(ORIGIN(RP1_OFFICIAL_MBOX) == 0x2000ff00 && LENGTH(RP1_OFFICIAL_MBOX) == 0x100, "RP1 shared SRAM v2 official mailbox");
"#
    } else {
        ""
    };

    let memory_x = format!(
        r#"MEMORY
{{
  RP1_APP_SRAM (rwx)    : ORIGIN = 0x20000000, LENGTH = {app_len}
{dma_region}{diag_region}{rpc_region}{scmi_region}{stub_region}{official_region}}}

_stack_start = {stack_start};
__rp1_shared_dma_start = ORIGIN(RP1_SHARED_DMA);
__rp1_shared_dma_end = ORIGIN(RP1_SHARED_DMA) + LENGTH(RP1_SHARED_DMA);
__rp1_debug_diag_start = ORIGIN(RP1_DEBUG_DIAG);
__rp1_debug_diag_end = ORIGIN(RP1_DEBUG_DIAG) + LENGTH(RP1_DEBUG_DIAG);
__rp1_rpc_start = ORIGIN(RP1_RPC);
__rp1_rpc_end = ORIGIN(RP1_RPC) + LENGTH(RP1_RPC);
__rp1_scmi_start = ORIGIN(RP1_SCMI);
__rp1_scmi_end = ORIGIN(RP1_SCMI) + LENGTH(RP1_SCMI);
__rp1_debug_stub_start = ORIGIN(RP1_DEBUG_STUB);
__rp1_debug_stub_end = ORIGIN(RP1_DEBUG_STUB) + LENGTH(RP1_DEBUG_STUB);
__rp1_debug_mailbox = ORIGIN(RP1_DEBUG_STUB);
__rp1_official_mailbox_start = ORIGIN(RP1_OFFICIAL_MBOX);
__rp1_official_mailbox_end = ORIGIN(RP1_OFFICIAL_MBOX) + LENGTH(RP1_OFFICIAL_MBOX);
{v2_asserts}
"#,
    );

    fs::write(out_dir.join("rp1-memory.x"), memory_x).unwrap();

    println!("cargo:rustc-link-search={}", manifest_dir);
    println!("cargo:rustc-link-search={}", out_dir.display());
}
