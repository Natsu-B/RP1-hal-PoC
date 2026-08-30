use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use goblin::elf::Elf;
use goblin::elf::program_header::PT_LOAD;

const RP1_SRAM_BASE: u64 = 0x2000_0000;
const RP1_SRAM_LIMIT: u64 = 0x2001_0000;
const RP1_MAX_IMAGE_LEN: u64 = RP1_SRAM_LIMIT - RP1_SRAM_BASE;
const TARGET: &str = "thumbv7m-none-eabi";

#[derive(Parser)]
#[command(name = "cargo-rp1")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Subcommand)]
enum CommandKind {
    Build(BuildArgs),
}

#[derive(clap::Args)]
struct BuildArgs {
    #[arg(long)]
    example: String,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    no_default_features: bool,
    #[arg(long, value_delimiter = ',')]
    features: Vec<String>,
}

struct ElfCheck {
    entry: u64,
    load_start: u64,
    load_end: u64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CommandKind::Build(args) => build(args),
    }
}

fn build(args: BuildArgs) -> Result<()> {
    if args.example != "minimal" {
        bail!(
            "unsupported example `{}`; only `minimal` is wired up",
            args.example
        );
    }

    let root = std::env::current_dir().context("read current directory")?;
    let example_dir = root.join("examples").join("minimal");
    let config_path = args
        .config
        .clone()
        .or_else(|| std::env::var_os("RP1_CONFIG").map(PathBuf::from))
        .unwrap_or_else(|| example_dir.join("rp1.toml"));
    let config = rp1_build::parse_config(&config_path)
        .map_err(|err| anyhow::anyhow!("parse {}: {err}", config_path.display()))?;
    validate_config_features(&config, &args.features)?;
    let package = "rp1-example-minimal";
    let target_dir = cargo_target_dir(&root);
    let raw_elf = target_dir
        .join(TARGET)
        .join("release")
        .join(package);
    let output_dir = target_dir.join("rp1").join("release");
    let output_elf = output_dir.join("RP1.elf");

    println!("[RP1] building example {}", args.example);
    let mut cargo = Command::new("cargo");
    cargo
        .arg("build")
        .arg("-p")
        .arg(package)
        .arg("--release")
        .arg("--target")
        .arg(TARGET);
    if args.no_default_features {
        cargo.arg("--no-default-features");
    }
    if !args.features.is_empty() {
        cargo.arg("--features").arg(args.features.join(","));
    }
    if config_path != example_dir.join("rp1.toml") {
        cargo.env("RP1_CONFIG", &config_path);
    }
    run(&mut cargo)?;

    fs::create_dir_all(&output_dir).context("create target/rp1/release")?;
    let note_bin = write_invocation_note(&config, &output_dir)?;
    attach_note(&raw_elf, &note_bin, &output_elf)?;
    let check = check_bootloader_compatible(&output_elf)?;

    println!(
        "[RP1] note: owner_rp1=0x{:x} owner_linux=0x{:x} owner_disabled=0x{:x} mailbox=0x{:x} version_kind={}",
        config.owner_rp1,
        config.owner_linux,
        config.owner_disabled,
        config.mailbox_flags,
        config.firmware_version_kind
    );
    println!("[RP1] raw elf: {}", display_path(&root, &raw_elf));
    println!("[RP1] note bin: {}", display_path(&root, &note_bin));
    println!("[RP1] output: {}", display_path(&root, &output_elf));
    println!("[RP1] entry: 0x{:08x}", check.entry);
    println!(
        "[RP1] load: 0x{:08x}..0x{:08x}",
        check.load_start, check.load_end
    );
    Ok(())
}

fn validate_config_features(config: &rp1_build::Rp1BuildConfig, features: &[String]) -> Result<()> {
    let has_feature = |name: &str| features.iter().any(|feature| feature == name);
    rp1_build::validate_layout_features(
        config,
        has_feature("debug-mailbox-layout-v1"),
        has_feature("debug-mailbox-layout-v2"),
    )
    .map_err(|err| anyhow::anyhow!("{err}"))
}

fn run(command: &mut Command) -> Result<()> {
    let status = command.status().context("spawn cargo build")?;
    if !status.success() {
        bail!("cargo build failed with {status}");
    }
    Ok(())
}

fn attach_note(input: &Path, note_bin: &Path, output: &Path) -> Result<()> {
    let status = Command::new("llvm-objcopy")
        .arg("--add-section")
        .arg(format!(".note.rp1={}", note_bin.display()))
        .arg("--set-section-flags")
        .arg(".note.rp1=alloc,readonly")
        .arg(input)
        .arg(output)
        .status()
        .context("spawn llvm-objcopy")?;
    if !status.success() {
        bail!("llvm-objcopy failed with {status}");
    }
    Ok(())
}

