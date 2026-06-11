# Skill: `version` Command

## When to Use

Use this skill when changing or reviewing the `regressify version` command.

## Key Files

- `src/cli/command.rs` — command variant
- `src/main.rs` — command dispatch
- `Cargo.toml` — package version source
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `version` prints the Cargo package version from `env!("CARGO_PKG_VERSION")`.
- `regressify --version` remains available through Clap.
- The command should not require browser/local test setup.

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```
