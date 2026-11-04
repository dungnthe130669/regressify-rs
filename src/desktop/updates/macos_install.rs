//! macOS opt-in self-update: verified download, safe extraction, staged bundle, atomic swap.
//!
//! The logic is plain filesystem/stream code compiled on every platform so Linux CI tests it;
//! only the real downloader, `codesign`, and relaunch are macOS-specific and are injected by
//! the caller. Nothing here runs a shell, a downloaded script, `sudo`, or `osascript`, and
//! nothing runs before the user clicks "Update now".

// Only the macOS desktop build calls these outside tests.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};

/// Hard ceiling for the archive regardless of what the manifest claims.
pub const MAX_ARCHIVE_SIZE: u64 = 256 * 1024 * 1024;
/// Hard ceiling for the one extracted binary.
const MAX_BINARY_SIZE: u64 = 512 * 1024 * 1024;
pub const APP_NAME: &str = "Regressify.app";
const BINARY_NAME: &str = "regressify-desktop";
/// Entries a desktop archive may contain; anything else is rejected, as `install.sh` does.
const ALLOWED_ENTRIES: [&str; 3] = [BINARY_NAME, "LICENSE", "LICENSING.md"];
const BACKUP_NAME: &str = "Regressify.app.previous";
const STAGE_NAME: &str = ".Regressify.app.update-stage";
const LOCK_NAME: &str = "macos-install.lock";
const WORK_DIR: &str = "macos-install";

/// Archive name for a CPU architecture, as published next to the installer.
pub fn archive_name(arch: &str) -> Option<&'static str> {
    match arch {
        "aarch64" => Some("regressify-desktop-aarch64-apple-darwin.zip"),
        "x86_64" => Some("regressify-desktop-x86_64-apple-darwin.zip"),
        _ => None,
    }
}

pub fn current_archive_name() -> Option<&'static str> {
    archive_name(std::env::consts::ARCH)
}

/// Size and digest the manifest promises for one archive.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveMeta {
    pub size: u64,
    pub sha256: String,
}

/// Reads `archive.<name>=<size>:<sha256>` from the key/value manifest text.
pub fn parse_archive_meta(manifest: &str, name: &str) -> Result<ArchiveMeta> {
    let mut fields = BTreeMap::new();
    for line in manifest
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let (key, value) = line
            .split_once('=')
            .context("Update install needs the key=value manifest format")?;
        ensure!(
            fields.insert(key.trim(), value.trim()).is_none(),
            "Duplicate update manifest field: {key}"
        );
    }
    let value = fields
        .get(format!("archive.{name}").as_str())
        .with_context(|| format!("The release manifest does not list {name}"))?;
    let (size, sha256) = value
        .split_once(':')
        .context("Malformed archive entry in the release manifest")?;
    ensure!(
        !size.is_empty() && size.bytes().all(|byte| byte.is_ascii_digit()),
        "Malformed archive size in the release manifest"
    );
    let size: u64 = size.parse().context("Malformed archive size")?;
    ensure!(
        (1..=MAX_ARCHIVE_SIZE).contains(&size),
        "Archive size is outside the allowed range"
    );
    ensure!(
        sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Malformed archive SHA-256 in the release manifest"
    );
    Ok(ArchiveMeta {
        size,
        sha256: sha256.to_ascii_lowercase(),
    })
}

/// `…/Regressify.app` when `exe` is `…/Regressify.app/Contents/MacOS/<binary>`; `None` for a
/// bare binary or `cargo run`.
pub fn bundle_from_exe(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.file_name()? == APP_NAME)
        .then(|| bundle.to_path_buf())
}

/// Whether the user can replace `bundle`: it is a real directory (not a symlink) and its parent
/// accepts new entries. Probed by creating a file, because mode bits miss ACLs and read-only
/// volumes.
pub fn bundle_replaceable(bundle: &Path) -> bool {
    let Some(parent) = bundle.parent() else {
        return false;
    };
    let is_real_dir = fs::symlink_metadata(bundle).is_ok_and(|meta| meta.file_type().is_dir());
    is_real_dir
        && tempfile::Builder::new()
            .prefix(".Regressify.probe")
            .tempfile_in(parent)
            .is_ok()
}

/// Bundle to offer an in-app update for, from the running executable.
pub fn installable_bundle() -> Option<PathBuf> {
    let bundle = bundle_from_exe(&std::env::current_exe().ok()?)?;
    bundle_replaceable(&bundle).then_some(bundle)
}

