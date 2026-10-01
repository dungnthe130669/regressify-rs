<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 014 — State/hooks, actions/frames, and screenshot-settling stage instrumentation

- Slug: `capture-state-actions-settling-stages`
- Implements plan section(s): `Phase 3 — Capture Instrumentation#0`, `Capture Diagnostics#0`
- Depends on: capture-queue-setup-stage

## Goal

Instrument state/hooks (restore/persist and before/ready hook stage, duration, outcome, safe error category), actions/frames (action index/type, safe target identity, frame/tab transitions, actionability condition awaited, retries, last unmet condition), and screenshot-settling (lazy-loading, font/image readiness, animation/caret handling, viewport/selector bounds, settling decisions) stages.

## Why this exists

Task 3.3's own acceptance criterion requires a fixture with a failing action to record the action index/type and the last unmet actionability condition, with no script contents or hook bodies appearing in the record.

## Settled investigation

- src/test/hooks.rs::DocumentHooks::load (lines 18-40+) already loads CSS override and before/ready hook script content from workspace files; this task's safe-error-category field must never include this script's text.
- src/test/test_action.rs already has scenario_log! calls for Check (line 274), Click (line 314), Focus (line 327), Goto (line 334), and Hide (line 346) actions, each naming the action and its target selector — these are the exact per-action-type call sites this task converts.
- src/test/page_stability.rs already has scenario_log! calls for delay waits (lines 24-30), page-settle outcomes (lines 47-67), and animation stabilization (lines 102-108) — these map directly to the screenshot-settling row's fields.
- actionability::ActionabilityConfig's poll_interval_ms=100 default (src/test/actionability.rs line 33) governs the existing actionability polling loop whose last-unmet-condition this task must surface explicitly.

## Inputs

- Task 3.1's queue/setup stage instrumentation
- src/test/hooks.rs, src/test/test_action.rs, src/test/page_stability.rs (modified, not replaced)

## Outputs

- State/hooks, actions/frames, and screenshot-settling diagnostic records for every capture, covering both success and the last-unmet-condition failure case

## Implementation steps

- Add tracing spans/events around DocumentHooks::load and its before/ready hook execution, recording stage/duration/outcome and a safe error category without the script's own source text.
- Convert each of test_action.rs's per-action-type scenario_log! calls (Check line 274, Click line 314, Focus line 327, Goto line 334, Hide line 346) to tracing events recording the action index/type and a safe target identity (a deterministic selector fingerprint/hash rather than the raw selector string) plus frame/tab transition info.
- Record each actionability retry/poll outcome and, on final timeout, the specific last unmet condition (e.g. 'element not visible', 'element obscured') rather than a generic timeout message.
- Convert page_stability.rs's existing scenario_log! calls (delay wait lines 24-30, settle outcome lines 47-67, animation stabilization lines 102-108) to tracing events for the screenshot-settling stage.
- Write failing_action_records_index_type_and_last_unmet_condition_without_script_contents: configure a scenario with a hook referencing a nonexistent element and an action targeting a selector that never becomes actionable, and assert the resulting records include the action's index/type and an explicit last-unmet-condition field, with no hook script source or raw selector text present.

## Done when

- Test failing_action_records_index_type_and_last_unmet_condition_without_script_contents records the failing action's index and type plus an explicit last-unmet-condition field.
- The same test's captured records contain no hook script source text and no raw selector string, only a safe target identity.

## Evidence to capture

- The fixture's exact hook/selector configuration and the resulting record's action-index/type/last-unmet-condition fields, plus confirmation that the raw hook source and raw selector text do not appear anywhere in output

## Do not change

- Do not record hook script source text or raw selector strings verbatim — Capture Diagnostics explicitly prohibits 'state or script contents' and Privacy and Safety flags selectors as potentially private.
- Do not alter DocumentHooks's existing CSS-override/hide-selector loading behavior; only add diagnostic recording around it.

## Reopen triggers

- Task 3.4's artifacts/finish stage work finds it needs the same safe-target-identity helper this task introduces for a different purpose, motivating sharing it.

## Context

This task covers the three middle rows of the Capture Diagnostics table — state/hooks, actions/frames, and screenshot-settling — which together account for the majority of existing scenario_log! call sites in src/test/, and are also the stages most likely to contain genuinely private content that must be reduced to safe identities rather than recorded verbatim.

## Options & tradeoffs

Recording raw selector strings would be more immediately readable for debugging but directly risks leaking private page structure/values per Privacy and Safety; a safe identity (deterministic hash/fingerprint, chosen) is less immediately readable but satisfies the plan's explicit requirement and still correlates repeated failures of the same selector.

## Decision

Instrument each of the three stages at their existing per-call-site locations in hooks.rs/test_action.rs/page_stability.rs, converting existing scenario_log! calls and adding a safe-identity transform for selectors.

## Acceptance criteria

- A fixture with a failing action records the action index/type and the last unmet actionability condition.
- No script contents or hook bodies, and no raw selector text, appear anywhere in that fixture's captured records.

## File touchpoints

- src/test/hooks.rs
- src/test/test_action.rs
- src/test/page_stability.rs
- src/test/actionability.rs (reference)

## Tests

- cargo test --lib test::test_action::tests -- failing_action_records_index_type_and_last_unmet_condition_without_script_contents

## Dependencies

- capture-queue-setup-stage

## Risks

- A selector fingerprint/hash scheme that is not stable across otherwise-identical runs (e.g. if it incorporates a random salt) would make it useless for correlating repeated failures of the same selector; mitigate by using a deterministic hash of the selector string so the same selector always produces the same safe identity.
