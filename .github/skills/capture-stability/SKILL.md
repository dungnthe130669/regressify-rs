---
name: capture-stability
description: 'Use when changing screenshot settling, lazy loading, animation freezing, caret hiding, selector clips, viewport captures, save names, PNG metadata, or capture manifests.'
---

# Stable Screenshot Capture

## Start Here

[Page stability](../../../src/test/page_stability.rs), [capture](../../../src/test/capture.rs), [scenario execution](../../../src/test/test_scenario.rs), and [PNG safety](../../../src/helpers/png.rs).

## Procedure and Invariants

1. Preserve bounded DOM/resource/font/image/layout readiness and configurable settling. Avoid replacing observable conditions with arbitrary sleeps.
1. Run lazy-load scrolling after initial navigation/readiness hooks and before prerequisite, scenario, or legacy actions so it cannot disturb their hover or scroll state. `noScrollTop` skips this pass; keep it distinct from selecting a viewport-only capture.
1. Bypass CSS smooth scrolling for the lazy-load pass and verify both offsets stay at the top for a quiet window before returning. Bound the completion wait and propagate failure; a fixed delay or later capture settling is not proof that scrolling finished.
1. Stabilize initial animations after readiness hooks and before lazy loading/actions when enabled. Share `prepare_screenshot` for `save` and final captures: resolved delay, animation freeze, bounded settling, then another freeze for late animations. Honor opt-outs and do not scroll or move the pointer during this preparation. `delay` resolves scenario -> workspace -> default; `postInteractionWait` remains final-only.
1. Hide text carets independently of animation freezing and settling. Preserve focus, selection, and values; do not blur controls or weaken comparison thresholds to hide cursor noise.
1. Support full-page, viewport, CSS target, and save captures without collisions. Optional `save.selector` clips only that save; scenario `selector` clips only `FINAL`. Keep filenames independent of these selectors, reject blanks and simultaneous scenario `selector`/`selectors`, and restore scroll after clips. `viewport` is reserved in all three selector fields and captures the visible viewport without scrolling. Element clips are the target's border box after centering it, so layers overflowing the target are excluded and overlays inside it are included.
1. Plan `save` checkpoints per viewport with the same applicability predicate as execution, including `needs`. Validate portable save names unconditionally, but check conditional collisions against actual per-viewport inventory; disjoint same-name saves are valid. Omit the automatic final capture when the last applicable own action is `save`, but always retain explicit scenario `selectors`. Prerequisite saves alone do not suppress finals. Progress, manifests, errors, and comparison must share this decision; never delete historical baselines.
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

For scenario and checkpoint preparation, with managed Chromium already installed:

```sh
cargo test --lib test::test_scenario::tests::lazy_loading_precedes_actions_and_preserves_hover -- --ignored --exact
cargo test --lib test::page_stability::tests::lazy_loading_waits_for_stable_offsets_and_bounds_stalled_scroll -- --ignored --exact
cargo test --lib test::page_stability::tests::screenshot_preparation_hides_carets_without_blurring_inputs -- --ignored --exact
cargo test --lib test::test_action::tests::save_actions_wait_for_the_resolved_screenshot_delay -- --ignored --exact
cargo test --lib test::test_scenario::tests::selector_saves_and_final_captures_match_pixels_and_inventory -- --ignored --exact
```

The selector fixture verifies default/full-page pixels, independent save/final clips, quoted selectors, skipped finals and their waits, explicit plural targets, viewport filtering, missing/hidden/invalid targets, preserved completed saves, and restored scroll offsets.

Cover absent/empty actions with capture settling disabled, smooth scrolling, delayed or interrupted returns, non-scrolling pages, and blocked-scroll timeouts. Checkpoint coverage must verify saved pixels, inherited/overridden delay, repeated saves, invalid-name rejection before waiting, animation opt-outs, and animations started during settling. Caret coverage must check inputs, textareas, and editable content with animation freezing both on and off, unchanged focus/selection/values, and identical screenshot pixels across a blink interval.

Inspect ignored Chromium fixtures for live behavior; unit tests alone do not prove rendering stability. Cover delayed resources, motion, clipped targets, multiple saves, invalid names, timeouts, and write failures.

Run `cargo test --lib test::test_scenario::tests::filtered_reference_and_test_manifests_preserve_baselines -- --ignored --exact` for real reference/test manifests, ordered pixels, all-actions-skipped final captures, scoped failure placeholders, peer isolation, and unchanged historical references. It uses loopback pages and cached Chromium only.

## Maintain

Update [scenario docs](../../../docs/create-scenario.md), [action docs](../../../docs/create-action.md), and this skill. Explain readiness heuristics and filename/metadata invariants in concise comments.
