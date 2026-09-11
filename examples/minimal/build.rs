fn main() {
    rp1_build::generate().expect("generate RP1 note");
    if std::env::var_os("CARGO_FEATURE_FREERTOS_R1_CRITICAL_TIMING").is_some() {
        println!("cargo:rustc-link-arg=--wrap=vPortEnterCritical");
        println!("cargo:rustc-link-arg=--wrap=vPortExitCritical");
    }
}
