#![allow(dead_code)]
use std::path::{Path, PathBuf};

use anyhow::Result;

use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;

pub fn cmod<P>(bin_dir: P, mode: u32) -> Result<()>
where
    P: AsRef<Path>,
{
    set_unix_mode(&bin_dir, mode)?;

    // Get all child files in the bin directory

    // Set the permission for the files in the bin directory
    let entries = fs::read_dir(&bin_dir)?;
    for entry in entries {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if file_type.is_dir() || !file_type.is_file() {
            continue;
        }

        let path = entry.path();

        set_unix_mode(&path, mode)?;
    }

    Ok(())
}

pub fn set_unix_mode<P>(file_path: P, mode: u32) -> Result<()>
where
    P: AsRef<Path>,
{
    #[cfg(feature = "local")]
    eprintln!("CMOD '{:#o}' for '{}'", mode, file_path.as_ref().display());

    let permissions = Permissions::from_mode(mode);
    fs::set_permissions(file_path, permissions)
        .map_err(|err| anyhow::anyhow!("Failed to set permissions: {:?}", err))
}
