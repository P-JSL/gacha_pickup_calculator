//! Offline asset verification and bounded, optionally authenticated CSV imports.
//!
//! The expected SHA-256 hashes are embedded from the checked-in Image manifest.
//! A modified local PNG is rejected even if its filename, PNG header and size look valid.
//! This limits same-account artwork substitution but cannot defend against replacing
//! the executable itself. Security updates still require independent dependency audits.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

pub const MAX_IMAGE_BYTES: u64 = 512 * 1024;
pub const MAX_CSV_BYTES: u64 = 8 * 1024 * 1024;
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const IMAGE_MANIFEST: &str = include_str!("../../Image/SHA256SUMS.txt");

fn parse_sha256(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.is_ascii() { return None; }
    let mut digest = [0_u8; 32];
    for (byte, pair) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        digest[byte] = ((high << 4) | low) as u8;
    }
    Some(digest)
}

fn image_digests() -> &'static HashMap<&'static str, [u8; 32]> {
    static DATA: OnceLock<HashMap<&'static str, [u8; 32]>> = OnceLock::new();
    DATA.get_or_init(|| {
        IMAGE_MANIFEST.lines().filter_map(|line| {
            let (hash, path) = line.split_once("  ")?;
            let expected = parse_sha256(hash)?;
            approved_image_key(path).then_some((path, expected))
        }).collect()
    })
}

fn approved_image_key(key: &str) -> bool {
    let mut parts = key.split('/');
    let game = parts.next();
    let filename = parts.next();
    if parts.next().is_some() { return false; }
    if !matches!(game, Some("HSR" | "WutheringWaves" | "Neverness" | "StellaSora")) {
        return false;
    }
    let Some(filename) = filename else { return false; };
    if !filename.ends_with(".png") || !(filename.starts_with("c-") || filename.starts_with("w-")) {
        return false;
    }
    let base = &filename[2..filename.len() - 4];
    !base.is_empty() && base.bytes().all(|b|
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Used by the updater to avoid re-downloading unchanged embedded assets.
/// This map is immutable and independent of runtime (potentially poisoned) locks.
pub(crate) fn baseline_digest(relative: &str) -> Option<[u8; 32]> {
    image_digests().get(relative).copied()
}

/// An image is trusted only if its bytes match the compile-time manifest.
pub fn verify_image_bytes(relative: &str, bytes: &[u8]) -> Result<(), &'static str> {
    if bytes.len() as u64 > MAX_IMAGE_BYTES || !bytes.starts_with(PNG_MAGIC) {
        return Err("Image is not an approved bounded PNG.");
    }
    // A successfully published update replaces the manifest atomically with
    // its catalog. An RwLock poisoned during publication is a hard error.
    let expected = crate::catalog::runtime_digest(relative)?
        .or_else(|| baseline_digest(relative))
        .ok_or("Image is not in a verified asset manifest.")?;
    let actual = Sha256::digest(bytes);
    if actual.as_slice() != expected {
        return Err("Local artwork differs from the signed-release source snapshot.");
    }
    Ok(())
}

/// Open once, validate the actual file handle, read a bounded amount of data,
/// and optionally require a caller-provided trusted SHA-256 fingerprint.
/// A digest proves identity to that fingerprint, not the truth of CSV claims.
pub fn read_csv_secure(path: &Path, expected_sha256: Option<&str>) -> Result<String, String> {
    let before = fs::symlink_metadata(path)
        .map_err(|e| format!("파일 확인 실패: {e}"))?;
    if !before.file_type().is_file() {
        return Err("일반 파일만 허용하며 심볼릭 링크·장치를 허용하지 않습니다.".to_owned());
    }
    let handle = File::open(path).map_err(|e| format!("CSV 열기 실패: {e}"))?;
    let meta = handle.metadata().map_err(|e| format!("CSV 메타데이터 실패: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_CSV_BYTES {
        return Err("CSV는 8 MiB 이하의 일반 파일이어야 합니다.".to_owned());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if before.dev() != meta.dev() || before.ino() != meta.ino() {
            return Err("파일 검사 중 변경이 발생했습니다.".to_owned());
        }
    }
    let mut bytes = Vec::with_capacity(meta.len().min(MAX_CSV_BYTES) as usize);
    handle.take(MAX_CSV_BYTES + 1).read_to_end(&mut bytes)
        .map_err(|e| format!("CSV 읽기 실패: {e}"))?;
    if bytes.len() as u64 > MAX_CSV_BYTES {
        return Err("읽는 중 CSV 크기가 8 MiB를 초과했습니다.".to_owned());
    }
    if let Some(expected) = expected_sha256.filter(|s| !s.trim().is_empty()) {
        let expected = parse_sha256(expected.trim()).ok_or(
            "입력된 SHA-256은 64자리 16진수여야 합니다.".to_owned())?;
        if Sha256::digest(&bytes).as_slice() != expected {
            return Err("CSV SHA-256이 입력한 신뢰 해시와 일치하지 않습니다.".to_owned());
        }
    }
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 CSV 읽기 실패: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_manifest_has_all_679_images_without_duplicate_keys() {
        assert_eq!(image_digests().len(), 679);
        for line in IMAGE_MANIFEST.lines() {
            let key = line.split_once("  ").expect("sha256 + two spaces").1;
            assert!(approved_image_key(key), "invalid image key: {key}");
        }
        assert!(!approved_image_key("HSR/../c-1001.png"));
        assert!(!approved_image_key("HSR/c-1001.png/extra"));
        assert!(!approved_image_key("Other/c-1001.png"));
    }

    #[test]
    fn image_hash_tampering_and_path_substitution_are_rejected() {
        let sample = include_bytes!("../../Image/HSR/c-1003.png");
        verify_image_bytes("HSR/c-1003.png", sample).unwrap();
        assert!(verify_image_bytes("HSR/c-1004.png", sample).is_err());
        let mut modified = sample.to_vec();
        modified.push(0);
        assert!(verify_image_bytes("HSR/c-1003.png", &modified).is_err());
    }

    #[test]
    fn csv_file_hash_and_size_checks_work_with_real_file_handle() {
        let path = std::env::temp_dir().join(format!(
            "gacha_csv_security_{}_{}.csv",
            std::process::id(), "sha256test"
        ));
        let bytes = b"pity,count\n1,2\n";
        std::fs::write(&path, bytes).unwrap();
        let expected = format!("{:x}", Sha256::digest(bytes));
        assert_eq!(read_csv_secure(&path, Some(&expected)).unwrap(), String::from_utf8_lossy(bytes));
        assert!(read_csv_secure(&path, Some(&"f".repeat(64))).is_err());
        assert!(read_csv_secure(&path, Some("invalid")).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