/// Exclusive per-user install lock; released when dropped or when the process exits.
#[derive(Debug)]
pub struct InstallLock {
    _file: File,
}

impl InstallLock {
    pub fn acquire(cache: &Path) -> Result<Self> {
        let dir = work_dir(cache)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(LOCK_NAME))
            .context("Cannot open the update install lock")?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => {
                bail!("Another Regressify window is already installing an update")
            }
            Err(TryLockError::Error(error)) => Err(error).context("Cannot lock the update install"),
        }
    }
}

/// Private per-user directory for the download; never a world-writable location.
fn work_dir(cache: &Path) -> Result<PathBuf> {
    let dir = cache.join(WORK_DIR);
    fs::create_dir_all(&dir).context("Cannot create the update download directory")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

/// Streams `reader` into `file`, failing unless it yields exactly `meta.size` bytes whose
/// SHA-256 equals `meta.sha256`. Never reads more than `size + 1` bytes.
pub fn copy_verified(
    reader: impl Read,
    file: &mut impl Write,
    meta: &ArchiveMeta,
    mut progress: impl FnMut(u64),
) -> Result<()> {
    ensure!(
        meta.size <= MAX_ARCHIVE_SIZE,
        "Archive size is outside the allowed range"
    );
    let mut reader = reader.take(meta.size + 1);
    let mut digest = Sha256::new();
    let mut received = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).context("Download interrupted")?;
        if read == 0 {
            break;
        }
        received += read as u64;
        ensure!(received <= meta.size, "Download is larger than announced");
        digest.update(&buffer[..read]);
        file.write_all(&buffer[..read])?;
        progress(received);
    }
    ensure!(received == meta.size, "Download is shorter than announced");
    let actual = hex(&digest.finalize());
    ensure!(
        actual == meta.sha256,
        "Download failed SHA-256 verification"
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Copies the desktop binary out of the archive into `destination`, executable.
///
/// Every entry is checked, not just the one extracted: only the three names `install.sh`
/// accepts, each exactly once, as plain top-level regular files. Names with separators, `..`,
/// absolute paths, and symlinks are rejected, and sizes are bounded on the bytes actually read.
pub fn extract_binary(archive: &Path, destination: &Path) -> Result<()> {
    let mut zip = zip::ZipArchive::new(File::open(archive)?).context("Archive is not a ZIP")?;
    ensure!(
        zip.len() <= ALLOWED_ENTRIES.len(),
        "Unexpected archive entries"
    );
    let mut seen = Vec::new();
    let mut binary_index = None;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).context("Cannot read archive entry")?;
        let name = entry
            .name()
            .context("Archive entry has an invalid name")?
            .into_owned();
        let name = name.as_str();
        ensure!(
            ALLOWED_ENTRIES.contains(&name) && !seen.contains(&name.to_owned()),
            "Unexpected archive entry"
        );
        ensure!(
            entry.is_file() && !entry.is_symlink() && !entry.is_dir(),
            "Archive entry is not a regular file"
        );
        if let Some(mode) = entry.unix_mode() {
            ensure!(
                mode & 0o170000 == 0o100000,
                "Archive entry is not a regular file"
            );
        }
        if name == BINARY_NAME {
            ensure!(
                (1..=MAX_BINARY_SIZE).contains(&entry.size()),
                "Archive binary has an unexpected size"
            );
            binary_index = Some(index);
        }
        seen.push(name.to_owned());
    }
    let index = binary_index.context("Archive does not contain the desktop binary")?;
    let entry = zip.by_index(index)?;
    let mut limited = entry.take(MAX_BINARY_SIZE + 1);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o755);
    }
    let mut output = options
        .open(destination)
        .context("Cannot create the staged binary")?;
    let written = io::copy(&mut limited, &mut output)?;
    ensure!(written <= MAX_BINARY_SIZE, "Archive binary is too large");
    output.flush()?;
    Ok(())
}

/// Recursive copy of the installed bundle so Info.plist, icon, and layout carry over. A
/// symlink or special file means this is not a bundle `install.sh` produced; refuse.
fn copy_bundle(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_bundle(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), &target)?;
        } else {
            bail!("The installed app contains a link or special file; not updating it");
        }
    }
    Ok(())
}

