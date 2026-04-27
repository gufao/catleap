use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

pub const WINE_EXPECTED_VERSION: &str = "1.0.0";
// Placeholder until first real release is published.
pub const WINE_RELEASE_URL: &str =
    "https://github.com/REPLACE_ME/catleap/releases/download/wine-catleap-1.0.0/wine-catleap-1.0.0.tar.xz";
pub const WINE_EXPECTED_SHA256: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
pub const REQUIRED_FREE_BYTES: u64 = 500 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InstallPhase {
    CheckingSpace,
    Downloading { bytes_done: u64, bytes_total: u64 },
    Verifying,
    Extracting,
    Codesigning,
    Done,
    Failed { error: String },
}

/// Compute the hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Bytes free at the given path's filesystem.
pub fn free_bytes(path: &Path) -> Result<u64, String> {
    use std::os::unix::ffi::OsStrExt;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|e| format!("path contains NUL byte: {e}"))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if ret != 0 {
        return Err("statvfs failed".to_string());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize as u64)
}

/// Verify a downloaded file's SHA-256 matches the expected hex digest.
pub fn verify_sha256(path: &Path, expected_hex: &str) -> Result<(), String> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected_hex) {
        Ok(())
    } else {
        Err(format!("SHA mismatch: expected {expected_hex}, got {actual}"))
    }
}

/// Extract a `.tar.xz` archive into `dest`. `dest` is created if missing.
pub fn extract_tar_xz(archive: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("mkdir {}: {e}", dest.display()))?;
    let f = fs::File::open(archive).map_err(|e| format!("open {}: {e}", archive.display()))?;
    let xz = xz2::read::XzDecoder::new(f);
    let mut tar = tar::Archive::new(xz);
    tar.unpack(dest).map_err(|e| format!("unpack: {e}"))?;
    Ok(())
}

/// Clear `com.apple.quarantine` xattrs and ad-hoc codesign the wine tree.
/// Idempotent.
pub fn clear_quarantine_and_sign(wine_root: &Path) -> Result<(), String> {
    use std::process::Command;

    // xattr exits non-zero when the attribute is already absent; ignore the
    // exit code, only fail if we can't spawn the process at all.
    let _ = Command::new("/usr/bin/xattr")
        .args(["-dr", "com.apple.quarantine"])
        .arg(wine_root)
        .status()
        .map_err(|e| format!("xattr: {e}"))?;

    let status = Command::new("/usr/bin/codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(wine_root)
        .status()
        .map_err(|e| format!("codesign: {e}"))?;
    if !status.success() {
        return Err(format!("codesign exit {}", status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Replace `<data_path>/wine` with the contents of `staging` via `rename(2)`.
///
/// Both `staging` and `target` MUST reside on the same filesystem; otherwise
/// the OS returns EXDEV ("cross-device link") and this function fails. The
/// individual renames (target→.old, staging→target) are atomic; the sequence
/// as a whole is not — a crash between them leaves a `.old` directory behind
/// that a subsequent run cleans up. Caller must serialise concurrent calls.
pub fn promote_staging(staging: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        let backup = target.with_extension("old");
        let _ = fs::remove_dir_all(&backup);
        fs::rename(target, &backup).map_err(|e| format!("backup: {e}"))?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("mkdir parent: {e}"))?;
    }
    fs::rename(staging, target).map_err(|e| format!("rename: {e}"))?;
    let _ = fs::remove_dir_all(target.with_extension("old"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn sha256_of_known_input() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a");
        let mut f = fs::File::create(&p).unwrap();
        f.write_all(b"hello").unwrap();
        // sha256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
        let h = sha256_file(&p).unwrap();
        assert_eq!(h, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }

    #[test]
    fn verify_sha256_accepts_match_rejects_mismatch() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a");
        fs::write(&p, b"hello").unwrap();
        assert!(verify_sha256(&p, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824").is_ok());
        assert!(verify_sha256(&p, "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").is_err());
    }

    #[test]
    fn promote_staging_replaces_target() {
        let tmp = TempDir::new().unwrap();
        let staging = tmp.path().join("staging");
        let target = tmp.path().join("target");
        fs::create_dir_all(staging.join("bin")).unwrap();
        fs::write(staging.join("bin/marker"), b"new").unwrap();
        fs::create_dir_all(target.join("bin")).unwrap();
        fs::write(target.join("bin/old"), b"old").unwrap();

        promote_staging(&staging, &target).unwrap();

        assert!(target.join("bin/marker").exists());
        assert!(!target.join("bin/old").exists());
    }

    #[test]
    fn extract_round_trip_tar_xz() {
        // Build a tiny tar.xz, extract, confirm files appear.
        let tmp = TempDir::new().unwrap();
        let archive = tmp.path().join("a.tar.xz");
        let f = fs::File::create(&archive).unwrap();
        let xz = xz2::write::XzEncoder::new(f, 1);
        let mut tar = tar::Builder::new(xz);
        let mut header = tar::Header::new_gnu();
        let payload = b"contents";
        header.set_size(payload.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "bin/wine64", &payload[..]).unwrap();
        tar.into_inner().unwrap().finish().unwrap();

        let dest = tmp.path().join("out");
        extract_tar_xz(&archive, &dest).unwrap();
        assert_eq!(fs::read(dest.join("bin/wine64")).unwrap(), payload);
    }

    #[test]
    fn free_bytes_returns_positive_for_tmp() {
        let tmp = TempDir::new().unwrap();
        let n = free_bytes(tmp.path()).unwrap();
        assert!(n > 0);
    }
}
