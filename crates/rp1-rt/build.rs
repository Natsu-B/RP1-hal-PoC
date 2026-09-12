use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=proc1.x");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_FREERTOS_PROC1_WORKER");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_FREERTOS");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_FREERTOS_WARM_DATA");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_PCIE_EP_INIT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_STACK_LOW");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_SNAPSHOT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_INIT");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_STUB");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let stack_low = env::var_os("CARGO_FEATURE_DEBUG_STACK_LOW").is_some();
    let snapshot = env::var_os("CARGO_FEATURE_DEBUG_SNAPSHOT").is_some();
    let mailbox_layout = env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT").is_some();
    let mailbox_layout_v1 = env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1").is_some();
    let rtos = env::var_os("CARGO_FEATURE_FREERTOS").is_some();

    let app_len = if rtos { "56K" } else if stack_low { "62K" } else { "64K" };
    let stack_start = if rtos {
        "0x2000f000"
    } else if stack_low {
        "0x2000f800"
    } else {
        "ORIGIN(RP1_APP_SRAM) + LENGTH(RP1_APP_SRAM)"
    };
    let diag_region = if snapshot {
        "  RP1_DEBUG_DIAG (rwx)  : ORIGIN = 0x2000f800, LENGTH = 1K\n"
    } else {
        "  RP1_DEBUG_DIAG (rwx)  : ORIGIN = 0x2000f800, LENGTH = 0\n"
    };
    let stub_region = if mailbox_layout_v1 {
        "  RP1_DEBUG_STUB (rwx)  : ORIGIN = 0x2000fc00, LENGTH = 0x300\n"
    } else if mailbox_layout {
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
__app_limit = ORIGIN(RP1_APP_SRAM) + LENGTH(RP1_APP_SRAM);
__rp1_debug_diag_start = ORIGIN(RP1_DEBUG_DIAG);
__rp1_debug_diag_end = ORIGIN(RP1_DEBUG_DIAG) + LENGTH(RP1_DEBUG_DIAG);
__rp1_debug_stub_start = ORIGIN(RP1_DEBUG_STUB);
__rp1_debug_stub_end = ORIGIN(RP1_DEBUG_STUB) + LENGTH(RP1_DEBUG_STUB);
__rp1_debug_mailbox = ORIGIN(RP1_DEBUG_STUB);
"#,
    );

    fs::write(out_dir.join("rp1-memory.x"), memory_x).unwrap();
    // Always resolvable; non-proc1 images retain their original empty layout.
    let proc1 = if env::var_os("CARGO_FEATURE_FREERTOS_PROC1_WORKER").is_some() {
        fs::read_to_string(PathBuf::from(&manifest_dir).join("proc1.x")).unwrap()
    } else { String::new() };
    fs::write(out_dir.join("rp1-proc1.x"), proc1).unwrap();

    // No new section or alignment when the experiment is disabled.
    let warm_data = if env::var_os("CARGO_FEATURE_FREERTOS_WARM_DATA").is_some() {
        r#".warm_data_shadow (NOLOAD) : ALIGN(4)
{
  __warm_data_shadow_start = .;
  . += 12 + SIZEOF(.data);
  __warm_data_shadow_end = .;
} > RP1_APP_SRAM
ASSERT((__warm_data_shadow_start & 3) == 0, "warm data header alignment")
ASSERT(__data_end >= __data_start, "invalid data range")
ASSERT(__warm_data_shadow_end - __warm_data_shadow_start == 12 + __data_end - __data_start, "warm data shadow size")
ASSERT(__data_end <= __sbss && __ebss <= __warm_data_shadow_start, "warm data overlaps BSS clear")
ASSERT(__warm_data_shadow_end <= __app_limit, "warm data overlaps reserved SRAM")
"#
    } else { "" };
    fs::write(out_dir.join("rp1-warm-data.x"), warm_data).unwrap();

    println!("cargo:rustc-link-search={}", manifest_dir);
    println!("cargo:rustc-link-search={}", out_dir.display());
}
