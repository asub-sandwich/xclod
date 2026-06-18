use anyhow::{Result, bail};
use std::process::Command;

use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_os = "linux")]
const ASSIMP_BIN: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/linux/assimp"));

#[cfg(target_os = "macos")]
const ASSIMP_BIN: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/macos/assimp"));

#[cfg(target_os = "windows")]
const ASSIMP_BIN: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/windows/assimp.exe"
));

/// extracts bundled assimp library into the cache and returns its path
pub fn resolve_assimp(cache_dir: &Path) -> Result<PathBuf> {
    vendored_binary(cache_dir, "assimp", ASSIMP_BIN)
}

/// on macOS/windows, exiftool is not bundled because holy moly!
/// verify its installed and on PATH, returning a clear install
/// error with install instructions if its missing
pub fn resolve_exiftool(_cache_dir: &Path) -> Result<PathBuf> {
    match Command::new("exiftool").arg("-ver").output() {
        Ok(out) if out.status.success() => Ok(PathBuf::from("exiftool")),
        _ => bail!(
            "exiftool is required for `extract` but was not found in your PATH!\n\
            ... please install it :P\n  {}",
            EXIFTOOL_INSTALL_HINT
        ),
    }
}

#[cfg(target_os = "linux")]
const EXIFTOOL_INSTALL_HINT: &str = "ubuntu/debian: apt install libimage-exiftool-perl\n\
    fedora: dnf install perl-Image-ExifTool\n\
    arch: pacman -S exiftool\n\
    alpine: apk add exiftool";

#[cfg(target_os = "macos")]
const EXIFTOOL_INSTALL_HINT: &str =
    "brew install exiftool\t(or install the `.pkg` from https://exiftool.org)";
#[cfg(target_os = "windows")]
const EXIFTOOL_INSTALL_HINT: &str = "1. download the Windows build from https://exiftool.org.\n\
    2. rename `exiftool(-k).exe` to `exiftool.exe` AND keep the `exiftool_files` folder beside it.\n\
    3. add both the executable file and the `exiftool_files` folder to somewhere in your PATH";

/// writes or loads vendored binary into cache
fn vendored_binary(cache_dir: &Path, name: &str, bytes: &[u8]) -> Result<PathBuf> {
    create_dir_all(cache_dir)?;
    let path = cache_dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let needs_write = match std::fs::metadata(&path) {
        Ok(meta) => meta.len() != bytes.len().try_into()?,
        Err(_) => true,
    };
    if needs_write {
        std::fs::write(&path, bytes)?;
    }
    #[cfg(unix)]
    {
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(path)
}
