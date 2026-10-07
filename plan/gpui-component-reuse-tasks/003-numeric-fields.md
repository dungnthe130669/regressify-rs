# 003: Numeric Fields With Lossless Drafts

Status: Verified; local commit pending. Depends on 001. See README for results
and the approved explicit `set_step(None)` correction.

## Scope

Migrate schema numeric controls in [ui/mod.rs](../../src/desktop/ui/mod.rs) to Kit `NumberInput` with a lossless application adapter. Source font-size preferences are owned separately by [018](018-source-font-size.md). Do not change text/path fields, config parsing, or numeric coercion policy.

## Ordered Work

1. Before edits, run `cargo test --locked --features gui-tests --lib numeric_stepper_ -- --test-threads=1` and `cargo test --locked --features gui-tests --lib pixel_match_threshold_ -- --test-threads=1`; confirm nonzero counts and record behavior. Inspect the native UI and lossless-editing skills.
1. Explicitly construct numeric field input state with `MaskPattern::None`; relying on the default mask is not equivalent because the selected-mask state prevents `ensure_number_mask` from adding it.
1. Call `InputState::set_step(None, window, cx)` explicitly and leave min/max unset so Kit emits `NumberInputEvent::Step` rather than rewriting text. Published 0.7.1 defaults to fixed step one; omitting the builder is insufficient. Subscribe to Step and call existing `adjust_field` for increment/decrement. Keep `InputEvent::Change` routed through `change_field`.
1. Render NumberInput only for numeric schema fields; retain Input for paths and other text. Preserve inherited lookup, integer/fractional steps, schema limits, explicit/inherited state, invalid drafts, reset, history, focus, disabled state, stable wrappers, and accessible names. Test the actual editable child, not wrapper metadata.
1. Extend existing native tests for invalid/intermediate text, exponent notation, keyboard stepping, busy state, blur without coercion, reset, and undo/redo. Review net deletion including subscriptions/adapters; maintain affected docs/guidance/memory.

## Acceptance And Checks

Run the two baseline filters above after editing and `cargo fmt --all -- --check`. These filter names are known; require a nonzero count and preserve their existing assertions. Add regressions to the existing native test module for any uncovered behavior, under the existing `numeric_stepper_` / `pixel_match_threshold_` filter where semantically appropriate; do not claim newly proposed cases already exist. No new parallel test module.

Follow [desktop lossless editing](../../.github/skills/desktop-lossless-editing/SKILL.md), [native UI](../../.github/skills/desktop-native-ui/SKILL.md), [configuration cascade](../../.github/skills/config-cascade/SKILL.md), and [Rust feature checks](../../.github/skills/rust-feature-checks/SKILL.md). Preserve invalid config drafts and document history. Commit only numeric schema-field adapter, its existing-module tests, and necessary maintained docs/guidance.

Reopen if the released NumberInput cannot emit steps without coercing text, if limits/inherited values cannot remain application-owned, or if undo/redo or raw invalid input changes. Do not move Source font controls into this task.
