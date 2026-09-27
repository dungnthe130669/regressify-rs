use std::{fs, io::Read};

use anyhow::{Context, Result, ensure};

use super::{Cancellation, Snapshot, command, discard_file_path};
use crate::desktop::document::MAX_DOCUMENT_BYTES;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRow {
    pub before: Option<usize>,
    pub after: Option<usize>,
    pub changed: bool,
}

#[derive(Clone, Debug)]
pub struct TextReview {
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub rows: Vec<TextRow>,
    pub changes: Vec<usize>,
    pub renamed_from: Option<String>,
    pub columns: usize,
}

pub fn load_text(
    snapshot: &Snapshot,
    path: &str,
    cancellation: &Cancellation,
) -> Result<TextReview> {
    let source = versions(snapshot, path, cancellation)?;
    let mut review = compare_text(
        source.before.as_deref().unwrap_or_default(),
        source.after.as_deref().unwrap_or_default(),
        cancellation,
    )?;
    review.renamed_from = snapshot
        .changes
        .iter()
        .find(|change| change.path == path)
        .and_then(|change| change.original_path.clone());
    Ok(review)
}

pub fn compare_text(before: &str, after: &str, cancellation: &Cancellation) -> Result<TextReview> {
    ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
    let before: Vec<String> = before.split_inclusive('\n').map(str::to_owned).collect();
    let after: Vec<String> = after.split_inclusive('\n').map(str::to_owned).collect();
    ensure!(
        before.len().max(after.len()) <= 20_000,
        "Too many lines for side-by-side comparison"
    );
    ensure!(
        before
            .iter()
            .chain(&after)
            .all(|line| line.len() <= 16 * 1024),
        "A line is too long for side-by-side comparison"
    );
    let previous = before.iter().map(String::as_str).collect::<Vec<_>>();
    let current = after.iter().map(String::as_str).collect::<Vec<_>>();
    let diff = similar::TextDiff::configure()
        .algorithm(similar::Algorithm::Patience)
        .timeout(std::time::Duration::from_millis(250))
        .diff_slices(&previous, &current);
    let mut rows = Vec::new();
    let mut changes = Vec::new();
    for operation in diff.ops() {
        ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
        let old = operation.old_range();
        let new = operation.new_range();
        let changed = operation.tag() != similar::DiffTag::Equal;
        if changed && rows.last().is_none_or(|row: &TextRow| !row.changed) {
            changes.push(rows.len());
        }
        for offset in 0..old.len().max(new.len()) {
            rows.push(TextRow {
                before: (offset < old.len()).then_some(old.start + offset),
                after: (offset < new.len()).then_some(new.start + offset),
                changed,
            });
        }
    }
    let columns = before
        .iter()
        .chain(&after)
        .map(|line| line.replace('\t', "    ").len())
        .max()
        .unwrap_or_default();
    Ok(TextReview {
        before,
        after,
        rows,
        changes,
        renamed_from: None,
        columns,
    })
}

pub(super) struct FileVersions {
    pub before: Option<String>,
    pub after: Option<String>,
}

fn text(bytes: Vec<u8>) -> Result<String> {
    ensure!(
        bytes.len() <= MAX_DOCUMENT_BYTES,
        "File exceeds the 4 MiB text comparison limit"
    );
    ensure!(!bytes.contains(&0), "Binary files require Source Diff");
    String::from_utf8(bytes).context("File is not UTF-8 text")
}

