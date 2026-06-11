# Skill: `compare` Command

## When to Use

Use this skill when changing or reviewing the `regressify compare` command or comparison report generation.

## Key Files

- `src/cli/args.rs` — `CompareArgs`
- `src/compare/actions.rs` — `compare` entry point and suite filtering
- `src/compare/report_gen.rs` — report JSON/config generation, threshold manifests, error marker handling
- `src/compare/image_compare.rs` — pixel comparison and diff image generation
- `src/test/capture.rs` — manifest and error marker constants
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `compare` finds each matching suite under the active Regressify output directory and compares its latest `bitmaps_test/<run>` folder.
- Visual mismatches, missing references, failed capture markers, and image comparison errors are report entries and should not make the command non-zero.
- Missing output directories or failed report/artifact writes are process failures.
- Per-scenario thresholds come from `.report-thresholds.json` when present; CLI `--mis-match-threshold` is the fallback for older runs.
- Report ordering uses `.report-order.json` when present and falls back to filename ordering otherwise.

## Review Checklist

- Does `generate_comparison_report` always write `report.json` and `html_report/config.js` when possible?
- Does it avoid `bail!` for visual diffs or missing references?
- Does it skip visual pixel comparison when `.error.json` markers exist?
- Are diff image paths relative to `html_report/` and stable for the report UI?

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

For behavior changes, compare a run with a known mismatch and confirm the process exits zero while the report marks the pair as failed clearly.