fn write_invocation_note(config: &rp1_build::Rp1BuildConfig, output_dir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(output_dir).context("create note output dir")?;
    let note = output_dir.join("rp1_note.bin");
    rp1_build::write_note_bin(config, &note).map_err(|err| anyhow::anyhow!("{err}"))?;
    Ok(note)
}

fn check_bootloader_compatible(path: &Path) -> Result<ElfCheck> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let elf = Elf::parse(&bytes).with_context(|| format!("parse ELF {}", path.display()))?;

    if elf
        .section_headers
        .iter()
        .filter_map(|header| elf.shdr_strtab.get_at(header.sh_name))
        .all(|name| name != ".note.rp1")
    {
        bail!("output ELF is missing .note.rp1");
    }

    if elf.entry < RP1_SRAM_BASE || elf.entry >= RP1_SRAM_LIMIT {
        bail!(
            "entry 0x{:08x} is outside RP1 SRAM range 0x{:08x}..0x{:08x}",
            elf.entry,
            RP1_SRAM_BASE,
            RP1_SRAM_LIMIT
        );
    }

    let mut load_start = u64::MAX;
    let mut load_end = RP1_SRAM_BASE;
    let mut count = 0usize;
    for ph in &elf.program_headers {
        if ph.p_type != PT_LOAD {
            continue;
        }
        let end = ph
            .p_paddr
            .checked_add(ph.p_memsz)
            .context("PT_LOAD address overflow")?;
        if ph.p_paddr < RP1_SRAM_BASE || end > RP1_SRAM_LIMIT {
            bail!(
                "PT_LOAD p_paddr=0x{:08x}..0x{:08x} is outside RP1 SRAM range 0x{:08x}..0x{:08x}",
                ph.p_paddr,
                end,
                RP1_SRAM_BASE,
                RP1_SRAM_LIMIT
            );
        }
        load_start = load_start.min(ph.p_paddr);
        load_end = load_end.max(end);
        count += 1;
    }

    if count == 0 {
        bail!("output ELF has no PT_LOAD segments");
    }
    let image_len = load_end
        .checked_sub(RP1_SRAM_BASE)
        .context("invalid image length")?;
    if image_len == 0 || image_len > RP1_MAX_IMAGE_LEN {
        bail!(
            "image_len 0x{:x} is outside allowed range 1..=0x{:x}",
            image_len,
            RP1_MAX_IMAGE_LEN
        );
    }

    Ok(ElfCheck {
        entry: elf.entry,
        load_start,
        load_end,
    })
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn cargo_target_dir(root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from) {
        Some(path) if path.is_absolute() => path,
        Some(path) => root.join(path),
        None => root.join("target"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FLAG_ENABLE: u32 = 0x1;
    const FLAG_V1: u32 = 0x2;
    const FLAG_V2: u32 = 0x4;

    fn config(flags: u32) -> rp1_build::Rp1BuildConfig {
        rp1_build::Rp1BuildConfig {
            owner_rp1: 0,
            owner_linux: 0,
            owner_disabled: 0,
            mailbox_flags: flags,
            firmware_version_kind: 0,
        }
    }

    fn note_flags(path: &Path) -> u32 {
        let note = fs::read(path).unwrap();
        u32::from_le_bytes(note[88..92].try_into().unwrap())
    }

    #[test]
    fn invocation_note_selection_ignores_stale_cargo_notes() {
        let root =
            std::env::temp_dir().join(format!("cargo-rp1-note-selection-{}", std::process::id()));
        let stale = root
            .join("target")
            .join(TARGET)
            .join("release")
            .join("build")
            .join("rp1-example-minimal-stale")
            .join("out");
        fs::create_dir_all(&stale).unwrap();
        fs::write(stale.join("rp1_note.bin"), [0xffu8; 96]).unwrap();

        let out = root.join("target").join("rp1").join("release");
        for (flags, expected) in [
            (FLAG_ENABLE, 0x1),
            (FLAG_ENABLE | FLAG_V1, 0x3),
            (FLAG_ENABLE | FLAG_V2, 0x5),
        ] {
            let note = write_invocation_note(&config(flags), &out).unwrap();
            assert_eq!(note_flags(&note), expected);
        }

        let _ = fs::remove_dir_all(root);
    }
}
