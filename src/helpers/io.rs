#![allow(dead_code)]

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};

/// Replace a file only after a complete temporary copy has been written in the
/// same directory, preventing truncated generated artifacts after interruption.
pub fn write_file_atomically(path: &Path, bytes: &[u8]) -> Result<()> {
    write_with_temporary_file(path, |temporary_file| temporary_file.write_all(bytes))
}

/// Stream a source file into a same-directory temporary file, then replace the
/// destination. This avoids partial or memory-heavy baseline promotion.
pub fn copy_file_atomically(source: &Path, destination: &Path) -> Result<()> {
    let mut source_file = fs::File::open(source)
        .with_context(|| format!("Failed to open source file '{}'", source.display()))?;
    let source_permissions = source_file.metadata()?.permissions();
    write_with_temporary_file(destination, |temporary_file| {
        std::io::copy(&mut source_file, temporary_file).map(|_| ())
    })?;
    fs::set_permissions(destination, source_permissions).with_context(|| {
        format!(
            "Failed to preserve permissions on copied file '{}'",
            destination.display()
        )
    })?;
    Ok(())
}

/// Whether a single filename component is reserved by Windows, including a
/// reserved device name followed by an extension.
pub fn is_windows_reserved_path_component(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or_default();
    let upper = stem.to_ascii_uppercase();
    matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || upper.strip_prefix("COM").is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || upper.strip_prefix("LPT").is_some_and(|number| {
            matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

fn write_with_temporary_file(
    path: &Path,
    write: impl FnOnce(&mut fs::File) -> std::io::Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("Path '{}' has no parent directory", path.display()))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("Path '{}' has no valid file name", path.display()))?;
    let temporary_path = parent.join(format!(".{file_name}.{}.tmp", uuid::Uuid::new_v4()));
    let existing_permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());

    let write_result = (|| -> Result<()> {
        let mut temporary_file = fs::File::create(&temporary_path).with_context(|| {
            format!(
                "Failed to create temporary file '{}'",
                temporary_path.display()
            )
        })?;
        write(&mut temporary_file).with_context(|| {
            format!(
                "Failed to write temporary file '{}'",
                temporary_path.display()
            )
        })?;
        temporary_file.sync_all()?;
        drop(temporary_file);
        if let Some(permissions) = existing_permissions {
            fs::set_permissions(&temporary_path, permissions)?;
        }
        fs::rename(&temporary_path, path)
            .with_context(|| format!("Failed to replace file '{}'", path.display()))?;
        Ok(())
    })();

    if write_result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    write_result
}

pub trait IOExtensions {
    fn normalize(&self) -> Result<PathBuf>;
    fn normalize_string(&self, is_folder: bool) -> Result<String>;
}

fn normalize_path_string(path: &Path) -> Result<String> {
    let mut path_str = std::fs::canonicalize(path)
        .unwrap_or_else(|_| std::path::PathBuf::from(path))
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Failed to convert path to string: {}", path.display()))?
        .replace('\\', "/");

    if path_str.starts_with("//?/") {
        // Remove the Windows extended-length path prefix
        path_str.drain(..4);
    }

    Ok(path_str)
}

impl IOExtensions for PathBuf {
    fn normalize(&self) -> Result<PathBuf> {
        Ok(PathBuf::from(normalize_path_string(self)?))
    }

    fn normalize_string(&self, is_folder: bool) -> Result<String> {
        let mut path = normalize_path_string(self)?;

        if is_folder && !path.ends_with('/') {
            path.push('/');
        }

        Ok(path)
    }
}

impl IOExtensions for Path {
    fn normalize(&self) -> Result<PathBuf> {
        Ok(PathBuf::from(normalize_path_string(self)?))
    }

    fn normalize_string(&self, is_folder: bool) -> Result<String> {
        let mut path = normalize_path_string(self)?;

        if is_folder && !path.ends_with('/') {
            path.push('/');
        }

        Ok(path)
    }
}

