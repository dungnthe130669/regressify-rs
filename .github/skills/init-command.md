# Skill: `init` Command

## When to Use

Use this skill when changing or reviewing the `regressify init` command.

## Key Files

- `src/init.rs` — command implementation and starter-file contents
- `src/cli/command.rs` — command variant
- `src/main.rs` — command dispatch
- `docs/commands.md`, `docs/get-started.md` — user-facing command semantics

## Behavior Contract

- `init` creates missing starter files under `common/` and `visual_tests/`.
- Existing files must never be overwritten.
- File and directory creation errors are process failures.
- Generated sample files should remain minimal and valid for the Rust CLI.

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test init::tests
```
