# Skill: `install` Command

## When to Use

Use this skill when changing or reviewing the `regressify install` command.

## Key Files

- `src/cli/command.rs` — command variant
- `src/main.rs` — command dispatch
- `src/test/browsers.rs` — Chromium fetcher and executable validation
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `install` fetches the Chromium browser binary used by the CDP runner.
- Fetch/download/permission failures are infrastructure errors and must produce a non-zero exit.
- The command should print the resolved browser executable path on success.

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

Avoid running the full browser download in routine unit tests.