pub fn format_size(size: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    let size_f64 = size as f64;

    if size_f64 >= TB {
        format!("{:.2} TB", size_f64 / TB)
    } else if size_f64 >= GB {
        format!("{:.2} GB", size_f64 / GB)
    } else if size_f64 >= MB {
        format!("{:.2} MB", size_f64 / MB)
    } else if size_f64 >= KB {
        format!("{:.2} KB", size_f64 / KB)
    } else {
        format!("{} B", size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_replaces_existing_content() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("artifact.json");
        fs::write(&path, b"old").unwrap();

        write_file_atomically(&path, b"new").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new");
        assert_eq!(
            fs::read_dir(temp_dir.path()).unwrap().count(),
            1,
            "temporary files must be cleaned up"
        );
    }

    #[test]
    fn atomic_copy_replaces_existing_content_without_temporary_files() {
        let temp_dir = TempDir::new().unwrap();
        let source = temp_dir.path().join("source.png");
        let destination = temp_dir.path().join("reference.png");
        fs::write(&source, b"new baseline").unwrap();
        fs::write(&destination, b"old baseline").unwrap();

        copy_file_atomically(&source, &destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(), b"new baseline");
        assert_eq!(
            fs::read_dir(temp_dir.path()).unwrap().count(),
            2,
            "temporary files must be cleaned up"
        );
    }

    #[test]
    fn windows_reserved_path_components_include_device_names_and_extensions() {
        for name in ["CON", "prn", "AUX.json", "nul.txt", "COM1", "lpt9.log"] {
            assert!(is_windows_reserved_path_component(name), "{name}");
        }
        for name in ["CONSOLE", "COM0", "COM10", "LPT0", "LPT10", "report.json"] {
            assert!(!is_windows_reserved_path_component(name), "{name}");
        }
    }

    // ==================== format_size tests ====================

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(2048), "2.00 KB");
        assert_eq!(format_size(1024 * 1023), "1023.00 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 * 2), "2.00 MB");
        assert_eq!(format_size(1024 * 1024 * 512), "512.00 MB");
        assert_eq!(format_size(1024 * 1024 * 1023), "1023.00 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_size(1024 * 1024 * 1024 * 2), "2.00 GB");
        assert_eq!(format_size(1024 * 1024 * 1024 * 512), "512.00 GB");
    }

    #[test]
    fn test_format_size_terabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024 * 1024), "1.00 TB");
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 1024 * 2), "2.00 TB");
        assert_eq!(format_size(1024u64 * 1024 * 1024 * 1024 * 10), "10.00 TB");
    }

    #[test]
    fn test_format_size_decimal_precision() {
        // Test various fractional sizes with 2 decimal precision
        assert_eq!(format_size(1536), "1.50 KB"); // 1.5 KB
        assert_eq!(format_size(1638), "1.60 KB"); // 1.6 KB
        assert_eq!(format_size(1075), "1.05 KB"); // 1.05 KB
    }

    #[test]
    fn test_format_size_boundary_values() {
        // Test boundaries between units
        let kb = 1024u64;
        let mb = kb * 1024;
        let gb = mb * 1024;
        let tb = gb * 1024;

        assert!(format_size(kb - 1).contains("B"));
        assert!(format_size(kb).contains("KB"));
        assert!(format_size(mb - 1).contains("KB"));
        assert!(format_size(mb).contains("MB"));
        assert!(format_size(gb - 1).contains("MB"));
        assert!(format_size(gb).contains("GB"));
        assert!(format_size(tb - 1).contains("GB"));
        assert!(format_size(tb).contains("TB"));
    }

    // ==================== IOExtensions::normalize tests ====================

    #[test]
    fn test_normalize_relative_path() {
        let path = PathBuf::from(".");
        let result = path.normalize();
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(
            normalized.to_str().unwrap().ends_with("/") || !normalized.to_str().unwrap().is_empty()
        );
    }

    #[test]
    fn test_normalize_absolute_path() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();
        let result = path.normalize();
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.to_str().unwrap().contains("\\"));
    }

    #[test]
    fn test_normalize_removes_backslashes() {
        // This test simulates Windows-style paths
        let path = PathBuf::from(".");
        let result = path.normalize();
        assert!(result.is_ok());
        let normalized = result.unwrap();
        let normalized_str = normalized.to_str().unwrap();
        // Verify no backslashes in result
        assert!(!normalized_str.contains("\\"));
    }

    #[test]
    fn test_normalize_handles_nonexistent_path() {
        let path = PathBuf::from("/nonexistent/path/that/does/not/exist");
        let result = path.normalize();
        assert!(result.is_ok());
        // Should still return a valid path even if it doesn't exist
        let normalized = result.unwrap();
        assert!(!normalized.to_str().unwrap().is_empty());
    }

    #[test]
    fn test_normalize_current_directory() {
        let path = PathBuf::from(".");
        let result = path.normalize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_normalize_root_directory() {
        let path = PathBuf::from("/");
        let result = path.normalize();
        assert!(result.is_ok());
        let normalized = result.unwrap();
        // Should be a valid path
        assert!(!normalized.to_str().unwrap().is_empty());
    }

    #[test]
    fn test_normalize_nested_path() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();

        let result = nested.normalize();
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.to_str().unwrap().contains("\\"));
    }

    // ==================== IOExtensions::normalize_string tests ====================

    #[test]
    fn test_normalize_string_as_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        fs::File::create(&file_path).unwrap();

        let result = file_path.normalize_string(false);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.contains("\\"));
        assert!(!normalized.ends_with("/"));
    }

    #[test]
    fn test_normalize_string_as_folder_without_trailing_slash() {
        let temp_dir = TempDir::new().unwrap();
        let folder_path = temp_dir.path().to_path_buf();

        let result = folder_path.normalize_string(true);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.contains("\\"));
        assert!(normalized.ends_with("/"));
    }

    #[test]
    fn test_normalize_string_as_folder_with_trailing_slash() {
        let temp_dir = TempDir::new().unwrap();
        let folder_path = temp_dir.path().to_path_buf();
        // Simulate a path that already has trailing slash (conceptually)

        let result = folder_path.normalize_string(true);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.contains("\\"));
        assert!(normalized.ends_with("/"));
    }

    #[test]
    fn test_normalize_string_file_vs_folder_difference() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let as_file = path.normalize_string(false).unwrap();
        let as_folder = path.normalize_string(true).unwrap();

        // File version should not end with /
        assert!(!as_file.ends_with("/"));
        // Folder version should end with /
        assert!(as_folder.ends_with("/"));
        // Both should have the same base path
        assert_eq!(as_file, as_folder.trim_end_matches("/"));
    }

    #[test]
    fn test_normalize_string_nonexistent_path() {
        let path = PathBuf::from("/nonexistent/test/path");
        let result_file = path.normalize_string(false);
        let result_folder = path.normalize_string(true);

        assert!(result_file.is_ok());
        assert!(result_folder.is_ok());
    }

    #[test]
    fn test_normalize_string_removes_backslashes() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let result = path.normalize_string(false);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.contains("\\"));
    }

    #[test]
    fn test_normalize_string_nested_path_as_file() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("level1").join("level2").join("level3");
        fs::create_dir_all(&nested).unwrap();

        let result = nested.normalize_string(false);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.ends_with("/"));
    }

    #[test]
    fn test_normalize_string_nested_path_as_folder() {
        let temp_dir = TempDir::new().unwrap();
        let nested = temp_dir.path().join("level1").join("level2").join("level3");
        fs::create_dir_all(&nested).unwrap();

        let result = nested.normalize_string(true);
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(normalized.ends_with("/"));
    }

    #[test]
    fn test_normalize_string_root_path() {
        let path = PathBuf::from("/");
        let result = path.normalize_string(true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_normalize_string_current_dir() {
        let path = PathBuf::from(".");
        let result_file = path.normalize_string(false);
        let result_folder = path.normalize_string(true);

        assert!(result_file.is_ok());
        assert!(result_folder.is_ok());
    }

    // ==================== Integration tests ====================

    #[test]
    fn test_normalize_and_normalize_string_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let normalized_pb = path.normalize().unwrap();
        let normalized_str = path.normalize_string(false).unwrap();

        // The string version should match the PathBuf version
        assert_eq!(normalized_pb.to_str().unwrap(), normalized_str);
    }

    #[test]
    fn test_multiple_normalize_calls_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_path_buf();

        let first = path.normalize().unwrap();
        let second = first.normalize().unwrap();
        let third = second.normalize().unwrap();

        assert_eq!(first, second);
        assert_eq!(second, third);
    }

    #[test]
    fn test_format_size_with_large_values() {
        let max_u64 = u64::MAX;
        let result = format_size(max_u64);
        assert!(result.contains("TB"));
    }

    #[test]
    fn test_special_characters_in_path() {
        let temp_dir = TempDir::new().unwrap();
        let path_with_spaces = temp_dir.path().join("folder with spaces");
        fs::create_dir_all(&path_with_spaces).unwrap();

        let result = path_with_spaces.normalize();
        assert!(result.is_ok());
        let normalized = result.unwrap();
        assert!(!normalized.to_str().unwrap().contains("\\"));
    }
}
