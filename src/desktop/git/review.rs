use std::{collections::BTreeSet, fs, io::Read, path::Path};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{Cancellation, Snapshot, command, discard_file_path};
use crate::desktop::{
    document::{Format, MAX_DOCUMENT_BYTES},
    schema,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Removed,
    Changed,
    Moved,
}

impl ChangeKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Removed => "Removed",
            Self::Changed => "Changed",
            Self::Moved => "Moved",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FieldChange {
    pub label: String,
    pub before: String,
    pub after: String,
    pub kind: ChangeKind,
}

#[derive(Clone, Debug)]
pub struct ChangeGroup {
    pub title: String,
    pub kind: ChangeKind,
    pub fields: Vec<FieldChange>,
}

#[derive(Clone, Debug)]
pub struct StructuredReview {
    pub groups: Vec<ChangeGroup>,
    pub unchanged: usize,
    pub source_only: bool,
    pub renamed_from: Option<String>,
}

pub fn load(
    snapshot: &Snapshot,
    path: &str,
    cancellation: &Cancellation,
) -> Result<StructuredReview> {
    ensure!(
        Path::new(path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| ["yaml", "yml", "json"]
                .iter()
                .any(|known| extension.eq_ignore_ascii_case(known))),
        "This file type has no structured comparison"
    );
    let source = versions(snapshot, path, cancellation)?;
    let original = snapshot
        .changes
        .iter()
        .find(|change| change.path == path)
        .and_then(|change| change.original_path.as_deref());
    let before = source
        .before
        .as_deref()
        .map(|source| Format::for_path(Path::new(original.unwrap_or(path))).parse(source))
        .transpose()
        .context("The last committed version cannot be parsed")?;
    let after = source
        .after
        .as_deref()
        .map(|source| Format::for_path(Path::new(path)).parse(source))
        .transpose()
        .context("The current saved version cannot be parsed")?;
    let mut review = compare(before.as_ref(), after.as_ref(), cancellation)?;
    if Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.to_ascii_lowercase().contains("viewports"))
        && (before.as_ref().is_some_and(Value::is_array)
            || after.as_ref().is_some_and(Value::is_array))
    {
        review.groups.clear();
        review.unchanged = 0;
        compare_items(
            "viewports",
            before.as_ref(),
            after.as_ref(),
            "",
            &mut review,
            cancellation,
        )?;
    }
    ensure!(
        review
            .groups
            .iter()
            .map(|group| group.fields.len())
            .sum::<usize>()
            <= 2000,
        "Too many changes for structured comparison; use Source Diff"
    );
    ensure!(
        review
            .groups
            .iter()
            .flat_map(|group| &group.fields)
            .all(|field| field.before.len().max(field.after.len()) <= 16 * 1024),
        "A changed value is too large for structured comparison; use Source Diff"
    );
    review.source_only = before == after && source.before != source.after;
    review.renamed_from = original.map(str::to_string);
    Ok(review)
}

fn compare(
    before: Option<&Value>,
    after: Option<&Value>,
    cancellation: &Cancellation,
) -> Result<StructuredReview> {
    let mut review = StructuredReview {
        groups: Vec::new(),
        unchanged: 0,
        source_only: false,
        renamed_from: None,
    };
    let mut fields = Vec::new();
    compare_fields("", before, after, &mut fields, 0, true, cancellation)?;
    if !fields.is_empty() {
        review.groups.push(ChangeGroup {
            title: "Settings".into(),
            kind: kind(before, after),
            fields,
        });
    }
    if before.is_some_and(Value::is_array) || after.is_some_and(Value::is_array) {
        return Ok(review);
    }
    for key in ["scenarios", "actions", "viewports"] {
        let previous = before.and_then(|value| value.get(key));
        let current = after.and_then(|value| value.get(key));
        if previous.is_some_and(Value::is_array) || current.is_some_and(Value::is_array) {
            compare_items(key, previous, current, "", &mut review, cancellation)?;
        }
    }
    if review.groups.is_empty() && (before.is_none() || after.is_none()) {
        review.groups.push(ChangeGroup {
            title: "File".into(),
            kind: kind(before, after),
            fields: vec![field("Contents", before, after)],
        });
    }
    Ok(review)
}

fn kind(before: Option<&Value>, after: Option<&Value>) -> ChangeKind {
    match (before, after) {
        (None, Some(_)) => ChangeKind::Added,
        (Some(_), None) => ChangeKind::Removed,
        _ => ChangeKind::Changed,
    }
}

