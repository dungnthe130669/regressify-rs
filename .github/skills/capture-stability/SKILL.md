---
name: capture-stability
description: 'Use when changing screenshot settling, lazy loading, animation freezing, selector clips, viewport captures, save names, PNG metadata, or capture manifests.'
---

# Stable Screenshot Capture

## Start Here

[Page stability](../../../src/test/page_stability.rs), [capture](../../../src/test/capture.rs), [scenario execution](../../../src/test/test_scenario.rs), and [PNG safety](../../../src/helpers/png.rs).

## Procedure and Invariants

1. Preserve bounded DOM/resource/font/image/layout readiness and configurable settling. Avoid replacing observable conditions with arbitrary sleeps.
1. Run lazy-load scrolling after initial navigation/readiness hooks and before prerequisite, scenario, or legacy actions so it cannot disturb their hover or scroll state. `noScrollTop` skips this pass; keep it distinct from selecting a viewport-only capture. Apply animation freezing only at its intended stage.
1. Bypass CSS smooth scrolling for the lazy-load pass and verify both offsets stay at the top for a quiet window before returning. Bound the completion wait and propagate failure; a fixed delay or later capture settling is not proof that scrolling finished.
1. Support full-page, viewport, CSS target, and intermediate save captures without collisions. Validate save names and keep stable target-specific ordering.
1. Write complete PNGs/manifests atomically; preserve required color metadata and resource bounds. Failed capture placeholders need error markers, not promotion.
1. Keep labels, scenario IDs, target names, thresholds, and raw/replaced URLs in sync across capture manifests and report consumers.
1. Persist `scenarioDisplayName` from the [shared resolver](../../../src/helpers/scenario_names.rs) against the complete suite before selection or URL replacements. Preserve the source label separately; generated names never feed IDs, capture keys, or filenames. Comparison regeneration must retain recorded names.
1. The shared error-placeholder PNG lives in `src/test/data/error-icon.png`, not the generated snapshot asset folder replaced by frontend builds.

## Checks

```sh
cargo test --lib test::capture::tests
cargo test --lib test::page_stability::tests
cargo test --lib test::test_scenario::tests
cargo test --lib helpers::png::tests
```

For pre-action lazy loading and hover preservation, with managed Chromium already installed:

```sh
cargo test --lib test::test_scenario::tests::lazy_loading_precedes_actions_and_preserves_hover -- --ignored --exact
cargo test --lib test::page_stability::tests::lazy_loading_waits_for_stable_offsets_and_bounds_stalled_scroll -- --ignored --exact
```

Cover absent/empty actions with capture settling disabled, smooth scrolling, delayed or interrupted returns, non-scrolling pages, and blocked-scroll timeouts.

Inspect ignored Chromium fixtures for live behavior; unit tests alone do not prove rendering stability. Cover delayed resources, motion, clipped targets, multiple saves, invalid names, timeouts, and write failures.

## Maintain

Update [scenario docs](../../../docs/create-scenario.md), [action docs](../../../docs/create-action.md), and this skill. Explain readiness heuristics and filename/metadata invariants in concise comments.
