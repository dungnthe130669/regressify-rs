---
name: desktop-git-review
description: 'Use when changing desktop Git status/diffs, selected commit/discard, branch/upstream checks, remote configuration, or destructive-action confirmation.'
---

# Desktop Git Review

## Start Here

[Git model](../../../src/desktop/git.rs), [structured diffs](../../../src/desktop/git/review.rs), and [Git UI](../../../src/desktop/ui/git.rs).

## Procedure and Invariants

1. Parse NUL-delimited machine output. Preserve rename pairs, conflicts, submodule state, literal filenames, and the repository root distinct from workspace root.
1. Revalidate repository/HEAD/branch and relevant remote/upstream configuration after review. For destructive selection, investigate content changes even when status letters remain unchanged; a status snapshot is not a content fingerprint.
1. Commit only selected paths while preserving unrelated staged work. Keep ignored comparison artifacts excluded unless explicitly included by the user.
1. Discard only selected files; protect unselected rename destinations and children, reject unsafe parents, and handle unborn branches without broad resets/cleans.
1. Pull is fast-forward-only and requires a clean appropriate state. Never force push, auto-stash, resolve conflicts, or contact real remotes without authorization.
1. Preserve dirty-editor guards and confirmation cancellation when context changes. Use no-ext-diff/no-textconv for previews and safe remote URL validation.
1. Keep keyboard selection scoped to the focused Changes or Comparison Changes tree. Select All and folder toggles exclude conflicts/submodules and must not include the other group implicitly. Focus/navigation never executes Git writes; commit-message inputs retain their text shortcuts.
1. Remote selection is repository-local `regressify.remote`; include it in the reviewed configuration fingerprint. Fetch, pull, push, and comparison counts must use that destination. Missing or removed active remotes must not fall back silently. Preserve local-upstream pulls and tracking branch names.
1. Keep remote form drafts bound to their opening snapshot; revalidate before add/select/update/rename/remove. URL edits preserve additional configured URLs and separate push URLs. Never retain credential-bearing URLs in snapshots or diagnostic logs.
1. Activity reporting shares cancellation with the process wrapper. Bound current/history logs, keep blocking work off the UI thread, show completion/cancellation/failure, and clear workspace-scoped logs on workspace changes.

## Checks

```sh
cargo test --features desktop --lib desktop::git::tests
cargo test --features gui-tests --lib desktop::ui::git::tests -- --test-threads=1
```

Reuse temporary repositories and local bare remotes. Set fixture-local `core.autocrlf=false` for byte-exact Windows assertions. Cover staged/untracked, renamed/deleted, conflicts, stale review, unsafe paths, and cancelled confirmation.

Remote-focused filters are `desktop::git::tests::remote_` and `desktop::ui::git::tests::remote_`. Native `./ci/desktop-smoke.ps1 -GitFlows` exercises remote forms, menus, removal cancellation, and progress/logs in compact and wide light/dark windows; observe the native UI skill's safety prerequisites.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill. Comment narrow pathspec and reviewed-state guarantees; record unresolved gaps in the issue list.
