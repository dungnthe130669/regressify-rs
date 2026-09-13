# Outstanding Issues

Issues identified during adversarial review that remain after the September 2026 hardening pass.

## Resolved In September 2026 Hardening Pass

- Scenario `selectors` supports full-page defaults, `viewport`, and visible CSS selector clips with stable target-specific filenames and report ordering.
- `asyncCompareLimit` is now a validated bounded worker limit for parallel pixel comparison.
- Report regeneration ignores generated diffs and clears stale diff artifacts when a pair becomes a pass.
- Screenshot output writes atomically, preserves color-management PNG chunks, and uses collision-resistant run IDs.
- Capture waits for an interactive DOM, lazy-loaded resources, decoded fonts/images, and stable layout; capture-only animation freezing is configurable.
- Interactive actions use full actionability validation, URL replacements apply to `goto`, and iframe waits/coordinates are hardened.
- URL waits support exact URLs, single-segment `*`, multi-segment `**`, and query-value wildcards.

## Resolved In June 2026 Fix Pass

- `press` now uses CDP `Input.dispatchKeyEvent` and supports modifier combinations like `Ctrl+A`, `Shift+Tab`, and `Meta+K`.
- Replacement profiles are loaded, validated, and applied during `test`; invalid regexes fail early as configuration errors.
- Resolved `hideSelectors`/`removeSelectors` are applied through the config cascade.
- `cookiePath` and persisted state paths are confined to the project/output directories; state respects `--site-env`.
- Unknown viewport names, duplicate viewport labels, empty labels, and zero dimensions fail before browser launch.
- Visual mismatches and website/action failures are report results, not process failures.
- `.tests.yml` and `.tests.json` files are parsed from their discovered paths instead of being incorrectly remapped to `.tests.yaml`.
- `needs` dependencies now fail early as configuration errors when missing, duplicated, cyclic, or deeper than 100 levels.
- Duplicate suite formats are deduplicated by suite name with `.tests.yaml` > `.tests.yml` > `.tests.json` precedence.
- `postInteractionWait` defaults to `1000` milliseconds through the workspace/suite/scenario cascade.
- `wait.url` uses active URL replacements in `test` mode; `ref` captures and waits against raw URLs.
- `restore`, `requiredLogin` / `--requiredLogin`, `init`, `install`, and `version` are implemented in the Rust CLI where they fit the Chromium/CDP design.
