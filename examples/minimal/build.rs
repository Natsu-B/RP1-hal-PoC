fn main() {
    if std::env::var_os("CARGO_FEATURE_FREERTOS_ENDPOINT_CONFIG_ONCE").is_some() {
        // Exact local closure only. Dependency feature unions still need sealed-build review.
        const ALLOWED: &[&str] = &["BAR2_READONLY_HANDSHAKE", "DEBUG_MAILBOX_LAYOUT",
            "DEBUG_STACK_LOW", "ENDPOINT_CLOCK_ONLY", "ENDPOINT_CONFIG_FOUNDATION",
            "FREERTOS_ENDPOINT_CONFIG_ONCE", "FREERTOS_ENDPOINT_UART", "FREERTOS_R1", "FREERTOS_TIMESYNC", "FREERTOS_DDR", "FREERTOS_VIRTIO_PROBE",
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
        if std::env::var_os("CARGO_FEATURE_FREERTOS_TIMESYNC").is_some() {
            let path = abi.with_file_name("timesync.x");
            std::fs::write(&path, "SECTIONS { .timesync 0x2000d900 (NOLOAD) : { KEEP(*(.timesync)); } > RP1_APP_SRAM } INSERT AFTER .scmi_shmem;\nASSERT(ADDR(.timesync) >= __scmi_shmem_end, \"TimeSync overlaps SCMI\")\nASSERT(SIZEOF(.timesync) == 64, \"TimeSync ABI size\")\nASSERT(ADDR(.timesync) + SIZEOF(.timesync) <= __app_limit, \"TimeSync exceeds SRAM\")\n").unwrap();
            println!("cargo:rustc-link-arg=-T{}", path.display());
        }
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_DDR").is_some() {
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("ddr.x");
        std::fs::write(&path, "SECTIONS { .ddr 0x2000d940 (NOLOAD) : { KEEP(*(.ddr)); } > RP1_APP_SRAM } INSERT AFTER .timesync;\nASSERT(SIZEOF(.ddr) == 256, \"DDR record size\")\nASSERT(ADDR(.ddr) + SIZEOF(.ddr) <= __app_limit, \"DDR exceeds SRAM\")\n").unwrap();
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_VIRTIO_PROBE").is_some() {
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("virtio-probe.x");
        std::fs::write(&path, r#"
SECTIONS {
 .virtio_mmio 0x2000da40 (NOLOAD) : { KEEP(*(.virtio_mmio)); } > RP1_APP_SRAM
 .openamp_telemetry 0x2000db40 (NOLOAD) : { KEEP(*(.openamp_telemetry)); } > RP1_APP_SRAM
} INSERT AFTER .ddr;
ASSERT(SIZEOF(.virtio_mmio) == 256, "VirtIO register bank size")
ASSERT(SIZEOF(.openamp_telemetry) == 64, "OpenAMP telemetry size")
ASSERT(ADDR(.virtio_mmio) >= __ebss, "VirtIO overlaps BSS/task storage")
ASSERT(ADDR(.virtio_mmio) >= __scmi_shmem_end, "VirtIO overlaps SCMI")
ASSERT(ADDR(.virtio_mmio) >= ADDR(.timesync) + SIZEOF(.timesync), "VirtIO overlaps TimeSync")
ASSERT(ADDR(.virtio_mmio) >= ADDR(.ddr) + SIZEOF(.ddr), "VirtIO overlaps DDR RPC")
ASSERT(ADDR(.openamp_telemetry) >= ADDR(.virtio_mmio) + SIZEOF(.virtio_mmio), "VirtIO overlaps telemetry")
ASSERT(ADDR(.openamp_telemetry) + SIZEOF(.openamp_telemetry) <= __app_limit, "OpenAMP overlaps reserved SRAM/MSP")
ASSERT(__app_limit <= _stack_start - 4096, "OpenAMP MSP separation")
"#).unwrap();
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    rp1_build::generate().expect("generate RP1 note");
    if std::env::var_os("CARGO_FEATURE_FREERTOS_R1_CRITICAL_TIMING").is_some() {
        println!("cargo:rustc-link-arg=--wrap=vPortEnterCritical");
        println!("cargo:rustc-link-arg=--wrap=vPortExitCritical");
    }
}
