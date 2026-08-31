use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use rp1_abi::note::{
    RP1_MAILBOX_FLAG_ENABLE, RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1, RP1_MAILBOX_FLAG_SHARED_SRAM_V2,
    RP1_MAILBOX_FLAGS_SUPPORTED_MASK, RP1_NOTE_ABI_VERSION, RP1_NOTE_MAGIC, RP1_NOTE_NAME,
    RP1_NOTE_TYPE_BOOT_V1, RP1_VERSION_NON_PIO, Rp1BootInfoV1,
};
use rp1_abi::owner::{
    DEV_DMA, DEV_GPIO, DEV_I2C0, DEV_I2C1, DEV_PIO0, DEV_PIO1, DEV_SPI0, DEV_TIMER, DEV_UART0,
    DEV_UART1, bit,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Rp1Toml {
    firmware: Firmware,
    linux: Linux,
    owner: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct Firmware {
    name: String,
    #[serde(default)]
    mailbox_layout_v1: bool,
    #[serde(default)]
    shared_sram_layout_v2: bool,
}

#[derive(Debug, Deserialize)]
struct Linux {
    mailbox: bool,
    pio: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rp1BuildConfig {
    pub owner_rp1: u64,
    pub owner_linux: u64,
    pub owner_disabled: u64,
    pub mailbox_flags: u32,
    pub firmware_version_kind: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerBitmap {
    pub owner_rp1: u64,
    pub owner_linux: u64,
    pub owner_disabled: u64,
}

pub fn generate() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let config_path = resolve_config_path(
        &manifest_dir,
        std::env::var_os("RP1_CONFIG").map(PathBuf::from),
    );
    println!("cargo:rerun-if-env-changed=RP1_CONFIG");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V2");
    println!("cargo:rerun-if-changed={}", config_path.display());
    let config = parse_config(&config_path)?;
    validate_layout_features(
        &config,
        std::env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1").is_some(),
        std::env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V2").is_some(),
    )?;
    fs::create_dir_all(&out_dir)?;
    let note_path = out_dir.join("rp1_note.bin");
    write_note_bin(&config, &note_path)?;
    println!("cargo:rustc-env=RP1_NOTE_BIN={}", note_path.display());
    Ok(note_path)
}

pub fn validate_layout_features(
    config: &Rp1BuildConfig,
    v1_feature: bool,
    v2_feature: bool,
) -> Result<(), Box<dyn Error>> {
    let v1_config = config.mailbox_flags & RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1 != 0;
    let v2_config = config.mailbox_flags & RP1_MAILBOX_FLAG_SHARED_SRAM_V2 != 0;
    if v1_config && v2_config {
        return Err(
            "[firmware] mailbox_layout_v1 and shared_sram_layout_v2 are mutually exclusive".into(),
        );
    }
    if v1_config != v1_feature || v2_config != v2_feature {
        return Err(
            "[firmware] shared SRAM layout config must match enabled debug-mailbox-layout feature"
                .into(),
        );
    }
    Ok(())
}

pub fn generate_from_paths(config_path: &Path, out_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let config = parse_config(config_path)?;
    fs::create_dir_all(out_dir)?;
    let note_path = out_dir.join("rp1_note.bin");
    write_note_bin(&config, &note_path)?;
    Ok(note_path)
}

pub fn parse_config(path: impl AsRef<Path>) -> Result<Rp1BuildConfig, Box<dyn Error>> {
    let config_text = fs::read_to_string(path)?;
    parse_config_text(&config_text)
}

fn resolve_config_path(manifest_dir: &Path, config_path: Option<PathBuf>) -> PathBuf {
    let Some(path) = config_path else {
        return manifest_dir.join("rp1.toml");
    };
    if path.is_absolute() {
        return path;
    }
    let package_relative = manifest_dir.join(&path);
    if package_relative.exists() {
        return package_relative;
    }
    for ancestor in manifest_dir.ancestors().skip(1) {
        let candidate = ancestor.join(&path);
        if candidate.exists() {
            return candidate;
        }
    }
    package_relative
}

fn parse_config_text(config_text: &str) -> Result<Rp1BuildConfig, Box<dyn Error>> {
    let config: Rp1Toml = toml::from_str(&config_text)?;
    if config.firmware.mailbox_layout_v1 && config.firmware.shared_sram_layout_v2 {
        return Err("mailbox_layout_v1 and shared_sram_layout_v2 are mutually exclusive".into());
    }
    let owners = owner_bitmap(&config.owner)?;
    let _ = config.firmware.name.as_str();
    let _ = config.linux.pio;
    Ok(Rp1BuildConfig {
        owner_rp1: owners.owner_rp1,
        owner_linux: owners.owner_linux,
        owner_disabled: owners.owner_disabled,
        mailbox_flags: u32::from(config.linux.mailbox) * RP1_MAILBOX_FLAG_ENABLE
            | u32::from(config.firmware.mailbox_layout_v1) * RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1
            | u32::from(config.firmware.shared_sram_layout_v2) * RP1_MAILBOX_FLAG_SHARED_SRAM_V2,
        firmware_version_kind: RP1_VERSION_NON_PIO,
    })
}

pub fn write_note_bin(
    config: &Rp1BuildConfig,
    output: impl AsRef<Path>,
) -> Result<(), Box<dyn Error>> {
    validate_note_flags(config)?;
    fs::write(output, encode_note(config))?;
    Ok(())
}

pub fn validate_note_flags(config: &Rp1BuildConfig) -> Result<(), Box<dyn Error>> {
    if config.mailbox_flags & !RP1_MAILBOX_FLAGS_SUPPORTED_MASK != 0 {
        return Err("[firmware] unsupported mailbox flag bits are set".into());
    }
    if config.mailbox_flags & (RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1 | RP1_MAILBOX_FLAG_SHARED_SRAM_V2)
        == (RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1 | RP1_MAILBOX_FLAG_SHARED_SRAM_V2)
    {
        return Err("[firmware] mailbox layout v1 and v2 flags are mutually exclusive".into());
    }
    Ok(())
}

pub fn owner_bitmap(owner: &BTreeMap<String, String>) -> Result<OwnerBitmap, Box<dyn Error>> {
    let mut bitmap = OwnerBitmap {
        owner_rp1: 0,
        owner_linux: 0,
        owner_disabled: 0,
    };

    for (key, value) in owner {
        let mask = bit(owner_key_bit(key)?);
        if (bitmap.owner_rp1 | bitmap.owner_linux | bitmap.owner_disabled) & mask != 0 {
            return Err(format!("duplicate owner assignment for {key}").into());
        }
        match value.as_str() {
            "rp1" => bitmap.owner_rp1 |= mask,
            "linux" => bitmap.owner_linux |= mask,
            "disabled" => bitmap.owner_disabled |= mask,
            _ => return Err(format!("invalid owner `{value}` for {key}").into()),
        }
    }

    Ok(bitmap)
}

fn encode_note(config: &Rp1BuildConfig) -> Vec<u8> {
    let desc = encode_desc(config);
    let mut note = Vec::new();
    write_u32(&mut note, RP1_NOTE_NAME.len() as u32);
    write_u32(&mut note, desc.len() as u32);
    write_u32(&mut note, RP1_NOTE_TYPE_BOOT_V1);
    note.extend_from_slice(RP1_NOTE_NAME);
    pad4(&mut note);
    note.extend_from_slice(&desc);
    pad4(&mut note);
    note
}

fn encode_desc(config: &Rp1BuildConfig) -> Vec<u8> {
    let mut desc = vec![0u8; Rp1BootInfoV1::SIZE];
    desc[0..8].copy_from_slice(&RP1_NOTE_MAGIC);
    put_u16(&mut desc, 8, RP1_NOTE_ABI_VERSION);
    put_u16(&mut desc, 10, Rp1BootInfoV1::SIZE as u16);
    put_u32(&mut desc, 12, 0);
    put_u32(&mut desc, 16, 0);

    put_u64(&mut desc, 48, config.owner_rp1);
    put_u64(&mut desc, 56, config.owner_linux);
    put_u64(&mut desc, 64, config.owner_disabled);
    put_u32(&mut desc, 72, config.mailbox_flags);
    put_u32(&mut desc, 76, config.firmware_version_kind);
    desc
}

fn owner_key_bit(key: &str) -> Result<u8, Box<dyn Error>> {
    match key {
        "gpio" => Ok(DEV_GPIO),
        "uart0" => Ok(DEV_UART0),
        "uart1" => Ok(DEV_UART1),
        "i2c0" => Ok(DEV_I2C0),
        "i2c1" => Ok(DEV_I2C1),
        "spi0" => Ok(DEV_SPI0),
        "pio0" => Ok(DEV_PIO0),
        "pio1" => Ok(DEV_PIO1),
        "dma" => Ok(DEV_DMA),
        "timer" => Ok(DEV_TIMER),
        _ => Err(format!("unknown owner key `{key}`").into()),
    }
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn pad4(out: &mut Vec<u8>) {
    while out.len() & 3 != 0 {
        out.push(0);
    }
}

fn put_u16(out: &mut [u8], offset: usize, value: u16) {
    out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut [u8], offset: usize, value: u32) {
    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut [u8], offset: usize, value: u64) {
    out[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use rp1_abi::note::RP1_MAILBOX_FLAGS_SUPPORTED_MASK;

    #[test]
    fn minimal_owner_bitmap_matches_expected_values() {
        let owner = BTreeMap::from([
            ("gpio".to_string(), "linux".to_string()),
            ("uart0".to_string(), "linux".to_string()),
            ("uart1".to_string(), "linux".to_string()),
            ("i2c0".to_string(), "linux".to_string()),
            ("i2c1".to_string(), "linux".to_string()),
            ("spi0".to_string(), "linux".to_string()),
            ("pio0".to_string(), "rp1".to_string()),
            ("pio1".to_string(), "rp1".to_string()),
            ("dma".to_string(), "linux".to_string()),
            ("timer".to_string(), "linux".to_string()),
        ]);

        assert_eq!(
            owner_bitmap(&owner).unwrap(),
            OwnerBitmap {
                owner_rp1: 0xc0,
                owner_linux: 0x33f,
                owner_disabled: 0x0,
            }
        );
    }

    #[test]
    fn minimal_config_values_are_exposed() {
        let owner = BTreeMap::from([
            ("gpio".to_string(), "linux".to_string()),
            ("uart0".to_string(), "linux".to_string()),
            ("uart1".to_string(), "linux".to_string()),
            ("i2c0".to_string(), "linux".to_string()),
            ("i2c1".to_string(), "linux".to_string()),
            ("spi0".to_string(), "linux".to_string()),
            ("pio0".to_string(), "rp1".to_string()),
            ("pio1".to_string(), "rp1".to_string()),
            ("dma".to_string(), "linux".to_string()),
            ("timer".to_string(), "linux".to_string()),
        ]);
        let owners = owner_bitmap(&owner).unwrap();
        let config = Rp1BuildConfig {
            owner_rp1: owners.owner_rp1,
            owner_linux: owners.owner_linux,
            owner_disabled: owners.owner_disabled,
            mailbox_flags: 1,
            firmware_version_kind: RP1_VERSION_NON_PIO,
        };
        assert_eq!(config.owner_rp1, 0xc0);
        assert_eq!(config.owner_linux, 0x33f);
        assert_eq!(config.owner_disabled, 0x0);
        assert_eq!(config.mailbox_flags, 1);
        assert_eq!(config.firmware_version_kind, 0);
    }

    fn flags_from_config(extra_firmware: &str, mailbox: bool) -> Result<u32, Box<dyn Error>> {
        Ok(parse_config_text(&format!(
            "[firmware]\nname = \"test\"\n{extra_firmware}[linux]\nmailbox = {mailbox}\npio = false\n[owner]\n"
        ))?
        .mailbox_flags)
    }

    #[test]
    fn mailbox_layout_v2_is_explicit_bit2() {
        assert_eq!(
            flags_from_config("", true).unwrap(),
            RP1_MAILBOX_FLAG_ENABLE
        );
        assert_eq!(
            flags_from_config("mailbox_layout_v1 = true\n", false).unwrap(),
            RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1
        );
        assert_eq!(
            flags_from_config("shared_sram_layout_v2 = true\n", false).unwrap(),
            RP1_MAILBOX_FLAG_SHARED_SRAM_V2
        );
        assert_eq!(
            flags_from_config("shared_sram_layout_v2 = true\n", true).unwrap(),
            RP1_MAILBOX_FLAG_ENABLE | RP1_MAILBOX_FLAG_SHARED_SRAM_V2
        );
        assert_eq!(
            flags_from_config("shared_sram_layout_v2 = true\n", true).unwrap()
                & !RP1_MAILBOX_FLAGS_SUPPORTED_MASK,
            0
        );
    }

    #[test]
    fn mailbox_layout_v1_and_v2_are_rejected() {
        assert!(
            flags_from_config(
                "mailbox_layout_v1 = true\nshared_sram_layout_v2 = true\n",
                true
            )
            .is_err()
        );
    }

    #[test]
    fn repo_relative_config_path_resolves_from_package_build_dir() {
        let root = std::env::temp_dir().join(format!("rp1-build-config-{}", std::process::id()));
        let manifest_dir = root.join("examples").join("minimal");
        let config_path = manifest_dir.join("rp1-scmi-uart.toml");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(&config_path, "").unwrap();

        assert_eq!(
            resolve_config_path(
                &manifest_dir,
                Some(PathBuf::from("examples/minimal/rp1-scmi-uart.toml"))
            ),
            config_path
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn layout_feature_truth_table() {
        for (flags, v1_feature, v2_feature, ok) in [
            (0, false, false, true),
            (RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1, true, false, true),
            (RP1_MAILBOX_FLAG_SHARED_SRAM_V2, false, true, true),
            (RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1, false, false, false),
            (RP1_MAILBOX_FLAG_SHARED_SRAM_V2, false, false, false),
            (
                RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1 | RP1_MAILBOX_FLAG_SHARED_SRAM_V2,
                true,
                true,
                false,
            ),
        ] {
            let config = Rp1BuildConfig {
                owner_rp1: 0,
                owner_linux: 0,
                owner_disabled: 0,
                mailbox_flags: flags,
                firmware_version_kind: RP1_VERSION_NON_PIO,
            };
            assert_eq!(
                validate_layout_features(&config, v1_feature, v2_feature).is_ok(),
                ok
            );
        }
    }

    #[test]
    fn public_note_writer_rejects_reserved_flag_bits() {
        let path = std::env::temp_dir().join("rp1-build-rejects-reserved-note.bin");
        let config = Rp1BuildConfig {
            owner_rp1: 0,
            owner_linux: 0,
            owner_disabled: 0,
            mailbox_flags: RP1_MAILBOX_FLAGS_SUPPORTED_MASK | 0x8,
            firmware_version_kind: RP1_VERSION_NON_PIO,
        };
        assert!(write_note_bin(&config, &path).is_err());
    }

    #[test]
    fn public_note_writer_rejects_v1_plus_v2_flags() {
        let path = std::env::temp_dir().join("rp1-build-rejects-v1-plus-v2-note.bin");
        let config = Rp1BuildConfig {
            owner_rp1: 0,
            owner_linux: 0,
            owner_disabled: 0,
            mailbox_flags: RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1 | RP1_MAILBOX_FLAG_SHARED_SRAM_V2,
            firmware_version_kind: RP1_VERSION_NON_PIO,
        };
        assert!(write_note_bin(&config, &path).is_err());
    }
}
