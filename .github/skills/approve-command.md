# Skill: `approve` Command

## When to Use

Use this skill when changing or reviewing the `regressify approve` command.

## Key Files

- `src/cli/args.rs` — `ApproveArgs`
- `src/compare/actions.rs` — `approve` entry point and suite filtering
- `src/compare/report_gen.rs` — `approve_screenshots`
- `src/test/capture.rs` — error marker sidecars
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `approve` promotes screenshots from each suite's latest `bitmaps_test/<run>` folder to `bitmaps_reference`.
- It must skip diff images and failed capture placeholders.
- It must remove stale error markers for references overwritten by successful captures.
- Skipping failed placeholders is not a process failure.
- Missing or unreadable output directories are artifact errors only when the command cannot continue safely.

## Review Checklist

- Does approval filter suites using the same matching semantics as `compare`?
- Does `approve_screenshots` check `read_error_marker` or marker sidecars before copying?
- Does it avoid copying `_diff.png` files?
- Does it keep logs clear about approved and skipped screenshots?

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

For behavior changes, create a latest test run containing one normal screenshot and one `.error.json` sidecar, then confirm only the normal screenshot is promoted successfully.
