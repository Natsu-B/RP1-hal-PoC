use std::{env, path::PathBuf, process::Command};
fn run(command: &mut Command) {
    assert!(command.status().expect("run OpenAMP build tool").success(), "OpenAMP build failed");
}
fn main() {
    println!("cargo:rerun-if-env-changed=RP1_OPENAMP_PREFIX");
    println!("cargo:rerun-if-changed=../../openamp/rp1.c");
    if env::var_os("CARGO_FEATURE_OPENAMP_RPMSG").is_none() { return; }
    let prefix=PathBuf::from(env::var_os("RP1_OPENAMP_PREFIX").expect("build pinned libraries with tools/build-openamp.py --rp1 first"));
    let out=PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let flags=["-mcpu=cortex-m3","-mthumb","-mfloat-abi=soft"];
    let source=PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../../openamp/rp1.c");
    run(Command::new("arm-none-eabi-gcc").args(flags).args(["-Os","-Wall","-Wextra","-Werror","-ffunction-sections","-fdata-sections","-DVIRTIO_DEVICE_SUPPORT=1","-DVIRTIO_DRIVER_SUPPORT=0","-DVIRTIO_USE_DCACHE","-I"]).arg(prefix.join("include")).arg("-c").arg(source).arg("-o").arg(out.join("rp1.o")));
    run(Command::new("arm-none-eabi-ar").arg("rcs").arg(out.join("librp1_openamp.a")).arg(out.join("rp1.o")));
    for name in ["libopen_amp.a","libmetal.a"] {
        let path=prefix.join("lib").join(name);
        println!("cargo:rerun-if-changed={}",path.display());
        let seal=Command::new("sha256sum").arg(path).output().unwrap();
        assert!(seal.status.success());
        println!("cargo:warning=OpenAMP input {}",String::from_utf8(seal.stdout).unwrap().trim());
    }
    println!("cargo:rustc-link-search=native={}",out.display());
    println!("cargo:rustc-link-search=native={}",prefix.join("lib").display());
    for lib in ["rp1_openamp","open_amp","metal"] { println!("cargo:rustc-link-lib=static={lib}"); }
    let libc=Command::new("arm-none-eabi-gcc").args(flags).arg("-print-file-name=libc.a").output().unwrap();
    let libc=PathBuf::from(String::from_utf8(libc.stdout).unwrap().trim());
    println!("cargo:rustc-link-search=native={}",libc.parent().unwrap().display());
    println!("cargo:rustc-link-lib=static=c");
}
