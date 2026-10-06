<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 002 — Add tracing/tracing-subscriber as direct engine dependencies

- Slug: `add-tracing-dependencies`
- Implements plan section(s): `Phase 1 — Contract and Foundations#0`, `Logging Mechanism: Options Considered#0`

## Goal

Add tracing and tracing-subscriber to Cargo.toml as unconditional (non-optional, non-feature-gated) dependencies of the shared engine crate.

## Why this exists

Phase 1 and every later phase need these crates available outside desktop/gui feature gates, per the plan's adopted decision to use tracing + tracing-subscriber layers; Current Evidence confirms neither is a direct dependency today.

## Settled investigation

- Cargo.toml's [dependencies] section (lines 25-75) does not list tracing, tracing-subscriber, or log.
- [features] defines default=["local"], local=[], cloud=[], desktop=[...optional deps via dep:name...], gui=["desktop",...], gui-tests=["gui","gpui-kit/test-support"]; an unconditional dependency added to [dependencies] without optional=true is available under every feature combination including bare default.
- The release profile sets panic = "abort" (Cargo.toml lines 96-98), relevant context (not this task's own scope) for any tracing-appender non-blocking writer added later.

## Inputs

- Cargo.toml at repository HEAD
- The plan's 'Recommended dependencies to evaluate during implementation' list

## Outputs

- An updated Cargo.toml with tracing and tracing-subscriber as unconditional direct dependencies
- Cargo.lock updated accordingly

## Implementation steps

- Add tracing and tracing-subscriber (with minimal initial features, e.g. default features plus env-filter) to Cargo.toml's [dependencies] table, not under optional=true or any feature gate.
- Run cargo build (default features) to regenerate Cargo.lock and confirm the crate compiles with these types available without --features desktop/gui.
- Run cargo build --features desktop and cargo build --features gui to confirm no feature-interaction regressions.

## Done when

- cargo build (default features) succeeds with tracing/tracing-subscriber resolvable from the crate root without any feature flag.
- cargo build --features desktop and cargo build --features gui succeed unchanged.

## Evidence to capture

- cargo build output showing successful compilation
- The diff of Cargo.toml/Cargo.lock

## Do not change

- Do not gate tracing/tracing-subscriber behind optional=true or any feature list — Task 1.3's acceptance criterion explicitly requires them available outside desktop/gui gates.
- Do not add tracing-appender or a log bridge in this task; the plan reserves those for evaluation only if later tasks show they are required.

## Reopen triggers

- Task 2.1's journal writer finds tracing-appender's non-blocking/lossy queue behavior unsuitable and needs a different appender dependency added here instead.
- A dependency-resolution conflict appears between tracing-subscriber's chosen features and an existing pinned crate.

## Context

The plan's Logging Mechanism: Options Considered section adopts tracing + tracing-subscriber as the only option offering structured fields, nested async-aware spans, and independently filtered multi-consumer outputs; Current Evidence already confirms neither crate is a direct dependency today. This task is the prerequisite every other Phase 1-7 Rust task needs before it can use tracing types.

## Options & tradeoffs

println!/eprintln! (status quo) needs no new dependency but lacks structure/attribution (rejected). log + a logger needs less dependency surface but would require custom plumbing for async span context (rejected). tracing+tracing-subscriber is the plan's adopted decision.

## Decision

Add tracing and tracing-subscriber as unconditional direct dependencies in Cargo.toml's base [dependencies] table, matching the plan's explicit decision.

## Acceptance criteria

- cargo build (default features) succeeds and tracing/tracing-subscriber types are importable from a default-feature build, not only --features desktop or --features gui.
- Cargo.lock is regenerated and committed alongside the Cargo.toml change.

## File touchpoints

- Cargo.toml
- Cargo.lock

## Tests

- cargo build
- cargo build --features desktop
- cargo build --features gui

## Dependencies

_none_

## Risks

- Picking subscriber features speculatively now could require a follow-up Cargo.toml edit once Task 1.5 settles the exact layer/formatter needed; mitigate by keeping the initial feature list minimal and letting Task 1.5 add more if needed.