fn format_value(key: &str, value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "Not set".into();
    };
    match value {
        Value::Bool(value) => if *value { "Yes" } else { "No" }.into(),
        Value::Null => "Null".into(),
        Value::String(value) if value.is_empty() => "Empty text".into(),
        Value::String(value) => value.clone(),
        Value::Number(number)
            if key.ends_with("Timeout")
                || matches!(key, "delay" | "postInteractionWait" | "wait") =>
        {
            format!("{number} ms")
        }
        Value::Number(number) if key == "misMatchThreshold" => format!("{number}%"),
        Value::Array(values) if values.is_empty() => "Empty list".into(),
        Value::Array(values) if values.iter().all(Value::is_string) => values
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn field(label: &str, before: Option<&Value>, after: Option<&Value>) -> FieldChange {
    FieldChange {
        label: label.into(),
        before: format_value(label, before),
        after: format_value(label, after),
        kind: kind(before, after),
    }
}

fn compare_fields(
    prefix: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    fields: &mut Vec<FieldChange>,
    depth: usize,
    collections: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
    ensure!(
        depth <= 32 && fields.len() <= 2000,
        "Structured comparison is too large; use Source Diff"
    );
    if before == after {
        return Ok(());
    }
    if before.is_some_and(Value::is_object) || after.is_some_and(Value::is_object) {
        if before.is_some_and(|value| !value.is_object())
            || after.is_some_and(|value| !value.is_object())
        {
            fields.push(field(
                if prefix.is_empty() {
                    "Contents"
                } else {
                    prefix
                },
                before,
                after,
            ));
            return Ok(());
        }
        let keys = before
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(|value| value.keys())
            .chain(
                after
                    .and_then(Value::as_object)
                    .into_iter()
                    .flat_map(|value| value.keys()),
            )
            .collect::<BTreeSet<_>>();
        if keys.is_empty() {
            fields.push(field(
                if prefix.is_empty() {
                    "Contents"
                } else {
                    prefix
                },
                before,
                after,
            ));
        }
        for key in keys {
            ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
            ensure!(
                fields.len() < 2000,
                "Too many changes for structured comparison; use Source Diff"
            );
            let previous = before.and_then(|value| value.get(key));
            let current = after.and_then(|value| value.get(key));
            if collections
                && ["scenarios", "actions", "viewports"].contains(&key.as_str())
                && (previous.is_some_and(Value::is_array) || current.is_some_and(Value::is_array))
            {
                continue;
            }
            let label = schema::label(key);
            let label = if prefix.is_empty() {
                label
            } else {
                format!("{prefix} / {label}")
            };
            if previous.is_some_and(Value::is_object) || current.is_some_and(Value::is_object) {
                compare_fields(
                    &label,
                    previous,
                    current,
                    fields,
                    depth + 1,
                    false,
                    cancellation,
                )?;
            } else if previous != current {
                fields.push(FieldChange {
                    label,
                    before: format_value(key, previous),
                    after: format_value(key, current),
                    kind: kind(previous, current),
                });
            }
        }
    } else {
        fields.push(field(
            if prefix.is_empty() {
                "Contents"
            } else {
                prefix
            },
            before,
            after,
        ));
    }
    Ok(())
}

fn identity(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn action_name(value: &Value) -> Option<&'static str> {
    crate::desktop::document::ACTIONS
        .iter()
        .copied()
        .find(|action| value.get(*action).is_some())
}

fn item_title(collection: &str, item: &Value, index: usize) -> String {
    let prefix = match collection {
        "scenarios" => "Scenario",
        "actions" => "Step",
        _ => "Viewport",
    };
    let label = identity(item, "label")
        .or_else(|| identity(item, "id"))
        .or_else(|| identity(item, "url"))
        .or_else(|| action_name(item).map(schema::label))
        .unwrap_or_else(|| (index + 1).to_string());
    if collection == "actions" {
        format!("{prefix} {}: {label}", index + 1)
    } else {
        format!("{prefix}: {label}")
    }
}

fn compare_items(
    collection: &str,
    before: Option<&Value>,
    after: Option<&Value>,
    parent: &str,
    review: &mut StructuredReview,
    cancellation: &Cancellation,
) -> Result<()> {
    ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
    if before.is_some_and(|value| !value.is_array()) || after.is_some_and(|value| !value.is_array())
    {
        review.groups.push(ChangeGroup {
            title: format!("{parent} {}", schema::label(collection))
                .trim()
                .into(),
            kind: kind(before, after),
            fields: vec![field(collection, before, after)],
        });
        return Ok(());
    }
    let previous = before
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let current = after
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    ensure!(
        previous.len().max(current.len()) <= 512,
        "Too many items for structured comparison; use Source Diff"
    );
    if previous == current {
        review.unchanged += current.len();
        if before != after {
            review.groups.push(ChangeGroup {
                title: if parent.is_empty() {
                    schema::label(collection)
                } else {
                    format!("{parent} / {}", schema::label(collection))
                },
                kind: kind(before, after),
                fields: vec![field(collection, before, after)],
            });
        }
        return Ok(());
    }
    let mut matched = vec![None; current.len()];
    let mut used = vec![false; previous.len()];
    for criterion in 0..5 {
        for (new_index, new) in current.iter().enumerate() {
            ensure!(!cancellation.is_cancelled(), "Git operation cancelled");
            if matched[new_index].is_some() {
                continue;
            }
            let matches = |old: &Value, new: &Value| {
                let old_id = identity(old, "id");
                let new_id = identity(new, "id");
                if old_id.is_some() && new_id.is_some() && old_id != new_id {
                    return false;
                }
                match criterion {
                    0 => old_id.is_some() && old_id == new_id,
                    1 => old == new,
                    2 => {
                        identity(old, "label").is_some()
                            && identity(old, "label") == identity(new, "label")
                    }
                    3 if collection == "scenarios" => {
                        identity(old, "url").is_some()
                            && identity(old, "url") == identity(new, "url")
                    }
                    4 if collection == "actions" => {
                        action_name(old).is_some() && action_name(old) == action_name(new)
                    }
                    _ => false,
                }
            };
            let candidates = previous
                .iter()
                .enumerate()
                .filter(|(index, old)| !used[*index] && matches(old, new))
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if let [old_index] = candidates.as_slice() {
                let competing = current
                    .iter()
                    .enumerate()
                    .filter(|(index, candidate)| {
                        matched[*index].is_none() && matches(&previous[*old_index], candidate)
                    })
                    .count();
                if competing == 1 {
                    matched[new_index] = Some(*old_index);
                    used[*old_index] = true;
                }
            }
        }
    }
    let mut old_order = matched.iter().flatten().copied().collect::<Vec<_>>();
    old_order.sort_unstable();
    let new_order = matched.iter().flatten().copied().collect::<Vec<_>>();
    let items = current
        .iter()
        .enumerate()
        .map(|(index, item)| {
            (
                matched[index].map(|old| (old, &previous[old])),
                Some((index, item)),
            )
        })
        .chain(
            previous
                .iter()
                .enumerate()
                .filter(|(index, _)| !used[*index])
                .map(|item| (Some(item), None)),
        );
    for (old, new) in items {
        let previous = old.map(|(_, item)| item);
        let current = new.map(|(_, item)| item);
        let (index, item) = new.or(old).expect("comparison item has a side");
        let title = item_title(collection, item, index);
        let title = if parent.is_empty() {
            title
        } else {
            format!("{parent} / {title}")
        };
        let mut fields = Vec::new();
        compare_fields(
            "",
            previous,
            current,
            &mut fields,
            0,
            collection == "scenarios",
            cancellation,
        )?;
        let moved = old.zip(new).is_some_and(|((old, _), _)| {
            old_order.iter().position(|index| *index == old)
                != new_order.iter().position(|index| *index == old)
        });
        if moved {
            fields.insert(
                0,
                FieldChange {
                    label: "Order".into(),
                    before: (old.expect("matched item").0 + 1).to_string(),
                    after: (new.expect("matched item").0 + 1).to_string(),
                    kind: ChangeKind::Moved,
                },
            );
        }
        if fields.is_empty() && previous == current {
            review.unchanged += 1;
        }
        if !fields.is_empty() || previous.is_none() || current.is_none() {
            review.groups.push(ChangeGroup {
                title: title.clone(),
                kind: if moved && previous == current {
                    ChangeKind::Moved
                } else {
                    kind(previous, current)
                },
                fields,
            });
        }
        if collection == "scenarios" {
            for nested in ["actions", "viewports", "scenarios"] {
                let previous = previous.and_then(|item| item.get(nested));
                let current = current.and_then(|item| item.get(nested));
                if previous.is_some_and(Value::is_array) || current.is_some_and(Value::is_array) {
                    ensure!(
                        nested != "scenarios",
                        "Nested scenarios require Source Diff"
                    );
                    compare_items(nested, previous, current, &title, review, cancellation)?;
                }
            }
        }
        ensure!(
            review.groups.len() <= 2000,
            "Too many changes for structured review; use Source Diff"
        );
    }
    Ok(())
}

pub(super) struct FileVersions {
    pub before: Option<String>,
    pub after: Option<String>,
}

fn text(bytes: Vec<u8>) -> Result<String> {
    ensure!(
        bytes.len() <= MAX_DOCUMENT_BYTES,
        "File exceeds the 4 MiB structured review limit"
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
                "Only regular files support structured review"
            );
            let length: usize = metadata[3].parse()?;
            ensure!(
                length <= MAX_DOCUMENT_BYTES,
                "Committed file exceeds the 4 MiB structured review limit"
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
                "Only regular files support structured review"
            );
            ensure!(
                metadata.len() <= MAX_DOCUMENT_BYTES as u64,
                "Saved file exceeds the 4 MiB structured review limit"
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
    use serde_json::json;

    #[test]
    fn structured_review_groups_settings_scenarios_actions_and_viewports() {
        let before = json!({"debug": false, "scenarios": [{"id":"checkout", "label":"Checkout", "url":"/checkout", "viewportNames":["desktop","tablet"], "actions":[{"wait":1000}, {"click":"#pay"}]}]});
        let after = json!({"debug": true, "scenarios": [{"id":"checkout", "label":"Checkout", "url":"/checkout/new", "viewportNames":["desktop"], "actions":[{"wait":2500}, {"click":"#pay"}, {"save":"done"}]}]});
        let review = compare(Some(&before), Some(&after), &Cancellation::default()).unwrap();
        assert_eq!(review.groups.len(), 4);
        assert_eq!(review.groups[0].fields[0].before, "No");
        assert_eq!(review.groups[0].fields[0].after, "Yes");
        assert_eq!(review.groups[1].title, "Scenario: Checkout");
        assert!(
            review.groups[1]
                .fields
                .iter()
                .any(|field| field.label == "URL" && field.after == "/checkout/new")
        );
        assert_eq!(review.groups[2].fields[0].before, "1000 ms");
        assert_eq!(review.groups[2].fields[0].after, "2500 ms");
        assert_eq!(review.groups[3].kind, ChangeKind::Added);
        assert_eq!(review.unchanged, 1);
    }

    #[test]
    fn structured_review_detects_moves_but_not_index_shifts_from_insertions() {
        let before = json!({"actions":[{"click":"#pay"},{"wait":1000}]});
        let moved = json!({"actions":[{"wait":1000},{"click":"#pay"}]});
        let review = compare(Some(&before), Some(&moved), &Cancellation::default()).unwrap();
        assert!(
            review
                .groups
                .iter()
                .all(|group| group.kind == ChangeKind::Moved)
        );
        let inserted = json!({"actions":[{"goto":"/cart"},{"click":"#pay"},{"wait":1000}]});
        let review = compare(Some(&before), Some(&inserted), &Cancellation::default()).unwrap();
        assert_eq!(review.groups.len(), 1);
        assert_eq!(review.groups[0].kind, ChangeKind::Added);
    }

    #[test]
    fn structured_review_preserves_unknown_null_false_and_ambiguous_changes() {
        let before = json!({"extension":null,"scenarios":[{"label":"Same","url":"/one"},{"label":"Same","url":"/two"}]});
        let after = json!({"extension":false,"scenarios":[{"label":"Same","url":"/three"},{"label":"Same","url":"/four"}]});
        let review = compare(Some(&before), Some(&after), &Cancellation::default()).unwrap();
        assert_eq!(review.groups[0].fields[0].before, "Null");
        assert_eq!(review.groups[0].fields[0].after, "No");
        assert_eq!(
            review
                .groups
                .iter()
                .filter(|group| group.kind == ChangeKind::Added)
                .count(),
            2
        );
        assert_eq!(
            review
                .groups
                .iter()
                .filter(|group| group.kind == ChangeKind::Removed)
                .count(),
            2
        );
    }

    #[test]
    fn structured_review_distinguishes_absent_empty_collections_and_item_removal() {
        let before =
            json!({"scenarios": [{"id":"removed", "url":"/old", "actions":[{"wait":1000}]}]});
        let after = json!({"scenarios": []});
        let review = compare(Some(&before), Some(&after), &Cancellation::default()).unwrap();
        assert_eq!(review.groups.len(), 2);
        assert!(
            review
                .groups
                .iter()
                .all(|group| group.kind == ChangeKind::Removed)
        );
        let before = json!({"enabled":false, "viewportNames":[], "override":null});
        let review = compare(Some(&before), Some(&json!({})), &Cancellation::default()).unwrap();
        assert_eq!(review.groups[0].fields.len(), 3);
        assert!(
            review.groups[0]
                .fields
                .iter()
                .all(|field| field.kind == ChangeKind::Removed && field.after == "Not set")
        );
        let review = compare(
            Some(&json!({})),
            Some(&json!({"scenarios":[]})),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(review.groups[0].fields[0].before, "Not set");
        assert_eq!(review.groups[0].fields[0].after, "Empty list");
        let review = compare(
            Some(&json!({"engineOptions": {}})),
            Some(&json!({"otherSettings": {}, "enabled": true})),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(review.groups[0].fields.len(), 3);
        assert!(review.groups[0].fields.iter().any(|field| {
            field.label == "Engine options" && field.before == "{}" && field.after == "Not set"
        }));
        assert!(review.groups[0].fields.iter().any(|field| {
            field.label == "Other settings" && field.before == "Not set" && field.after == "{}"
        }));
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(compare(Some(&before), None, &cancellation).is_err());
    }
}
