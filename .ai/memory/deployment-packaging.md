# Deployment Packaging

Verified: 2026-09-23

- The [worker](../../src/desktop/worker.rs) regenerates selected snapshots, then
  passes a fresh staging directory to either provider. Azure's optional
  [optimization copy](../../src/desktop/deployment/budget.rs) inventories only
  that staging directory, not the original output tree.
- The [runner](../../src/desktop/runner.rs) must keep unchosen report text as
  `None`, otherwise generated CLI flags override workspace snapshot settings.
  Saved activity version 1 prefilled built-in text; loading it migrates those
  strings to inheritance. Version 2 preserves explicit defaults and custom text.
  The [worker staging test](../../tests/desktop_worker.rs) verifies configured
  title, heading, and Markdown links in the final staged HTML without publishing.
- [Staging](../../src/desktop/deployment.rs) reads `reference`, `test`, and
  `diffImage` from selected reports' `html_report/config.js`. Run age is not the
  criterion: referenced older-run images stay, while unreferenced current-run
  images, diffs, and metadata are skipped. Cache-busted paths are deduplicated.
- The shared allowlist preserves report scripts, workers, web fonts, icons, and
  hosting policies. The [suite template](../../src/snapshot/data/test_suite_report.html)
  uses WOFF/WOFF2, not the TTF files still generated for local output.
- Verification: `cargo test --features desktop --lib desktop::deployment::tests::staging_includes_only_selected_report_dependencies`
  asserts the exact ZIP entry set and unchanged source files. Its unused-TTF case
  failed before the allowlist fix. Checking only that required files exist does
  not detect extra uploaded files.

See the [packaging skill](../../.github/skills/deployment-packaging/SKILL.md) for
the maintained procedure and checks.
