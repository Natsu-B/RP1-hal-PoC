use std::{env, fs, path::Path, process::Command};

const COMMIT: &str = "3a22924e0a9ddbbc8b0758881c33b3422a5cc20d";
const GCC_VERSION: &str = "14.2.1";

fn output(command: &mut Command) -> String {
    let result = command.output().expect("run build tool");
    assert!(
        result.status.success(),
        "{command:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout)
        .expect("tool output UTF-8")
        .trim()
        .into()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=c");
    println!("cargo:rerun-if-env-changed=RP1_FREERTOS_CC");
    println!("cargo:rerun-if-env-changed=RP1_FREERTOS_AR");
    let target = env::var("TARGET").unwrap();
    // Host tests only exercise Rust boundary checks, never an emulated kernel.
    if env::var("HOST").as_deref() == Ok(target.as_str()) {
        println!(
            "cargo:warning=rp1-freertos: host build contains no kernel; only pure API tests are usable"
        );
        return;
    }
    assert_eq!(
        target, "thumbv7m-none-eabi",
        "only proc0 Cortex-M3 is supported"
    );
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let vendor = Path::new(&root)
        .join("../../third-party/FreeRTOS-Kernel")
        .canonicalize()
        .expect("initialize the pinned third-party/FreeRTOS-Kernel submodule");
    let git = |args: &[&str]| output(Command::new("git").arg("-C").arg(&vendor).args(args));
    assert_eq!(
        Path::new(&git(&["rev-parse", "--show-toplevel"])),
        vendor,
        "FreeRTOS must be its own pinned git checkout"
    );
    assert_eq!(
        git(&["rev-parse", "HEAD"]),
        COMMIT,
        "wrong FreeRTOS Kernel commit"
    );
    assert!(
        git(&["status", "--porcelain", "--untracked-files=no"]).is_empty(),
        "modified FreeRTOS vendor source is not supported"
    );
    println!("cargo:rerun-if-changed={}", vendor.display());
    let git_dir = git(&["rev-parse", "--absolute-git-dir"]);
    println!("cargo:rerun-if-changed={git_dir}/HEAD");
    println!("cargo:rerun-if-changed={git_dir}/index");
    let cc = env::var("RP1_FREERTOS_CC").unwrap_or_else(|_| "arm-none-eabi-gcc".into());
    let ar = env::var("RP1_FREERTOS_AR").unwrap_or_else(|_| "arm-none-eabi-ar".into());
    assert_eq!(
        output(Command::new(&cc).arg("-dumpmachine")),
        "arm-none-eabi"
    );
    assert_eq!(
        output(Command::new(&cc).arg("-dumpfullversion")),
        GCC_VERSION,
        "compiler change requires an explicit toolchain review"
    );
    let out = env::var("OUT_DIR").unwrap();
    let compiler = output(Command::new(&cc).arg("--version"));
    let archiver = output(Command::new(&ar).arg("--version"));
    let rustc = output(Command::new(env::var("RUSTC").unwrap()).arg("--version"));
    assert_eq!(rustc, "rustc 1.96.0 (ac68faa20 2026-05-25)",
        "RTOS Rust toolchain change requires explicit review");
    fs::write(
        Path::new(&out).join("toolchain.txt"),
        format!(
            "kernel={COMMIT}\ntarget={target}\ncc={cc}\n{compiler}\nar={ar}\n{archiver}\n{rustc}\n"
        ),
    )
    .unwrap();
    println!(
        "cargo:warning=rp1-freertos: kernel {COMMIT}; {}; {rustc}",
        compiler.lines().next().unwrap()
    );
    let mut objects = Vec::new();
    let mut sources = vec![
        vendor.join("tasks.c"),
        vendor.join("list.c"),
        vendor.join("queue.c"),
        vendor.join("portable/GCC/ARM_CM3/port.c"),
        Path::new(&root).join("c/bridge.c"),
    ];
    if env::var_os("CARGO_FEATURE_CRITICAL_TIMING").is_some() {
        sources.push(Path::new(&root).join("c/critical_timing.c"));
    }
    for (index, source) in sources.iter().enumerate() {
        let object = Path::new(&out).join(format!("freertos-{index}.o"));
        output(
            Command::new(&cc)
                .args([
                    "-std=c11",
                    "-mcpu=cortex-m3",
                    "-mthumb",
                    "-mfloat-abi=soft",
                    "-Os",
                    "-ffunction-sections",
                    "-fdata-sections",
                    "-ffreestanding",
                    "-fno-common",
                    "-fno-unwind-tables",
                    "-fno-asynchronous-unwind-tables",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                ])
                .args(env::var_os("CARGO_FEATURE_ASSERT_PROBE")
                    .map(|_| "-DRP1_FREERTOS_ASSERT_PROBE=1"))
                .args(env::var_os("CARGO_FEATURE_TASK_POOL_2304")
                    .map(|_| "-DRP1_FREERTOS_TASK_POOL_2304=1"))
                .arg("-I")
                .arg(Path::new(&root).join("c"))
                .arg("-I")
                .arg(vendor.join("include"))
                .arg("-I")
                .arg(vendor.join("portable/GCC/ARM_CM3"))
                .arg("-c")
                .arg(source)
                .arg("-o")
                .arg(&object),
        );
        objects.push(object);
    }
    output(
        Command::new(ar)
            .arg("crs")
            .arg(Path::new(&out).join("librp1_freertos.a"))
            .args(objects),
    );
    println!("cargo:rustc-link-search=native={out}");
    println!("cargo:rustc-link-lib=static=rp1_freertos");
}
