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
                .args(env::var_os("CARGO_FEATURE_SYNC_POOL_R1")
                    .filter(|_| source.ends_with("c/bridge.c"))
                    .map(|_| "-DRP1_FREERTOS_SYNC_POOL_R1=1"))
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
    for (feature, members) in [
        ("memcpy", &[
            ("memcpy", "0fdf219488b13b471c3b667e5450cba6e4982cbb672624159453a311a92cde73", "", "memcpy"),
            ("aeabi_memcpy", "c60f15b208dbfd9ba3857c0b24abf198df018813001e820607a878b2749cc3be", "U memcpy", "__aeabi_memcpy __aeabi_memcpy4 __aeabi_memcpy8"),
        ][..]),
        ("memset", &[
            ("memset", "e35e6e2ee6e2c57a8775b57520cbff6f2b8eb82eac11b18c277347c64399e07d", "", "memset"),
            ("aeabi_memset", "76454e09c3d824631080c0d6e27a1904a77a799c0fd0d609168094932165d881", "U memset", "__aeabi_memset __aeabi_memset4 __aeabi_memset8"),
            ("aeabi_memclr", "207ab5288298b9b329767a83dc5da3526d4db311c17dc39a5bcb9f52047c047c", "U __aeabi_memset", "__aeabi_memclr __aeabi_memclr4 __aeabi_memclr8"),
        ][..]),
    ] {
        if env::var_os(format!("CARGO_FEATURE_NEWLIB_{}", feature.to_uppercase())).is_none() {
            continue;
        }
        // Reuse installed, pinned Cortex-M3 soft-ABI libc members only.
        // No full libc, allocator, syscalls, or home-grown memory implementation.
        let archive = output(Command::new(&cc).args([
            "-mcpu=cortex-m3", "-mthumb", "-mfloat-abi=soft", "-print-file-name=libc.a"]));
        assert!(Path::new(&archive).is_file(), "Cortex-M3 newlib archive missing");
        println!("cargo:rerun-if-changed={archive}");
        let mut provenance = format!("archive={archive}\n");
        for &(name, expected, dependencies, definitions) in members {
            let member_name = format!("libc_a-{name}.o");
            let member = Command::new(&ar).args(["p", &archive, &member_name])
                .output().expect("extract newlib memory member");
            assert!(member.status.success(), "newlib {feature} extraction failed");
            let object = Path::new(&out).join(format!("newlib-{name}.o"));
            fs::write(&object, member.stdout).unwrap();
            let hash = output(Command::new("sha256sum").arg(&object));
            assert_eq!(hash.split_whitespace().next().unwrap(), expected,
                "newlib {feature} change requires explicit source/ABI review");
            assert_eq!(output(Command::new("arm-none-eabi-nm").arg("-u").arg(&object)),
                dependencies, "{feature} members must have no other runtime dependencies");
            let defined = output(Command::new("arm-none-eabi-nm")
                .args(["--defined-only", "--extern-only", "--format=posix"]).arg(&object));
            assert_eq!(defined.lines().map(|line| line.split_whitespace().next().unwrap())
                .collect::<Vec<_>>().join(" "), definitions, "unexpected {feature} symbols");
            provenance.push_str(&format!("member={member_name}\n{hash}\nundefined={dependencies}\ndefined={definitions}\n"));
            objects.push(object);
        }
        fs::write(Path::new(&out).join(format!("newlib-{feature}.txt")), provenance).unwrap();
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
