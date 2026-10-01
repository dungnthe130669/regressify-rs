<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 019 — Cancellation and partial-publication handling

- Slug: `deployment-cancellation-partial-publication`
- Implements plan section(s): `Phase 4 — Deployment Instrumentation#0`, `Deployment Diagnostics#0`
- Depends on: deployment-provider-upload-stages

## Goal

Make a cancelled-mid-upload mocked run distinguish local cancellation from unknown/unconfirmed remote state in the resulting diagnostic record.

## Why this exists

Deployment Diagnostics explicitly requires 'Cancellation must distinguish local termination from confirmed remote state' and 'Do not report success merely because upload bytes completed' — this is Task 4.4's own acceptance criterion.

## Settled investigation

- src/desktop/deployment/cloudflare.rs::deploy() already wraps deploy_inner in tokio::time::timeout(self.deadline, ...) (lines 318-320), returning an explicit 'exceeded its deadline; check the Worker's deployment history before retrying' error on timeout — this existing message already models 'we don't know the remote state, check manually.'
- The desktop-worker-lifecycle skill documents that 'Cancellation must terminate and reap descendants' checked 'after launch, payload write, event decoding, child failure, and UI/window closure' — deployment cancellation goes through this same general worker-cancellation path, not a deployment-specific one.
- progress::DeploymentResult::has_safe_url() (src/progress.rs lines 366-379) already encodes the existing safe/unsafe URL contract for a reported deployment URL; this task's unknown-remote-state case must not populate this contract's success path at all when state is unconfirmed.

## Inputs

- Task 4.3's per-provider upload/completion instrumentation
- The existing worker cancellation path (src/desktop/runner.rs, reference) and Cloudflare's existing deadline/timeout pattern (reference)

## Outputs

- A diagnostic outcome type/value that distinguishes 'locally cancelled, remote state unknown' from 'confirmed remote failure' from 'confirmed remote success', attached to the deployment journal

## Implementation steps

- Define an explicit outcome classification (an enum variant on DiagnosticRecordV1's outcome: Option<Outcome> field) with at least confirmed-success, confirmed-failure, and local-cancellation-unknown-remote-state.
- Hook this classification into the existing worker cancellation path so a cancellation that arrives mid-upload (before any provider confirmation) records local-cancellation-unknown-remote-state, never silently defaulting to failure or success.
- Ensure the existing Deployed{url}/has_safe_url() lifecycle event is only emitted for the confirmed-success case.
- Write cancellation_mid_upload_reports_unknown_remote_state: cancel a mocked run mid-upload (after the request is sent but before a response is read, using a mock server that blocks on a controllable gate as cloudflare.rs's own deployment_deadline_stops_waiting_for_provider_headers test does) and assert the resulting outcome is the dedicated unknown-remote-state variant and no Deployed{url} event was emitted.

## Done when

- Test cancellation_mid_upload_reports_unknown_remote_state cancels a mocked deployment after the upload request is sent but before any response is read, and asserts the resulting diagnostic record's outcome equals the dedicated local-cancellation-unknown-remote-state variant, not confirmed-success or confirmed-failure.
- The same test asserts zero Deployed{url} lifecycle events were emitted for that cancelled run.

## Evidence to capture

- The exact cancellation point used in cancellation_mid_upload_reports_unknown_remote_state ('after request sent, before response read') and its asserted outcome variant plus the absence of any Deployed{url} event

## Do not change

- Do not report success merely because upload bytes completed sending — Deployment Diagnostics is explicit that completion of the upload is not the same as confirmed publication.
- Do not repurpose the existing Deployed{url} lifecycle event to carry an 'unknown' state; it remains a success-only signal per has_safe_url()'s existing contract.

## Reopen triggers

- Task 5.5's log-health warning UI finds it needs to render this outcome classification differently from a simple pass/fail, requiring an additional field.

## Context

A deployment that is cancelled mid-upload genuinely does not know whether the provider received and will act on the partial request; reporting this as either success or failure would both be wrong. This task makes 'we don't know' a first-class, explicit outcome rather than an approximation.

## Options & tradeoffs

Treating any cancellation as a failure would be simpler but could mask an actually-successful publication the user cancelled out of impatience, misleading a later retry into double-publishing; treating it as unconfirmed (chosen) costs one more enum variant but matches the plan's explicit requirement to distinguish local termination from confirmed remote state.

## Decision

Add a dedicated local-cancellation-unknown-remote-state outcome variant, distinct from confirmed success/failure, and gate the existing Deployed{url} event on confirmed success only.

## Acceptance criteria

- Test cancellation_mid_upload_reports_unknown_remote_state's diagnostic record outcome is the dedicated unknown-remote-state variant, not success or failure.
- The same test confirms no Deployed{url} lifecycle event was emitted for that cancelled run.

## File touchpoints

- src/desktop/deployment.rs
- src/desktop/deployment/cloudflare.rs (reference)
- src/progress.rs (reference)

## Tests

- cargo test --features desktop --lib desktop::deployment::tests -- cancellation_mid_upload_reports_unknown_remote_state

## Dependencies

- deployment-provider-upload-stages

## Risks

- Simulating 'cancelled after request sent, before response read' reliably in a mocked-HTTP test can be timing-sensitive; mitigate by using a mock server that blocks on a controllable gate (matching cloudflare.rs's own deployment_deadline_stops_waiting_for_provider_headers test, line 2489) rather than a real sleep-based race.