pub(super) fn versions(
    snapshot: &Snapshot,
    path: &str,
    cancellation: &Cancellation,
) -> Result<FileVersions> {
    let repository = snapshot
        .repository
        .as_ref()
        .context("Workspace is not a Git repository")?;
    let change = snapshot
        .changes
        .iter()
        .find(|change| change.path == path)
        .context("File is no longer changed; refresh Git status")?;
    ensure!(
        !change.conflict && !change.submodule,
        "Conflicts and submodules require Source Diff"
    );
    let after_path = discard_file_path(&repository.root, path)?;
    let before_path = change.original_path.as_deref().unwrap_or(path);
    discard_file_path(&repository.root, before_path)?;
    let before = if let Some(head) = &snapshot.head {
        ensure!(
            head.len() >= 40
                && head.len() <= 64
                && head.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Invalid Git revision"
        );
        let listing = command(
            &repository.root,
            [
                "ls-tree",
                "-l",
                "-z",
                "--full-tree",
                head,
                "--",
                before_path,
            ],
            cancellation,
        )?;
        let entry = listing
            .stdout
            .split(|byte| *byte == 0)
            .find(|entry| !entry.is_empty());
        if let Some(entry) = entry {
            let separator = entry
                .iter()
                .position(|byte| *byte == b'\t')
                .context("Invalid Git tree entry")?;
            ensure!(
                &entry[separator + 1..] == before_path.as_bytes(),
                "Git returned a different file"
            );
            let metadata = std::str::from_utf8(&entry[..separator])?
                .split_whitespace()
                .collect::<Vec<_>>();
            ensure!(
                metadata.len() == 4
                    && matches!(metadata[0], "100644" | "100755")
                    && metadata[1] == "blob",
                "Only regular files support text comparison"
            );
            let length: usize = metadata[3].parse()?;
            ensure!(
                length <= MAX_DOCUMENT_BYTES,
                "Committed file exceeds the 4 MiB text comparison limit"
            );
            Some(text(
                command(
                    &repository.root,
                    ["cat-file", "blob", metadata[2]],
                    cancellation,
                )?
                .stdout,
            )?)
        } else {
            None
        }
    } else {
        None
    };
    ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
    let after = match fs::symlink_metadata(&after_path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file() && !metadata.file_type().is_symlink(),
                "Only regular files support text comparison"
            );
            ensure!(
                metadata.len() <= MAX_DOCUMENT_BYTES as u64,
                "Saved file exceeds the 4 MiB text comparison limit"
            );
            let mut bytes = Vec::new();
            fs::File::open(&after_path)?
                .take(MAX_DOCUMENT_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            Some(text(bytes)?)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    Ok(FileVersions { before, after })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_diff_aligns_changes_without_parsing_source() {
        let before = "scenarios: [}\n# old\nunchanged\nremoved\n";
        let after = "scenarios: [}\n# new\ninserted\nunchanged\n";
        let review = compare_text(before, after, &Cancellation::default()).unwrap();
        assert_eq!(review.before.concat(), before);
        assert_eq!(review.after.concat(), after);
        assert_eq!(review.changes, vec![1, 4]);
        assert_eq!(review.rows.len(), 5);
        assert_eq!(review.rows[2].before, None);
        assert_eq!(review.rows[2].after, Some(2));
        assert_eq!(review.rows[3].before, Some(2));
        assert_eq!(review.rows[3].after, Some(3));
        assert!(!review.rows[3].changed);
        assert_eq!(review.rows[4].after, None);
    }

    #[test]
    fn text_diff_preserves_empty_files_unicode_and_line_endings() {
        for (before, after) in [
            ("", ""),
            ("", "new\n"),
            ("old\n", ""),
            ("caf\u{e9}\r\n", "caf\u{e9}\n"),
            ("line\n", "line"),
            ("# comment\n", "# changed comment\n"),
        ] {
            let review = compare_text(before, after, &Cancellation::default()).unwrap();
            assert_eq!(review.before.concat(), before);
            assert_eq!(review.after.concat(), after);
            assert_eq!(review.changes.is_empty(), before == after);
        }
        assert!(compare_text("", &"x".repeat(16 * 1024 + 1), &Cancellation::default()).is_err());
        assert!(compare_text("", &"\n".repeat(20_001), &Cancellation::default()).is_err());
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(compare_text("before", "after", &cancellation).is_err());
    }
}
