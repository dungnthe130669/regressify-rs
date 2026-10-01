<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 013 — Navigation stage capture instrumentation

- Slug: `capture-navigation-stage`
- Implements plan section(s): `Phase 3 — Capture Instrumentation#0`, `Capture Diagnostics#0`
- Depends on: capture-queue-setup-stage

## Goal

Instrument the navigation stage to record sanitized destination identity, redirect/navigation milestones, observed response status when available, waits, and timeout cause.

## Why this exists

Capture Diagnostics' table requires this stage; Task 3.2's own acceptance criterion requires a navigation-timeout fixture to identify navigation as the failing stage with a timeout cause and no raw URL credentials present.

## Settled investigation

- src/test/test_action.rs's Goto action handler (lines 332-343) already calls crate::progress::scenario_log!("{} Navigating to URL: {}", log_prefix, target_url) (line 334) before actionability::cdp_with_timeout(...) (lines 335-337) — this existing call site logs the RAW target_url (post URL-replacement, potentially containing credentials/query values); this task's sanitized identity must NOT reuse this raw string verbatim.
- actionability::cdp_with_timeout (src/test/actionability.rs line 42+) is the existing generic timeout wrapper already used for the Goto action's navigation wait; this task's timeout-cause field should derive from this same wrapper's existing timeout error.
- config::replacements::apply_replacements (src/config/replacements.rs lines 71+) is applied to the Goto action's URL before navigation (test_action.rs calls apply_action_url_replacements, line 333); this task's sanitized identity must be derived after replacement but before logging.

## Inputs

- Task 3.1's queue/setup stage instrumentation
- src/test/test_action.rs's existing Goto handler and src/test/actionability.rs's cdp_with_timeout (reference)

## Outputs

- Navigation diagnostic records with sanitized destination identity, redirect/navigation milestones, response status when available, waits, and timeout cause

## Implementation steps

- Convert test_action.rs's Goto handler's scenario_log! call (line 334) to a tracing span/event that records a sanitized destination identity (scheme+host+path with query/fragment stripped or redacted) instead of the raw target_url.
- Record navigation start/redirect/completion milestones and, when chromiumoxide's CDP response is available, its observed HTTP status.
- Record the specific timeout cause when actionability::cdp_with_timeout's existing timeout path (src/test/actionability.rs) is hit for this action.
- Write navigation_timeout_identifies_stage_and_cause_without_credentials: configure a Goto action whose target URL includes a userinfo credential and a query-string secret, force a navigation timeout, and assert the resulting record set identifies navigation as the failing stage with an explicit timeout cause, with neither secret present anywhere in output.

## Done when

- Test navigation_timeout_identifies_stage_and_cause_without_credentials produces a record identifying the navigation stage as the failure point with an explicit timeout cause.
- The same test's full captured record set contains zero occurrences of the injected userinfo credential or query-string secret.

## Evidence to capture

- The exact injected credential/secret values used in navigation_timeout_identifies_stage_and_cause_without_credentials and a confirmation scan showing their absence from every captured record

## Do not change

- Do not log the raw (post-replacement) target_url as the sanitized destination identity; it must be stripped/redacted of credentials and sensitive query/fragment values first, per Privacy and Safety.
- Do not alter actionability::cdp_with_timeout's existing timeout behavior; only attach diagnostic recording to its existing error path.

## Reopen triggers

- Task 3.3's actions/frames instrumentation finds it needs the same sanitized-destination-identity helper this task introduces, motivating extracting it to a shared location.

## Context

Navigation is the first browser-driven stage after queue/setup and the one most likely to leak a credential or signed URL into a log today, since the existing scenario_log! call at test_action.rs:334 logs the raw post-replacement URL; this task must close that specific gap while still giving a diagnosable timeout cause.

## Options & tradeoffs

Logging the full raw URL (status quo) is simplest and most diagnosable but is exactly the credential-leak risk Privacy and Safety warns against; a sanitized identity (scheme+host+path only, chosen) is less complete but matches the plan's explicit requirement to omit credentials and sensitive query/fragment values.

## Decision

Derive a sanitized destination identity (scheme+host+path, credentials/query/fragment stripped) at the Goto action's existing call site, replacing its raw-URL scenario_log! call.

## Acceptance criteria

- A navigation-timeout fixture test identifies navigation as the failing stage with an explicit timeout cause.
- The same fixture's full captured output contains no raw URL credentials or query-string secrets, verified by an explicit absence scan for the injected synthetic values.

## File touchpoints

- src/test/test_action.rs
- src/test/actionability.rs (reference)
- src/config/replacements.rs (reference)

## Tests

- cargo test --lib test::test_action::tests -- navigation_timeout_identifies_stage_and_cause_without_credentials

## Dependencies

- capture-queue-setup-stage

## Risks

- Stripping too little from the destination identity (e.g. keeping query parameters) could still leak a signed-URL-style secret embedded in a path segment; mitigate by reusing the same sanitize()-style regex patterns already proven in src/desktop/deployment/diagnostics.rs for authorization/secret-shaped text, applied to the path too.
