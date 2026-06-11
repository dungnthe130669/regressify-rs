# Skill: `test` Command

## When to Use

Use this skill when changing or reviewing the `regressify test` command.

## Key Files

- `src/cli/args.rs` — `TestArgs`
- `src/test/test_suites.rs` — `exec_test_suites`, replacement profile loading, capture job orchestration
- `src/test/test_suite.rs` — per-scenario launch and outcome classification
- `src/test/test_scenario.rs` — scenario lifecycle and URL replacements
- `src/test/capture.rs` — screenshot output, report order, threshold manifests, error markers
- `src/compare/report_gen.rs` — post-capture comparison
- `src/config/mod.rs`, `src/config/replacements.rs`, `src/config/viewports.rs` — resolved config and early validation
- `docs/commands.md` — user-facing command semantics

## Behavior Contract

- `test` captures current screenshots and compares against references unless `--skip-compare` is set.
- Configuration errors fail before browser fetch/launch: malformed suites, invalid viewports, invalid URL replacement regexes, unsupported browsers.
- Replacement profile order is CLI `--replacement-profile`, `REPLACEMENT_PROFILE`, then `default`.
- URL replacements apply to scenario URLs and action `wait.url` values in `test` mode.
- Page/action failures write placeholder screenshots and report entries, continue sibling scenarios, and exit zero.
- Visual mismatches, missing references, and comparison failures are report data and exit zero.
- Browser fetch/launch failures and report/artifact generation errors are non-zero.

## Review Checklist

- Are selected suites parsed with `parse_test_suites` instead of silently skipped?
- Are report order and threshold manifests written before capture?
- Does `build_capture_jobs` return `Result` and propagate viewport errors?
- Are `ScenarioOutcome::PageFailed` and `ScenarioOutcome::InfraFailed` kept distinct?
- Does comparison failure counting only represent report generation errors, not visual diffs?

## Verification

Run:

```bash
cargo fmt --check
cargo check
cargo test
```

For command semantics, test one suite with an intentionally missing selector and confirm the command exits zero while the report records the failed scenario clearly.
