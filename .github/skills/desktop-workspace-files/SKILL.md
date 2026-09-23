---
name: desktop-workspace-files
description: 'Use when changing desktop file discovery, Save/Save As, suite creation/duplication/rename/delete, workspace paths, preferences, or external-conflict handling.'
---

# Desktop Workspace Files

## Start Here

[Workspace operations](../../../src/desktop/workspace.rs),
[session transitions](../../../src/desktop/session.rs), and
[atomic I/O](../../../src/helpers/io.rs).

## Procedure and Invariants

1. Resolve workspace-relative components with portable-name and canonical
   containment checks. Validate existing ancestors, not just the final filename.
1. Validate both structured values and original typed source before save; direct
   typed parsing catches duplicate fields that a generic value can collapse.
1. Compare on-disk bytes with saved_source before replacing, renaming, or deleting.
   Preserve the draft on conflict. Atomic replacement alone does not detect edits.
1. Create/Save As must not clobber an existing file, even between existence check
   and write. Preserve the existing no-clobber temporary-file mechanism.
1. Duplication removes explicit suite IDs to avoid shared output. Preserve format
   on rename and do not erase the source if creating the destination fails.
1. Keep discovery precedence aligned with the CLI; runnable_suite must reject a
   selected file shadowed by another format. Merge preferences without losing
   another window's recent workspaces or settings.

## Checks

```sh
cargo test --features desktop --lib desktop::workspace::tests
cargo test --features desktop --lib desktop::session::tests
```

Cover external edits/deletion, existing destinations, path traversal/symlinks,
reserved names, duplicate/shadowed suites, read-only paths, and concurrent settings.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill. Comment why conflict
and no-clobber guards exist; retain failure-path tests whenever I/O is refactored.
