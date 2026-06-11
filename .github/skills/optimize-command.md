# Skill: `optimize` Command

## When to Use

Use this skill when changing or reviewing the `regressify optimize` command.

## Key Files

- `src/cli/args.rs` — `OptimizeArgs`
- `src/optimizer/mod.rs` — command entry point
- `src/optimizer/image_optimizer.rs` — PNG optimization logic
- `src/optimizer/process.rs` — optimization execution helpers
- `src/helpers/io.rs`, `src/helpers/numbers.rs` — formatting and path helpers
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `optimize` compresses PNG assets according to `--min-quality`, `--max-quality`, `--speed`, and `--min-size-kb`.
- Quality must stay between 0 and 100; speed must stay between 1 and 10.
- Optimization should avoid rewriting files that do not meet the minimum size threshold.
- File I/O and optimizer failures are process failures when the command cannot complete safely.

## Review Checklist

- Does argument validation in `src/cli/args.rs` still reject invalid ranges?
- Are optimization outputs deterministic enough for test artifacts?
- Does the command avoid unnecessary rewrites of small files?
- Are errors surfaced with enough path context to fix broken assets?

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

For behavior changes, run optimization on a temporary PNG set and confirm size thresholds, quality settings, and unchanged small files behave as expected.
