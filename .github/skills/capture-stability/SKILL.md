---
name: capture-stability
description: 'Use when changing screenshot settling, lazy loading, animation freezing, selector clips, viewport captures, save names, PNG metadata, or capture manifests.'
---

# Stable Screenshot Capture

## Start Here

[Page stability](../../../src/test/page_stability.rs),
[capture](../../../src/test/capture.rs), [scenario execution](../../../src/test/test_scenario.rs),
and [PNG safety](../../../src/helpers/png.rs).

## Procedure and Invariants

1. Preserve bounded DOM/resource/font/image/layout readiness and configurable
   settling. Avoid replacing observable conditions with arbitrary sleeps.
1. Keep lazy-load scrolling and noScrollTop behavior distinct from selecting a
   viewport-only capture. Apply animation freezing only at its intended stage.
1. Support full-page, viewport, CSS target, and intermediate save captures without
   collisions. Validate save names and keep stable target-specific ordering.
1. Write complete PNGs/manifests atomically; preserve required color metadata and
   resource bounds. Failed capture placeholders need error markers, not promotion.
1. Keep labels, scenario IDs, target names, thresholds, and raw/replaced URLs in
   sync across capture manifests and report consumers.

## Checks

```sh
cargo test --lib test::capture::tests
cargo test --lib test::page_stability::tests
cargo test --lib helpers::png::tests
```

Inspect ignored Chromium fixtures for live behavior; unit tests alone do not
prove rendering stability. Cover delayed resources, motion, clipped targets,
multiple saves, invalid names, timeouts, and write failures.

## Maintain

Update [scenario docs](../../../docs/create-scenario.md),
[action docs](../../../docs/create-action.md), and this skill. Explain readiness
heuristics and filename/metadata invariants in concise comments.
