fn main() {
    if std::env::var_os("CARGO_FEATURE_FREERTOS_ENDPOINT_CONFIG_ONCE").is_some() {
        // Exact local closure only. Dependency feature unions still need sealed-build review.
        const ALLOWED: &[&str] = &["BAR2_READONLY_HANDSHAKE", "DEBUG_MAILBOX_LAYOUT",
            "DEBUG_STACK_LOW", "ENDPOINT_CLOCK_ONLY", "ENDPOINT_CONFIG_FOUNDATION",
            "FREERTOS_ENDPOINT_CONFIG_ONCE", "FREERTOS_ENDPOINT_UART", "FREERTOS_R1",
            "FREERTOS_SCMI_READONLY", "PLL_SYS_CORE_LOCK_ONLY", "STATE3_COMPOSITE_BOUNDARY",
            "STATE5_COMPOSITE_BOUNDARY", "UART0_FUNCTIONAL_CLOCK_BEFORE_RESET_DONE", "UART0_RESET_ONLY"];
        for (name, _) in std::env::vars_os() {
            if let Some(feature) = name.to_str().and_then(|v| v.strip_prefix("CARGO_FEATURE_")) {
                assert!(ALLOWED.contains(&feature), "config-once rejects extra feature {feature}");
            }
        }
        // Keep the sealed Linux RAM helper and boot observer's two SRAM ABIs.
        // Fail linking if LTO changes telemetry ordering or the BSS no longer fits.
        println!("cargo:rustc-link-arg=--section-start=.bss=0x2000a298");
        println!("cargo:rustc-link-arg=--section-start=.scmi_shmem=0x2000d7c0");
        let abi = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("config-once-abi.x");
        std::fs::write(&abi, "ASSERT(RP1_SCMI_TELEMETRY == 0x2000a298, \"config-once telemetry ABI moved\")\nASSERT(__scmi_shmem_start == 0x2000d7c0, \"config-once shmem ABI moved\")\n").unwrap();
        println!("cargo:rustc-link-arg=-T{}", abi.display());
    }
    rp1_build::generate().expect("generate RP1 note");
    if std::env::var_os("CARGO_FEATURE_FREERTOS_R1_CRITICAL_TIMING").is_some() {
        println!("cargo:rustc-link-arg=--wrap=vPortEnterCritical");
        println!("cargo:rustc-link-arg=--wrap=vPortExitCritical");
    }
}
