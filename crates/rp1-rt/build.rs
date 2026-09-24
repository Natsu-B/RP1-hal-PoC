use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=link.x");
    println!("cargo:rerun-if-changed=proc1.x");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_FREERTOS_PROC1_WORKER");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_FREERTOS");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_SCMI_SHMEM");
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
    let local_monitor = env::var_os("CARGO_FEATURE_FREERTOS_LOCAL_MONITOR_STACK").is_some();
    assert!(!local_monitor || (rtos
        && env::var_os("CARGO_FEATURE_PCIE_EP_INIT").is_none()
        && env::var_os("CARGO_FEATURE_FREERTOS_PROC1_WORKER").is_none()),
        "local monitor layout is proc0-only and excludes the legacy local PCIe initializer");

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

    let local_region = if local_monitor {
        "  RP1_LOCAL_MONITOR (rw) : ORIGIN = 0x10003800, LENGTH = 2K\n"
    } else { "" };
    let memory_x = format!(
        r#"MEMORY
{{
  RP1_APP_SRAM (rwx)    : ORIGIN = 0x20000000, LENGTH = {app_len}
{diag_region}{stub_region}{local_region}}}

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

    // Never reuse fb00: that is the RTOS fault-record ABI. This reservation
    // participates in __image_end/__app_limit and is initialized by SCMI startup.
    let scmi = if env::var_os("CARGO_FEATURE_SCMI_SHMEM").is_some() {
        r#".scmi_shmem (NOLOAD) : ALIGN(64)
{
  __scmi_shmem_start = .;
  KEEP(*(.scmi_shmem));
  __scmi_shmem_end = .;
} > RP1_APP_SRAM
ASSERT(__scmi_shmem_end - __scmi_shmem_start == 256, "SCMI reservation must be exactly 256 bytes")
ASSERT((__scmi_shmem_start & 63) == 0, "SCMI alignment")
ASSERT(__scmi_shmem_start >= __ebss, "SCMI overlaps BSS")
ASSERT(__scmi_shmem_end <= __app_limit, "SCMI exceeds application SRAM")
"#
    } else { "" };
    fs::write(out_dir.join("rp1-scmi.x"), scmi).unwrap();

    let mut linker = fs::read_to_string(PathBuf::from(&manifest_dir).join("link.x")).unwrap();
    if local_monitor {
        // Shared code/data still load in place below e000. Proc-local stack is
        // allocated, but never uploaded or BSS-cleared by the shared loader.
        linker = format!("PHDRS {{ shared PT_LOAD FLAGS(7); local PT_NULL FLAGS(6); }}\n{linker}")
            .replacen("> RP1_APP_SRAM", "> RP1_APP_SRAM :shared", 1)
            .replace("    __data_end = .;", "    . = ALIGN(8);\n    __data_end = .;");
        linker.push_str(r#"
SECTIONS {
  .local_monitor_stack 0x10003800 (NOLOAD) : ALIGN(8) {
    __local_monitor_stack_start = .;
    KEEP(*(.local_monitor_stack));
    __local_monitor_stack_end = .;
  } > RP1_LOCAL_MONITOR :local
  ASSERT(__local_monitor_stack_start == 0x10003800, "local monitor base")
  ASSERT(__local_monitor_stack_end == 0x10004000, "local monitor 512-word budget")
}
"#);
    }
    fs::write(out_dir.join("link.x"), linker).unwrap();
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rustc-link-search={}", manifest_dir);
}
