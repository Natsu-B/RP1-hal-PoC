use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use rp1_abi::note::{
    RP1_MAILBOX_FLAG_ENABLE, RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1, RP1_NOTE_ABI_VERSION,
    RP1_NOTE_MAGIC, RP1_NOTE_NAME, RP1_NOTE_TYPE_BOOT_V1, RP1_VERSION_NON_PIO, Rp1BootInfoV1,
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
    let config_path = std::env::var_os("RP1_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("rp1.toml"));
    println!("cargo:rerun-if-env-changed=RP1_CONFIG");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1");
    println!("cargo:rerun-if-changed={}", config_path.display());
    let config = parse_config(&config_path)?;
    validate_mailbox_layout_v1_feature(
        &config,
        std::env::var_os("CARGO_FEATURE_DEBUG_MAILBOX_LAYOUT_V1").is_some(),
    )?;
    fs::create_dir_all(&out_dir)?;
    let note_path = out_dir.join("rp1_note.bin");
    write_note_bin(&config, &note_path)?;
    println!("cargo:rustc-env=RP1_NOTE_BIN={}", note_path.display());
    Ok(note_path)
}

fn validate_mailbox_layout_v1_feature(
    config: &Rp1BuildConfig,
    feature_enabled: bool,
) -> Result<(), Box<dyn Error>> {
    let config_enabled = config.mailbox_flags & RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1 != 0;
    if config_enabled != feature_enabled {
        return Err(
            "[firmware] mailbox_layout_v1 must match feature debug-mailbox-layout-v1".into(),
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

fn parse_config_text(config_text: &str) -> Result<Rp1BuildConfig, Box<dyn Error>> {
    let config: Rp1Toml = toml::from_str(config_text)?;
    let owners = owner_bitmap(&config.owner)?;
    let _ = config.firmware.name.as_str();
    let _ = config.linux.pio;
    Ok(Rp1BuildConfig {
        owner_rp1: owners.owner_rp1,
        owner_linux: owners.owner_linux,
        owner_disabled: owners.owner_disabled,
        mailbox_flags: u32::from(config.linux.mailbox) * RP1_MAILBOX_FLAG_ENABLE
            | u32::from(config.firmware.mailbox_layout_v1) * RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1,
        firmware_version_kind: RP1_VERSION_NON_PIO,
    })
}

pub fn write_note_bin(
    config: &Rp1BuildConfig,
    output: impl AsRef<Path>,
) -> Result<(), Box<dyn Error>> {
    fs::write(output, encode_note(config))?;
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

    fn note_with_mailbox_options(mailbox: bool, layout_v1: Option<bool>) -> Vec<u8> {
        let layout_v1 = layout_v1
            .map(|value| format!("mailbox_layout_v1 = {value}\n"))
            .unwrap_or_default();
        let config = parse_config_text(&format!(
            "[firmware]\nname = \"test\"\n{layout_v1}[linux]\nmailbox = {mailbox}\npio = false\n[owner]\n"
        ))
        .unwrap();
        encode_note(&config)
    }

    fn encoded_mailbox_flags(note: &[u8]) -> u32 {
        u32::from_le_bytes(note[88..92].try_into().unwrap())
    }

    #[test]
    fn mailbox_layout_v1_is_explicit_opt_in() {
        assert_eq!(
            encoded_mailbox_flags(&note_with_mailbox_options(false, None)),
            0
        );

        let absent = note_with_mailbox_options(true, None);
        let explicit_false = note_with_mailbox_options(true, Some(false));
        assert_eq!(absent, explicit_false);
        assert_eq!(encoded_mailbox_flags(&absent), RP1_MAILBOX_FLAG_ENABLE);

        let layout_only = note_with_mailbox_options(false, Some(true));
        assert_eq!(
            encoded_mailbox_flags(&layout_only),
            RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1
        );

        let enabled_layout = note_with_mailbox_options(true, Some(true));
        assert_eq!(
            encoded_mailbox_flags(&enabled_layout),
            RP1_MAILBOX_FLAGS_SUPPORTED_MASK
        );
        assert_eq!(
            encoded_mailbox_flags(&enabled_layout) & !RP1_MAILBOX_FLAGS_SUPPORTED_MASK,
            0
        );
    }

    #[test]
    fn mailbox_layout_v1_feature_config_truth_table() {
        for (config_enabled, feature_enabled, expected) in [
            (false, false, true),
            (false, true, false),
            (true, false, false),
            (true, true, true),
        ] {
            let config = Rp1BuildConfig {
                owner_rp1: 0,
                owner_linux: 0,
                owner_disabled: 0,
                mailbox_flags: u32::from(config_enabled) * RP1_MAILBOX_FLAG_PRIVATE_LAYOUT_V1,
                firmware_version_kind: RP1_VERSION_NON_PIO,
            };
            assert_eq!(
                validate_mailbox_layout_v1_feature(&config, feature_enabled).is_ok(),
                expected
            );
        }
    }
}
