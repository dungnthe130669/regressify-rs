use std::{
    io::{Seek, SeekFrom},
    path::Path,
};

use anyhow::{Context, Result, ensure};

const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

pub(super) struct Package {
    pub file: tempfile::NamedTempFile,
    pub bytes: u64,
    pub files: u64,
    pub maximum_file: u64,
}

impl Package {
    pub fn create(root: &Path, file_limit: u64) -> Result<Self> {
        ensure!(
            root.join("index.html").is_file(),
            "Deployment requires index.html"
        );
        let mut file = tempfile::NamedTempFile::new_in(
            root.parent()
                .context("Deployment staging directory has no parent")?,
        )?;
        let mut archive = zip::ZipWriter::new(file.as_file_mut());
        let mut bytes = 0;
        let mut files = 0;
        let mut maximum_file = 0;
        let mut last_update = std::time::Instant::now();
        for entry in walkdir::WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
        {
            let entry = entry?;
            ensure!(
                !entry.file_type().is_symlink(),
                "Deployment package cannot contain symlinks"
            );
            if !entry.file_type().is_file() {
                continue;
            }
            let length = entry.metadata()?.len();
            bytes += length;
            files += 1;
            maximum_file = maximum_file.max(length);
            ensure!(
                bytes <= 10 * MAX_PACKAGE_BYTES,
                "Deployment content exceeds packaging limits"
            );
            ensure!(
                files <= file_limit,
                "Deployment exceeds this provider's {file_limit}-file package limit"
            );
            let name = entry
                .path()
                .strip_prefix(root)?
                .to_str()
                .context("Deployment paths must be UTF-8")?
                .replace('\\', "/");
            let compressed = entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "png" | "jpg" | "jpeg" | "webp" | "woff" | "woff2" | "zip" | "gz" | "br"
                    )
                });
            let options =
                zip::write::SimpleFileOptions::default().compression_method(if compressed {
                    zip::CompressionMethod::Stored
                } else {
                    zip::CompressionMethod::Deflated
                });
            archive.start_file(name, options)?;
            let copied = std::io::copy(&mut std::fs::File::open(entry.path())?, &mut archive)?;
            ensure!(
                copied == length,
                "Deployment content changed during packaging"
            );
            if files == 1 || files.is_multiple_of(50) || last_update.elapsed().as_secs() >= 1 {
                super::phase(&format!(
                    "Packaged {files} report files ({:.2} MiB uncompressed)",
                    bytes as f64 / (1024.0 * 1024.0)
                ));
                last_update = std::time::Instant::now();
            }
        }
        archive.finish()?;
        ensure!(
            file.as_file().metadata()?.len() <= MAX_PACKAGE_BYTES,
            "Compressed deployment package exceeds 2 GiB"
        );
        super::phase(&format!(
            "Package ready: {files} files, {:.2} MiB content, {:.2} MiB ZIP to upload",
            bytes as f64 / (1024.0 * 1024.0),
            file.as_file().metadata()?.len() as f64 / (1024.0 * 1024.0)
        ));
        file.as_file_mut().seek(SeekFrom::Start(0))?;
        Ok(Self {
            file,
            bytes,
            files,
            maximum_file,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Package;

    #[test]
    fn packaging_enforces_provider_file_limits_and_keeps_nested_paths() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("assets")).unwrap();
        std::fs::write(root.path().join("index.html"), "report").unwrap();
        std::fs::write(root.path().join("assets/capture.png"), "png").unwrap();
        assert!(Package::create(root.path(), 1).is_err());
        let package = Package::create(root.path(), 2).unwrap();
        assert_eq!(package.files, 2);
        let mut archive = zip::ZipArchive::new(package.file.reopen().unwrap()).unwrap();
        assert_eq!(
            archive.by_name("assets/capture.png").unwrap().compression(),
            zip::CompressionMethod::Stored
        );
    }
}
