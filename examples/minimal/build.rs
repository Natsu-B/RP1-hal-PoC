fn main() {
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_BULK").is_some() {
        generate_amp_resources();
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_ENDPOINT_CONFIG_ONCE").is_some() {
        // Exact local closure only. Dependency feature unions still need sealed-build review.
        const ALLOWED: &[&str] = &["BAR2_READONLY_HANDSHAKE", "DEBUG_MAILBOX_LAYOUT",
            "DEBUG_STACK_LOW", "ENDPOINT_CLOCK_ONLY", "ENDPOINT_CONFIG_FOUNDATION",
            "FREERTOS_ENDPOINT_CONFIG_ONCE", "FREERTOS_ENDPOINT_UART", "FREERTOS_R1", "FREERTOS_TIMESYNC", "FREERTOS_DDR", "FREERTOS_VIRTIO_PROBE", "FREERTOS_OPENAMP", "FREERTOS_OPENAMP_RPMSG", "FREERTOS_AMP_BULK", "FREERTOS_AMP_BULK_DDR_DIAGNOSTICS", "FREERTOS_AMP_CONTROL", "FREERTOS_AMP_PROC1",
            "FREERTOS_AMP_PROC1_GPIO_IRQ", "FREERTOS_AMP_OBSERVATION", "FREERTOS_AMP_PROC1_PROFILE", "FREERTOS_SCMI_READONLY", "PLL_SYS_CORE_LOCK_ONLY", "STATE3_COMPOSITE_BOUNDARY",
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
    if std::env::var_os("CARGO_FEATURE_FREERTOS_OPENAMP").is_some() {
        assert!(std::env::var_os("CARGO_FEATURE_FREERTOS_VIRTIO_PROBE").is_none());
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("openamp.x");
        std::fs::write(&path, r#"
SECTIONS {
 .openamp_control 0x2000da40 (NOLOAD) : { KEEP(*(.openamp_control)); } > RP1_APP_SRAM
 .openamp_telemetry 0x2000da80 (NOLOAD) : { KEEP(*(.openamp_telemetry)); } > RP1_APP_SRAM
 .openamp_state 0x2000db80 (NOLOAD) : { KEEP(*(.openamp_state)); } > RP1_APP_SRAM
} INSERT AFTER .ddr;
ASSERT(SIZEOF(.openamp_control) == 64, "OpenAMP control ABI")
ASSERT(SIZEOF(.openamp_telemetry) == 256, "OpenAMP telemetry ABI")
ASSERT(ADDR(.openamp_control) >= __ebss, "OpenAMP overlaps BSS/tasks")
ASSERT(ADDR(.openamp_control) >= __scmi_shmem_end, "OpenAMP overlaps SCMI")
ASSERT(ADDR(.openamp_control) >= ADDR(.timesync) + SIZEOF(.timesync), "OpenAMP overlaps TimeSync")
ASSERT(ADDR(.openamp_control) >= ADDR(.ddr) + SIZEOF(.ddr), "OpenAMP overlaps DDR RPC")
ASSERT(ADDR(.openamp_state) >= ADDR(.openamp_telemetry) + SIZEOF(.openamp_telemetry), "OpenAMP state overlaps telemetry")
ASSERT(ADDR(.openamp_state) + SIZEOF(.openamp_state) <= __app_limit, "OpenAMP exceeds app SRAM")
ASSERT(__app_limit <= _stack_start - 4096, "OpenAMP overlaps MSP")
"#).unwrap();
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_BULK").is_some() {
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("amp-bulk.x");
        std::fs::write(&path, r#"
ASSERT(DEFINED(rp1_bulk_receive), "bulk endpoint not linked")
ASSERT(SIZEOF(.openamp_state) <= 0x480, "bulk and OpenAMP exceed shared SRAM reservation")
"#).unwrap();
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_BULK_DDR_DIAGNOSTICS").is_some() || std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_CONTROL").is_some() {
        // Reuse the existing 2304-word pool; each of the eight task stacks is unchanged.
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("amp-ddr-diagnostics.x");
        std::fs::write(&path, r#"
SECTIONS {
 .ddr_diagnostics_text 0x2000d290 : { KEEP(*(.ddr_diagnostics_text)); } > RP1_APP_SRAM
} INSERT AFTER .rp1_clock_profile;
ASSERT(__ebss <= ADDR(.rp1_clock_profile), "diagnostics overlap task storage")
ASSERT(ADDR(.rp1_clock_profile) + SIZEOF(.rp1_clock_profile) <= ADDR(.ddr_diagnostics_text), "diagnostics overlap clock profile")
ASSERT(SIZEOF(.ddr_diagnostics_text) > 0, "diagnostics missing")
ASSERT(ADDR(.ddr_diagnostics_text) + SIZEOF(.ddr_diagnostics_text) <= 0x2000d7c0, "diagnostics overlap SCMI")
ASSERT(__app_limit <= _stack_start - 4096, "diagnostics overlap MSP")
"#).unwrap();
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    rp1_build::generate().expect("generate RP1 note");
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_CONTROL").is_some() {
        assert!(std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_BULK_DDR_DIAGNOSTICS").is_none(),
            "AMP control owns a separate two-task pool; R1 diagnostic stacks are not compatible");
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("amp-control.x");
        std::fs::write(&path, "SECTIONS { .amp_control_contract : { KEEP(*(.amp_control_contract)); } > RP1_APP_SRAM } INSERT AFTER .rp1_clock_profile;\nASSERT(SIZEOF(.amp_control_contract) == 24, \"AMP control contract size\")\nASSERT(ADDR(.amp_control_contract) + SIZEOF(.amp_control_contract) <= 0x2000d7c0, \"AMP control overlaps SCMI\")\n").unwrap();
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_R1_CRITICAL_TIMING").is_some() {
        println!("cargo:rustc-link-arg=--wrap=vPortEnterCritical");
        println!("cargo:rustc-link-arg=--wrap=vPortExitCritical");
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_PROC1").is_some() {
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("amp-proc1.x");
        std::fs::write(&path, r#"
SECTIONS {
 .amp_proc1_storage 0x2000ba00 (NOLOAD) : { KEEP(*(.amp_proc1.storage)); } > RP1_APP_SRAM
 .amp_proc1_vectors 0x2000be00 : { __amp_proc1_vectors = .; KEEP(*(.amp_proc1.vectors)); } > RP1_APP_SRAM
 .amp_proc1_stack 0x2000bf40 (NOLOAD) : {
  __amp_proc1_guard_low = .;
  __amp_proc1_stack_low = . + 8; __amp_proc1_stack_top = __amp_proc1_stack_low + 2048;
  __amp_proc1_guard_high = __amp_proc1_stack_top;
  KEEP(*(.amp_proc1.stack));
 } > RP1_APP_SRAM
} INSERT AFTER .amp_control_contract;
ASSERT(ADDR(.amp_control_contract) + SIZEOF(.amp_control_contract) <= 0x2000ba00, "proc1 overlaps proc0/clock contract")
ASSERT(SIZEOF(.amp_proc1_storage) <= 0x400, "proc1 storage exceeds owned local SRAM")
ASSERT(SIZEOF(.amp_proc1_vectors) == 320, "proc1 vector table size")
ASSERT(ADDR(.amp_proc1_vectors) % 512 == 0, "proc1 VTOR alignment")
ASSERT(ADDR(.amp_proc1_stack) + SIZEOF(.amp_proc1_stack) <= ADDR(.ddr_diagnostics_text), "proc1 overlaps DDR diagnostics")
ASSERT(DEFINED(rp1_control_session) && DEFINED(rp1_control_retire), "proc1 handoff hooks missing")
ASSERT(DEFINED(AmpProc1Body) && DEFINED(AmpProc1Entry), "proc1 runtime missing")
"#).unwrap();
        if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_PROC1_GPIO_IRQ").is_some() {
            use std::io::Write;
            std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(br#"
SECTIONS {
 .amp_proc1_irq_storage 0x2000c750 (NOLOAD) : {
  KEEP(*(.amp_proc1.irq_status)); KEEP(*(.amp_proc1.irq_handle));
 } > RP1_APP_SRAM
 .amp_proc1_irq_text 0x2000c7e0 : {
  KEEP(*(.amp_proc1.irq_text)); KEEP(*(.amp_proc1.irq_contract));
 } > RP1_APP_SRAM
} INSERT AFTER .amp_proc1_stack;
ASSERT(SIZEOF(.amp_proc1_irq_storage) >= 128 && SIZEOF(.amp_proc1_irq_storage) <= 0x90, "GPIO IRQ local status/handle budget")
ASSERT(ADDR(.amp_proc1_irq_storage) >= ADDR(.amp_proc1_stack) + SIZEOF(.amp_proc1_stack), "GPIO IRQ overlaps C1 stack")
ASSERT(ADDR(.amp_proc1_irq_text) + SIZEOF(.amp_proc1_irq_text) <= ADDR(.ddr_diagnostics_text), "GPIO IRQ overlaps diagnostics")
ASSERT(DEFINED(AmpProc1GpioIrq), "GPIO IRQ vector missing")
"#).unwrap();
        }
        println!("cargo:rustc-link-arg=-T{}", path.display());
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_OBSERVATION").is_some() {
        assert!(std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_PROC1_GPIO_IRQ").is_some());
        let path=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("amp-observation.x");
        std::fs::write(&path,r#"
SECTIONS {
 .amp_observation_state 0x2000ce00 (NOLOAD) : { KEEP(*(.amp_observation_state)); } > RP1_APP_SRAM
 .amp_observation_text ALIGN(ADDR(.amp_observation_state) + SIZEOF(.amp_observation_state), 4) : { KEEP(*(.amp_observation_text)); } > RP1_APP_SRAM
} INSERT AFTER .amp_proc1_irq_text;
SECTIONS {
 .amp_observation_codec ALIGN(ADDR(.openamp_state) + SIZEOF(.openamp_state), 4) : { KEEP(*(.amp_observation_codec)); } > RP1_APP_SRAM
} INSERT AFTER .openamp_state;
ASSERT(ADDR(.amp_proc1_irq_text) + SIZEOF(.amp_proc1_irq_text) <= ADDR(.amp_observation_state), "collector overlaps C1 IRQ code")
ASSERT(SIZEOF(.amp_observation_state) > 0 && SIZEOF(.amp_observation_state) <= 0x490, "collector state budget")
ASSERT(ADDR(.amp_observation_state) + SIZEOF(.amp_observation_state) <= ADDR(.ddr_diagnostics_text), "collector overlaps DDR diagnostics")
ASSERT(ADDR(.amp_observation_text) >= ADDR(.amp_observation_state) + SIZEOF(.amp_observation_state), "collector code overlaps state")
ASSERT(SIZEOF(.amp_observation_text) > 0 && ADDR(.amp_observation_text) + SIZEOF(.amp_observation_text) <= ADDR(.ddr_diagnostics_text), "collector code exceeds local gap")
ASSERT(DEFINED(rp1_control_take_trace), "collector consumer handoff missing")
ASSERT(SIZEOF(.amp_observation_codec) > 0 && ADDR(.amp_observation_codec) + SIZEOF(.amp_observation_codec) <= __app_limit, "collector codec exceeds OpenAMP tail")
"#).unwrap();
        println!("cargo:rustc-link-arg=-T{}",path.display());
    }
    if std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_PROC1_PROFILE").is_some() {
        assert!(std::env::var_os("CARGO_FEATURE_FREERTOS_AMP_OBSERVATION").is_some());
        let path=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("amp-proc1-profile.x");
        // Code above the MSP top; scratch 0x2000f600 and results 0x2000f700..0x2000f800 stay unlinked.
        std::fs::write(&path,r#"
MEMORY { RP1_PROFILE_TEXT (rx) : ORIGIN = 0x2000f000, LENGTH = 0x600 }
SECTIONS {
 .amp_proc1_profile_text 0x2000f000 : { KEEP(*(.amp_proc1.profile_text)); } > RP1_PROFILE_TEXT
} INSERT BEFORE .inbound_dummy_page;
ASSERT(_stack_start == 0x2000f000, "profile text must sit above the fixed MSP top")
ASSERT(SIZEOF(.amp_proc1_profile_text) > 0 && SIZEOF(.amp_proc1_profile_text) <= 0x600, "profile text exceeds 0x2000f000..0x2000f600 (scratch follows)")
"#).unwrap();
        println!("cargo:rustc-link-arg=-T{}",path.display());
    }
}

fn generate_amp_resources() {
    use std::{env, fs, path::PathBuf, process::Command};
    println!("cargo:rerun-if-env-changed=RP1_AMP_GENERATOR");
    println!("cargo:rerun-if-env-changed=RP1_AMP_RECORD_DIR");
    let generator = PathBuf::from(env::var_os("RP1_AMP_GENERATOR")
        .expect("freertos-amp-bulk requires RP1_AMP_GENERATOR (amp-deployment.py)"))
        .canonicalize().expect("AMP generator path");
    let profiles = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../profiles").canonicalize().unwrap();
    let profile = profiles.join("amp-deployment.json");
    println!("cargo:rerun-if-changed={}", generator.display());
    println!("cargo:rerun-if-changed={}", profiles.display());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("amp-policy");
    // This directory contains only this build script's disposable products.
    if out.exists() { fs::remove_dir_all(&out).unwrap(); }
    let status = Command::new("python3").arg("-B").arg(&generator).arg(&profile)
        .arg("--output").arg(&out).status().expect("run AMP generator");
    assert!(status.success(), "AMP policy rejected");
    if let Some(record) = env::var_os("RP1_AMP_RECORD_DIR") {
        let record = PathBuf::from(record);
        fs::create_dir(&record).expect("new AMP evidence directory");
        for entry in fs::read_dir(&out).unwrap() {
            let path = entry.unwrap().path();
            fs::copy(&path, record.join(path.file_name().unwrap())).unwrap();
        }
        fs::copy(profile, record.join("amp-deployment.json")).unwrap();
        fs::copy(generator, record.join("generator.py")).unwrap();
    }
}