/// Removes a leftover of ours by exact path; files and symlinks are unlinked, never followed.
fn remove_own(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Builds the replacement bundle in a staging directory beside `bundle` (same volume, so the
/// later rename is atomic): a copy of the installed bundle with only the binary replaced.
pub fn stage_bundle(
    bundle: &Path,
    new_binary: &Path,
    sign: &dyn Fn(&Path) -> Result<()>,
    verify: &dyn Fn(&Path) -> Result<()>,
) -> Result<PathBuf> {
    let parent = bundle
        .parent()
        .context("Application has no parent folder")?;
    let stage = parent.join(STAGE_NAME);
    remove_own(&stage)?;
    let result = (|| {
        copy_bundle(bundle, &stage)?;
        let target = stage.join("Contents").join("MacOS").join(BINARY_NAME);
        ensure!(
            fs::symlink_metadata(&target).is_ok_and(|meta| meta.file_type().is_file()),
            "The installed app does not have the expected layout"
        );
        // A rename inside the stage keeps the swap of the executable atomic; copy first
        // because the download cache may be on another volume.
        let incoming = stage.join("Contents").join("MacOS").join(".incoming");
        fs::copy(new_binary, &incoming)?;
        fs::rename(&incoming, &target)?;
        sign(&stage).context("Cannot sign the updated app")?;
        verify(&stage).context("The updated app failed signature verification")?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(stage),
        Err(error) => {
            let _ = remove_own(&stage);
            Err(error)
        }
    }
}

/// Filesystem operations the swap uses, so tests can fail each step.
pub trait SwapOps {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
}

pub struct RealOps;

impl SwapOps for RealOps {
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }
}

/// Replaces `bundle` with `staged`, keeping exactly one backup (`Regressify.app.previous`).
/// Any failure restores the old app; the rollback outcome is reported in the error.
pub fn swap_bundle(
    bundle: &Path,
    staged: &Path,
    ops: &dyn SwapOps,
    verify: &dyn Fn(&Path) -> Result<()>,
) -> Result<()> {
    ensure!(
        !fs::symlink_metadata(bundle)?.file_type().is_symlink(),
        "Refusing to replace a symlinked app"
    );
    let backup = bundle
        .parent()
        .context("Application has no parent folder")?
        .join(BACKUP_NAME);
    // Older backup: this is the one we are about to supersede.
    remove_own(&backup)?;
    ops.rename(bundle, &backup)
        .context("Cannot move the current app aside")?;
    let installed = ops
        .rename(staged, bundle)
        .context("Cannot put the updated app in place")
        .and_then(|()| verify(bundle).context("The installed update failed verification"));
    if let Err(error) = installed {
        // Drop whatever landed at the app path (ours) and restore the backup.
        let _ = remove_own(bundle);
        return match ops.rename(&backup, bundle) {
            Ok(()) => Err(error.context("Update rolled back; the previous app is unchanged")),
            Err(restore) => Err(error.context(format!(
                "Update failed and the previous app could not be restored ({restore}); it is kept at {}",
                backup.display()
            ))),
        };
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Downloading { received: u64, total: u64 },
    Verifying,
    Installing,
}

/// Everything environment-specific, injected so tests need no network or `codesign`.
pub struct Deps<'a> {
    /// Opens an HTTPS URL as a byte stream.
    pub open: &'a dyn Fn(&str) -> Result<Box<dyn Read>>,
    pub sign: &'a dyn Fn(&Path) -> Result<()>,
    pub verify: &'a dyn Fn(&Path) -> Result<()>,
    pub ops: &'a dyn SwapOps,
}

/// Downloads, verifies, stages, and swaps. On any error nothing about the installed app
/// changes (or it is rolled back) and the temporary download is deleted.
pub fn install(
    bundle: &Path,
    cache: &Path,
    url: &str,
    meta: &ArchiveMeta,
    deps: &Deps,
    progress: &mut dyn FnMut(Phase),
) -> Result<()> {
    ensure!(
        url.starts_with("https://"),
        "Update downloads must use HTTPS"
    );
    let dir = work_dir(cache)?;
    let archive = tempfile::Builder::new()
        .prefix("download-")
        .tempfile_in(&dir)
        .context("Cannot create the download file")?;
    let total = meta.size;
    let reader = (deps.open)(url)?;
    let mut writer = io::BufWriter::new(archive.as_file());
    copy_verified(reader, &mut writer, meta, |received| {
        progress(Phase::Downloading { received, total })
    })?;
    writer.flush()?;
    drop(writer);
    progress(Phase::Verifying);
    let binary = tempfile::Builder::new()
        .prefix("binary-")
        .tempfile_in(&dir)?
        .into_temp_path();
    // `create_new` needs a free name: keep the unique path, drop the placeholder file.
    let binary_path = binary.to_path_buf();
    fs::remove_file(&binary_path)?;
    extract_binary(archive.path(), &binary_path)?;
    progress(Phase::Installing);
    let staged = stage_bundle(bundle, &binary_path, deps.sign, deps.verify);
    let _ = fs::remove_file(&binary_path);
    let staged = staged?;
    if let Err(error) = swap_bundle(bundle, &staged, deps.ops, deps.verify) {
        let _ = remove_own(&staged);
        return Err(error);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;

    const GOOD_HASH: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn manifest() -> String {
        format!(
            "version=2.2.2.0\ntag=v2.2.2\narchive.regressify-desktop-aarch64-apple-darwin.zip=10:{GOOD_HASH}\narchive.regressify-desktop-x86_64-apple-darwin.zip=11:{}\n",
            GOOD_HASH.to_uppercase().replace('B', "C")
        )
    }

    fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, data) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn meta_for(bytes: &[u8]) -> ArchiveMeta {
        ArchiveMeta {
            size: bytes.len() as u64,
            sha256: hex(&Sha256::digest(bytes)),
        }
    }

    fn fake_app(parent: &Path) -> PathBuf {
        let app = parent.join(APP_NAME);
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::create_dir_all(app.join("Contents/Resources")).unwrap();
        fs::write(app.join("Contents/MacOS").join(BINARY_NAME), b"old").unwrap();
        fs::write(app.join("Contents/Info.plist"), b"plist").unwrap();
        fs::write(app.join("Contents/Resources/Regressify.icns"), b"icon").unwrap();
        app
    }

    fn binary_of(app: &Path) -> Vec<u8> {
        fs::read(app.join("Contents/MacOS").join(BINARY_NAME)).unwrap()
    }

    fn ok_step(_: &Path) -> Result<()> {
        Ok(())
    }

    #[test]
    fn archive_is_selected_by_arch() {
        assert_eq!(
            archive_name("aarch64"),
            Some("regressify-desktop-aarch64-apple-darwin.zip")
        );
        assert_eq!(
            archive_name("x86_64"),
            Some("regressify-desktop-x86_64-apple-darwin.zip")
        );
        assert_eq!(archive_name("riscv64"), None);
        let live = manifest();
        let arm = parse_archive_meta(&live, archive_name("aarch64").unwrap()).unwrap();
        assert_eq!((arm.size, arm.sha256.as_str()), (10, GOOD_HASH));
        let intel = parse_archive_meta(&live, archive_name("x86_64").unwrap()).unwrap();
        assert_eq!(intel.size, 11);
        assert!(intel.sha256.bytes().all(|b| b == b'c'));
    }

    #[test]
    fn archive_meta_rejects_bad_manifests() {
        let name = "regressify-desktop-aarch64-apple-darwin.zip";
        let line = |value: &str| format!("archive.{name}={value}\n");
        for bad in [
            line(&format!("0:{GOOD_HASH}")),
            line(&format!("{}:{GOOD_HASH}", MAX_ARCHIVE_SIZE + 1)),
            line(&format!("-5:{GOOD_HASH}")),
            line(&format!("abc:{GOOD_HASH}")),
            line("10:abcd"),
            line(&format!("10:{}z", &GOOD_HASH[1..])),
            line("10"),
            format!(
                "{}{}",
                line(&format!("10:{GOOD_HASH}")),
                line(&format!("11:{GOOD_HASH}"))
            ),
            "no equals sign".into(),
            "version=1\n".into(),
        ] {
            assert!(parse_archive_meta(&bad, name).is_err(), "{bad:?}");
        }
        assert!(parse_archive_meta(&line(&format!("10:{GOOD_HASH}")), name).is_ok());
    }

    #[test]
    fn bundle_detection_requires_the_app_layout() {
        let inside = Path::new("/Applications/Regressify.app/Contents/MacOS/regressify-desktop");
        assert_eq!(
            bundle_from_exe(inside),
            Some(PathBuf::from("/Applications/Regressify.app"))
        );
        for outside in [
            "/usr/local/bin/regressify-desktop",
            "/home/u/target/debug/regressify-desktop",
            "/Applications/Other.app/Contents/MacOS/regressify-desktop",
            "/Applications/Regressify.app/Contents/Resources/regressify-desktop",
            "/Regressify.app/MacOS/regressify-desktop",
            "regressify-desktop",
        ] {
            assert_eq!(bundle_from_exe(Path::new(outside)), None, "{outside}");
        }
    }

    #[test]
    fn replaceable_needs_a_writable_real_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let app = fake_app(dir.path());
        assert!(bundle_replaceable(&app));
        assert!(!bundle_replaceable(&dir.path().join("Missing.app")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let link = dir.path().join("Link.app");
            symlink(&app, &link).unwrap();
            assert!(!bundle_replaceable(&link));
            let locked = tempfile::tempdir().unwrap();
            let locked_app = fake_app(locked.path());
            fs::set_permissions(locked.path(), fs::Permissions::from_mode(0o555)).unwrap();
            // Root ignores directory modes; only assert where the OS enforces them.
            let enforced = tempfile::Builder::new().tempfile_in(locked.path()).is_err();
            let replaceable = bundle_replaceable(&locked_app);
            fs::set_permissions(locked.path(), fs::Permissions::from_mode(0o755)).unwrap();
            if enforced {
                assert!(!replaceable);
            }
        }
    }

    #[test]
    fn lock_admits_one_installer_at_a_time() {
        let cache = tempfile::tempdir().unwrap();
        let first = InstallLock::acquire(cache.path()).unwrap();
        assert!(InstallLock::acquire(cache.path()).is_err());
        drop(first);
        assert!(InstallLock::acquire(cache.path()).is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(cache.path().join(WORK_DIR))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o700);
        }
    }

    #[test]
    fn verified_copy_checks_size_and_hash() {
        let data = b"payload bytes".to_vec();
        let meta = meta_for(&data);
        let run = |bytes: &[u8], meta: &ArchiveMeta| {
            let mut out = Vec::new();
            let mut last = 0;
            let result = copy_verified(bytes, &mut out, meta, |received| last = received);
            (result, last)
        };
        let (good, last) = run(&data, &meta);
        assert!(good.is_ok());
        assert_eq!(last, data.len() as u64);
        assert!(
            run(&data[..5], &meta)
                .0
                .unwrap_err()
                .to_string()
                .contains("shorter")
        );
        let long = [data.as_slice(), b"x"].concat();
        assert!(
            run(&long, &meta)
                .0
                .unwrap_err()
                .to_string()
                .contains("larger")
        );
        let mut tampered = data.clone();
        tampered[0] ^= 1;
        assert!(
            run(&tampered, &meta)
                .0
                .unwrap_err()
                .to_string()
                .contains("SHA-256")
        );
        let wrong = ArchiveMeta {
            sha256: GOOD_HASH.into(),
            ..meta.clone()
        };
        assert!(run(&data, &wrong).0.is_err());
        let huge = ArchiveMeta {
            size: MAX_ARCHIVE_SIZE + 1,
            ..meta
        };
        assert!(run(&data, &huge).0.is_err());
    }

    #[test]
    fn a_never_ending_stream_is_cut_off() {
        let meta = ArchiveMeta {
            size: 1000,
            sha256: GOOD_HASH.into(),
        };
        let mut out = Vec::new();
        let result = copy_verified(io::repeat(7), &mut out, &meta, |_| {});
        assert!(result.is_err());
        assert!(out.len() <= 1000);
    }

    fn extract(entries: &[(&str, &[u8])]) -> (tempfile::TempDir, Result<()>) {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        fs::write(&archive, zip_bytes(entries)).unwrap();
        let result = extract_binary(&archive, &dir.path().join("out"));
        (dir, result)
    }

    #[test]
    fn extraction_takes_only_the_binary() {
        let (dir, result) = extract(&[
            ("LICENSE", b"l"),
            (BINARY_NAME, b"new-binary"),
            ("LICENSING.md", b"m"),
        ]);
        result.unwrap();
        assert_eq!(fs::read(dir.path().join("out")).unwrap(), b"new-binary");
        let entries = fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(entries, 2, "no other entry is written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.path().join("out"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        assert!(extract(&[(BINARY_NAME, b"x")]).1.is_ok());
    }

    #[test]
    fn extraction_rejects_unsafe_or_unexpected_entries() {
        for entries in [
            vec![("../regressify-desktop", b"x".as_slice())],
            vec![("/etc/passwd", b"x")],
            vec![("sub/regressify-desktop", b"x")],
            vec![(BINARY_NAME, b"x"), ("extra.txt", b"y")],
            vec![(BINARY_NAME, b"x"), ("LICENSE/", b"")],
            vec![("LICENSE", b"only")],
            vec![
                (BINARY_NAME, b"x"),
                ("LICENSE", b"a"),
                ("LICENSING.md", b"b"),
                ("NOTICE", b"c"),
            ],
            vec![(BINARY_NAME, b"")],
        ] {
            let (dir, result) = extract(&entries);
            assert!(result.is_err(), "{entries:?}");
            assert!(!dir.path().join("out").exists(), "{entries:?}");
        }
        assert!(extract_binary(Path::new("/nonexistent.zip"), Path::new("/tmp/x")).is_err());
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.zip"), b"not a zip").unwrap();
        assert!(extract_binary(&dir.path().join("a.zip"), &dir.path().join("o")).is_err());
    }

    #[test]
    fn extraction_rejects_symlink_entries() {
        let mut writer = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        writer
            .add_symlink(
                BINARY_NAME,
                "/etc/passwd",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("a.zip");
        fs::write(&archive, writer.finish().unwrap().into_inner()).unwrap();
        assert!(extract_binary(&archive, &dir.path().join("out")).is_err());
        assert!(!dir.path().join("out").exists());
    }

    #[test]
    fn staging_replaces_only_the_binary_and_signs_it() {
        let dir = tempfile::tempdir().unwrap();
        let app = fake_app(dir.path());
        let new_binary = dir.path().join("new");
        fs::write(&new_binary, b"new").unwrap();
        let signed = RefCell::new(Vec::new());
        let sign = |path: &Path| {
            signed.borrow_mut().push(path.to_path_buf());
            // The signed tree must already hold the new binary.
            assert_eq!(binary_of(path), b"new");
            Ok(())
        };
        let staged = stage_bundle(&app, &new_binary, &sign, &ok_step).unwrap();
        assert_eq!(staged.parent(), app.parent(), "same volume as the app");
        assert_eq!(*signed.borrow(), vec![staged.clone()]);
        assert_eq!(
            fs::read(staged.join("Contents/Info.plist")).unwrap(),
            b"plist"
        );
        assert_eq!(
            fs::read(staged.join("Contents/Resources/Regressify.icns")).unwrap(),
            b"icon"
        );
        assert!(!staged.join("Contents/MacOS/.incoming").exists());
        assert_eq!(binary_of(&app), b"old", "the installed app is untouched");
    }

    #[test]
    fn staging_failures_clean_up_and_leave_the_app_alone() {
        let dir = tempfile::tempdir().unwrap();
        let app = fake_app(dir.path());
        let new_binary = dir.path().join("new");
        fs::write(&new_binary, b"new").unwrap();
        let fail = |_: &Path| -> Result<()> { bail!("boom") };
        assert!(stage_bundle(&app, &new_binary, &fail, &ok_step).is_err());
        assert!(stage_bundle(&app, &new_binary, &ok_step, &fail).is_err());
        assert!(stage_bundle(&app, &dir.path().join("absent"), &ok_step, &ok_step).is_err());
        assert!(!dir.path().join(STAGE_NAME).exists());
        assert_eq!(binary_of(&app), b"old");
        // A stale stage of ours from a crashed run is replaced.
        fs::create_dir_all(dir.path().join(STAGE_NAME).join("junk")).unwrap();
        let staged = stage_bundle(&app, &new_binary, &ok_step, &ok_step).unwrap();
        assert!(!staged.join("junk").exists());
        // A bundle with an unexpected layout or a link inside is refused.
        let odd = dir.path().join("odd");
        fs::create_dir_all(&odd).unwrap();
        let odd = odd.join(APP_NAME);
        fs::create_dir_all(&odd).unwrap();
        assert!(stage_bundle(&odd, &new_binary, &ok_step, &ok_step).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc/hosts", app.join("Contents/link")).unwrap();
            assert!(stage_bundle(&app, &new_binary, &ok_step, &ok_step).is_err());
        }
    }

    struct FailNth {
        calls: Cell<usize>,
        fail_at: usize,
    }

    impl SwapOps for FailNth {
        fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
            self.calls.set(self.calls.get() + 1);
            if self.calls.get() == self.fail_at {
                return Err(io::Error::other("injected"));
            }
            fs::rename(from, to)
        }
    }

    fn staged_pair(dir: &Path) -> (PathBuf, PathBuf) {
        let app = fake_app(dir);
        let staged = dir.join(STAGE_NAME);
        fs::create_dir_all(staged.join("Contents/MacOS")).unwrap();
        fs::write(staged.join("Contents/MacOS").join(BINARY_NAME), b"new").unwrap();
        (app, staged)
    }

    #[test]
    fn swap_keeps_exactly_one_backup() {
        let dir = tempfile::tempdir().unwrap();
        let (app, staged) = staged_pair(dir.path());
        // An older backup is superseded, not accumulated.
        fs::create_dir_all(dir.path().join(BACKUP_NAME).join("old-backup")).unwrap();
        swap_bundle(&app, &staged, &RealOps, &ok_step).unwrap();
        assert_eq!(binary_of(&app), b"new");
        assert_eq!(binary_of(&dir.path().join(BACKUP_NAME)), b"old");
        assert!(!dir.path().join(BACKUP_NAME).join("old-backup").exists());
        assert!(!staged.exists());
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 2, "{names:?}");
    }

    #[test]
    fn swap_failure_at_each_step_restores_the_old_app() {
        // Step 1 fails moving the app aside; step 2 fails moving the new app in.
        for fail_at in [1, 2] {
            let dir = tempfile::tempdir().unwrap();
            let (app, staged) = staged_pair(dir.path());
            let ops = FailNth {
                calls: Cell::new(0),
                fail_at,
            };
            let error = swap_bundle(&app, &staged, &ops, &ok_step).unwrap_err();
            assert!(format!("{error:#}").contains("injected"), "{error:#}");
            assert_eq!(binary_of(&app), b"old", "fail_at {fail_at}");
            assert!(staged.exists(), "the caller removes its own stage");
        }
    }

    #[test]
    fn swap_rolls_back_when_the_installed_update_fails_verification() {
        let dir = tempfile::tempdir().unwrap();
        let (app, staged) = staged_pair(dir.path());
        let reject = |_: &Path| -> Result<()> { bail!("bad signature") };
        let error = swap_bundle(&app, &staged, &RealOps, &reject).unwrap_err();
        assert!(format!("{error:#}").contains("rolled back"), "{error:#}");
        assert_eq!(binary_of(&app), b"old");
    }

    #[test]
    fn swap_reports_the_backup_when_the_rollback_itself_fails() {
        let dir = tempfile::tempdir().unwrap();
        let (app, staged) = staged_pair(dir.path());
        // Calls: 1 aside (ok), 2 new in (fails), 3 restore (fails).
        struct FailFrom(Cell<usize>);
        impl SwapOps for FailFrom {
            fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
                self.0.set(self.0.get() + 1);
                if self.0.get() >= 2 {
                    return Err(io::Error::other("disk gone"));
                }
                fs::rename(from, to)
            }
        }
        let error = swap_bundle(&app, &staged, &FailFrom(Cell::new(0)), &ok_step).unwrap_err();
        let text = format!("{error:#}");
        assert!(
            text.contains("could not be restored") && text.contains(BACKUP_NAME),
            "{text}"
        );
        assert_eq!(binary_of(&dir.path().join(BACKUP_NAME)), b"old");
    }

    #[cfg(unix)]
    #[test]
    fn swap_refuses_a_symlinked_app_and_never_follows_a_backup_link() {
        let dir = tempfile::tempdir().unwrap();
        let real = tempfile::tempdir().unwrap();
        let target = fake_app(real.path());
        let link = dir.path().join(APP_NAME);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let staged = dir.path().join(STAGE_NAME);
        fs::create_dir_all(&staged).unwrap();
        assert!(swap_bundle(&link, &staged, &RealOps, &ok_step).is_err());
        assert_eq!(binary_of(&target), b"old");

        // A symlink planted as the backup name is unlinked, not followed.
        let dir = tempfile::tempdir().unwrap();
        let (app, staged) = staged_pair(dir.path());
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("keep"), b"keep").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(BACKUP_NAME)).unwrap();
        swap_bundle(&app, &staged, &RealOps, &ok_step).unwrap();
        assert!(outside.path().join("keep").exists());
    }

    fn deps_for<'a>(
        open: &'a dyn Fn(&str) -> Result<Box<dyn Read>>,
        ops: &'a dyn SwapOps,
    ) -> Deps<'a> {
        Deps {
            open,
            sign: &ok_step,
            verify: &ok_step,
            ops,
        }
    }

    #[test]
    fn install_runs_every_phase_and_cleans_the_download() {
        let dir = tempfile::tempdir().unwrap();
        let app = fake_app(dir.path());
        let cache = tempfile::tempdir().unwrap();
        let bytes = zip_bytes(&[(BINARY_NAME, b"new-binary"), ("LICENSE", b"l")]);
        let meta = meta_for(&bytes);
        let urls = RefCell::new(Vec::new());
        let open = |url: &str| -> Result<Box<dyn Read>> {
            urls.borrow_mut().push(url.to_owned());
            Ok(Box::new(io::Cursor::new(bytes.clone())))
        };
        let mut phases = Vec::new();
        install(
            &app,
            cache.path(),
            "https://example.test/a.zip",
            &meta,
            &deps_for(&open, &RealOps),
            &mut |phase| phases.push(phase),
        )
        .unwrap();
        assert_eq!(*urls.borrow(), ["https://example.test/a.zip"]);
        assert_eq!(binary_of(&app), b"new-binary");
        assert_eq!(binary_of(&dir.path().join(BACKUP_NAME)), b"old");
        assert!(matches!(phases.first(), Some(Phase::Downloading { .. })));
        assert_eq!(
            &phases[phases.len() - 2..],
            [Phase::Verifying, Phase::Installing]
        );
        let leftovers: Vec<_> = fs::read_dir(cache.path().join(WORK_DIR))
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert!(
            leftovers.iter().all(|name| name == LOCK_NAME),
            "{leftovers:?}"
        );
    }

    #[test]
    fn install_aborts_without_changes_on_any_bad_input() {
        let bytes = zip_bytes(&[(BINARY_NAME, b"new-binary")]);
        let good = meta_for(&bytes);
        let https = "https://example.test/a.zip";
        let run = |url: &str, meta: &ArchiveMeta, served: Vec<u8>| {
            let dir = tempfile::tempdir().unwrap();
            let app = fake_app(dir.path());
            let cache = tempfile::tempdir().unwrap();
            let open = |_: &str| -> Result<Box<dyn Read>> {
                Ok(Box::new(io::Cursor::new(served.clone())))
            };
            let result = install(
                &app,
                cache.path(),
                url,
                meta,
                &deps_for(&open, &RealOps),
                &mut |_| {},
            );
            if result.is_err() {
                assert_eq!(binary_of(&app), b"old");
                assert!(!dir.path().join(BACKUP_NAME).exists());
                assert!(!dir.path().join(STAGE_NAME).exists());
                let leftovers = fs::read_dir(cache.path().join(WORK_DIR)).map_or(0, |dir| {
                    dir.filter(|entry| entry.as_ref().unwrap().file_name() != LOCK_NAME)
                        .count()
                });
                assert_eq!(leftovers, 0, "temporary download deleted");
            }
            result
        };
        assert!(run(https, &good, bytes.clone()).is_ok());
        assert!(run("http://example.test/a.zip", &good, bytes.clone()).is_err());
        let bad_hash = ArchiveMeta {
            sha256: GOOD_HASH.into(),
            ..good.clone()
        };
        assert!(run(https, &bad_hash, bytes.clone()).is_err());
        assert!(run(https, &good, bytes[..bytes.len() - 1].to_vec()).is_err());
        assert!(run(https, &good, [bytes.clone(), vec![0]].concat()).is_err());
        // Correct hash, but the archive holds an unexpected entry.
        let evil = zip_bytes(&[(BINARY_NAME, b"x"), ("../evil", b"y")]);
        assert!(run(https, &meta_for(&evil), evil).is_err());
        // Correct hash, signing fails: nothing is swapped.
        let dir = tempfile::tempdir().unwrap();
        let app = fake_app(dir.path());
        let cache = tempfile::tempdir().unwrap();
        let open =
            |_: &str| -> Result<Box<dyn Read>> { Ok(Box::new(io::Cursor::new(bytes.clone()))) };
        let fail = |_: &Path| -> Result<()> { bail!("no codesign") };
        let deps = Deps {
            open: &open,
            sign: &fail,
            verify: &ok_step,
            ops: &RealOps,
        };
        assert!(install(&app, cache.path(), https, &good, &deps, &mut |_| {}).is_err());
        assert_eq!(binary_of(&app), b"old");
        assert!(!dir.path().join(STAGE_NAME).exists());
        // Open failure (offline) changes nothing either.
        let offline = |_: &str| -> Result<Box<dyn Read>> { bail!("offline") };
        let deps = deps_for(&offline, &RealOps);
        assert!(install(&app, cache.path(), https, &good, &deps, &mut |_| {}).is_err());
    }
}
