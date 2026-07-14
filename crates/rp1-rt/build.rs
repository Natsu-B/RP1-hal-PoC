use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PCIE_EP_INIT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_STACK_LOW");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_SNAPSHOT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_INIT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_STUB");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let stack_low = env::var_os("CARGO_FEATURE_DEBUG_STACK_LOW").is_some();
    let snapshot = env::var_os("CARGO_FEATURE_DEBUG_SNAPSHOT").is_some();
    let mailbox_layout = env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT").is_some();

    let app_len = if stack_low { "62K" } else { "64K" };
    let stack_start = if stack_low {
        "0x2000f800"
    } else {
        "ORIGIN(RP1_APP_SRAM) + LENGTH(RP1_APP_SRAM)"
    };
    let diag_region = if snapshot {
        "  RP1_DEBUG_DIAG (rwx)  : ORIGIN = 0x2000f800, LENGTH = 1K\n"
    } else {
        "  RP1_DEBUG_DIAG (rwx)  : ORIGIN = 0x2000f800, LENGTH = 0\n"
    };
    let stub_region = if mailbox_layout {
        "  RP1_DEBUG_STUB (rwx)  : ORIGIN = 0x2000fc00, LENGTH = 1K\n"
    } else if stack_low {
        "  RP1_DEBUG_STUB (rwx)  : ORIGIN = 0x2000fc00, LENGTH = 0\n"
    } else {
        "  RP1_DEBUG_STUB (rwx)  : ORIGIN = 0x20010000, LENGTH = 0\n"
    };

    let memory_x = format!(
        r#"MEMORY
{{
  RP1_APP_SRAM (rwx)    : ORIGIN = 0x20000000, LENGTH = {app_len}
{diag_region}{stub_region}}}

_stack_start = {stack_start};
__rp1_debug_diag_start = ORIGIN(RP1_DEBUG_DIAG);
__rp1_debug_diag_end = ORIGIN(RP1_DEBUG_DIAG) + LENGTH(RP1_DEBUG_DIAG);
__rp1_debug_stub_start = ORIGIN(RP1_DEBUG_STUB);
__rp1_debug_stub_end = ORIGIN(RP1_DEBUG_STUB) + LENGTH(RP1_DEBUG_STUB);
__rp1_debug_mailbox = ORIGIN(RP1_DEBUG_STUB);
"#,
    );

    fs::write(out_dir.join("rp1-memory.x"), memory_x).unwrap();

    println!("cargo:rustc-link-search={}", manifest_dir);
    println!("cargo:rustc-link-search={}", out_dir.display());
}
