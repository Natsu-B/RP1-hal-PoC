fn main() {
    if std::env::var_os("CARGO_FEATURE_SCMI_UART_COEXIST").is_some() {
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let config = manifest.join("rp1-scmi-uart.toml");
        println!("cargo:rerun-if-changed={}", config.display());
        let note = rp1_build::generate_from_paths(&config, &out).expect("generate SCMI RP1 note");
        println!("cargo:rustc-env=RP1_NOTE_BIN={}", note.display());
    } else {
        rp1_build::generate().expect("generate RP1 note");
    }
}
