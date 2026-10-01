<!-- istari:plan-review generated file — safe to delete -->
<!-- istari:plan-review owner:0b66699e22aad8678758c61eed6abb2d7b50b4662c942041beee20cc459c70c2 -->
# Task 018 — API/upload, retry/polling, and completion/cleanup stages per provider

- Slug: `deployment-provider-upload-stages`
- Implements plan section(s): `Phase 4 — Deployment Instrumentation#0`, `Deployment Diagnostics#0`
- Depends on: deployment-journal-reuse

## Goal

Instrument API/upload, retry/polling, and completion/cleanup stages for each of the three providers (Azure, Netlify, Cloudflare), so a mocked transient-error-then-success run shows retry attempts, backoff reason, and a final confirmed-publication record with no secrets/tokens/signed URLs in captured output.

## Why this exists

Deployment Diagnostics' required-context table and Task 4.3's own acceptance criterion both require this, per-provider, as the highest-value diagnostic surface for deployment failures today.

## Settled investigation

- Each provider's deploy() entry point is confirmed: swa.rs::deploy (line 347), netlify.rs::deploy (line 148), cloudflare.rs::deploy (line 316) wrapping a private deploy_inner (line 323) inside tokio::time::timeout(self.deadline, ...).
- swa.rs already has a passing test named deployment_protocol_retries_upload_and_never_sends_token_to_blob (line 607) and netlify.rs has retry-budget handling ('Netlify request retry budget exhausted', line 145) — both existing retry mechanisms this task instruments around.
- src/desktop/deployment/diagnostics.rs::http_error() (lines 14-63) already extracts request_id/code from provider HTTP responses and calls failure()/sanitize() — this is the existing per-HTTP-call error-construction path this task attaches diagnostic records to.
- The deployment-packaging skill confirms Azure uploads a ZIP, Netlify uploads via a full SHA-1 manifest, and Cloudflare uses content/extension/MIME hashes and raw single-asset or Base64 multipart uploads — genuinely different upload shapes per provider.
- This task resolves a naming inconsistency between its own Acceptance Criteria and Tests sections for the Azure/swa provider: rather than authoring a new, separately-named swa_retry_then_success_records_attempts test, this task extends the existing, already-passing swa.rs test deployment_protocol_retries_upload_and_never_sends_token_to_blob (line 607) in place with new diagnostic-record assertions. Netlify and Cloudflare have no equivalent pre-existing named test, so this task authors netlify_retry_then_success_records_attempts and cloudflare_retry_then_success_records_attempts as genuinely new tests for those two providers only.

## Inputs

- Task 4.2's preflight/staging/packaging instrumentation
- Each provider's existing deploy()/retry/error-handling code (reference)
- src/desktop/deployment/diagnostics.rs::http_error() (reused)

## Outputs

- API/upload, retry/polling, and completion/cleanup diagnostic records for each of Azure, Netlify, and Cloudflare

## Implementation steps

- For each provider module (swa.rs, netlify.rs, cloudflare.rs), add a diagnostic record around each outbound HTTP call recording operation name, a safe endpoint category, HTTP status, safe provider error code/request ID (reusing http_error()'s existing extraction), batch/file counts, bytes, and attempt duration.
- Add a retry/polling diagnostic record at each provider's existing retry loop recording attempt number, retry reason/backoff, provider state transitions, elapsed/deadline time, and a periodic no-progress heartbeat for long polls.
- Add a completion/cleanup diagnostic record at each provider's successful-publication return path, recording confirmed publication state and a validated public URL plus cleanup/cancellation status.
- For Azure, extend the existing swa.rs test deployment_protocol_retries_upload_and_never_sends_token_to_blob in place — reusing its existing mocked transient-error-then-success setup — by adding new assertions for retry-attempt count, backoff reason, and a final confirmed-publication diagnostic record, without removing or weakening its existing token-absence assertion. For Netlify and Cloudflare, author new tests netlify_retry_then_success_records_attempts and cloudflare_retry_then_success_records_attempts (reusing each provider's own existing mock-HTTP test infrastructure) asserting the same three properties. All three must show zero occurrences of any injected synthetic secret/token/signed-URL fragment.

## Done when

- For Azure (swa.rs), the existing, now-extended test deployment_protocol_retries_upload_and_never_sends_token_to_blob shows at least 2 retry attempts, a recorded backoff reason, and a final confirmed-publication record, in addition to its original token-absence assertion (which must still pass unchanged).
- For Netlify (netlify.rs) and Cloudflare (cloudflare.rs), new tests netlify_retry_then_success_records_attempts and cloudflare_retry_then_success_records_attempts each show the same three properties.
- None of the three tests' captured diagnostic output contains the test's injected synthetic secret/token/signed-URL fragment.

## Evidence to capture

- Each of the three per-provider test names (deployment_protocol_retries_upload_and_never_sends_token_to_blob for Azure, extended in place; netlify_retry_then_success_records_attempts; cloudflare_retry_then_success_records_attempts) and their pass/fail result, plus the exact injected synthetic secret string and a confirmation scan showing its absence from captured output in all three
- Confirmation that Azure's original token-absence assertion in deployment_protocol_retries_upload_and_never_sends_token_to_blob still passes unchanged after this task's new assertions are added

## Do not change

- Do not re-implement http_error()/failure()/sanitize()'s existing redaction; attach diagnostic records around their calls, not alternative text construction.
- Do not unify the three providers' upload-stage fields into one shared code path that hides their genuinely different upload shapes — Deployment Diagnostics explicitly requires keeping Azure/Netlify/Cloudflare stage detail distinct.
- Do not remove or weaken deployment_protocol_retries_upload_and_never_sends_token_to_blob's existing token-leak assertion while extending it with new retry/backoff/publication assertions — both old and new assertions must pass in the same test.
- Do not author a second, separately-named Azure test (e.g. swa_retry_then_success_records_attempts) alongside the extended existing one — Azure has exactly one test for this scenario, extended in place.

## Reopen triggers

- Task 4.4's cancellation work finds this task's completion/cleanup records insufficient to distinguish confirmed-remote-state from unknown-remote-state.

## Context

Today, a deployment failure 'surfaces only as terminal stderr output, with no durable, shareable record for support or post-mortem'; this task is the highest-value instrumentation point for that exact complaint, covering the three real providers this codebase supports. For Azure specifically, this task extends the existing deployment_protocol_retries_upload_and_never_sends_token_to_blob test in place rather than introducing a second, differently-named test for the same scenario, keeping one authoritative Azure retry-then-success test instead of two overlapping, inconsistently-named ones.

## Options & tradeoffs

A single shared 'HTTP call' instrumentation wrapper used identically by all three providers would be less code, but Deployment Diagnostics explicitly requires keeping Azure ZIP/upload/status, Netlify manifest/missing-file/upload retries, and Cloudflare session/bucket/upload/final publication stages distinct; per-provider instrumentation (chosen) costs more code but preserves that required distinction.

## Decision

Instrument each provider's existing upload/retry/completion call sites individually, reusing the shared diagnostics.rs error-construction helpers but keeping provider-specific stage detail explicit per Deployment Diagnostics. For Azure, extend the existing deployment_protocol_retries_upload_and_never_sends_token_to_blob test in place rather than authoring a second, separately-named test for the same mocked scenario; for Netlify and Cloudflare, author new tests since neither has a pre-existing equivalent.

## Acceptance criteria

- The existing, now-extended Azure/swa.rs test deployment_protocol_retries_upload_and_never_sends_token_to_blob, plus new tests netlify_retry_then_success_records_attempts and cloudflare_retry_then_success_records_attempts, each show at least 2 retry attempts, a backoff reason, and a final confirmed-publication diagnostic record.
- None of the three tests' captured diagnostic output contains the test's injected synthetic secret string (e.g. a literal 'synthetic-secret-token-12345' value planted in the mock's auth header), and Azure's original token-absence assertion in deployment_protocol_retries_upload_and_never_sends_token_to_blob still passes unchanged.

## File touchpoints

- src/desktop/deployment/swa.rs
- src/desktop/deployment/netlify.rs
- src/desktop/deployment/cloudflare.rs
- src/desktop/deployment/diagnostics.rs (reference)
- src/progress.rs (reference)

## Tests

- cargo test --features desktop --lib desktop::deployment::swa::tests -- deployment_protocol_retries_upload_and_never_sends_token_to_blob
- cargo test --features desktop --lib desktop::deployment::netlify::tests -- netlify_retry_then_success_records_attempts
- cargo test --features desktop --lib desktop::deployment::cloudflare::tests -- cloudflare_retry_then_success_records_attempts

## Dependencies

- deployment-journal-reuse

## Risks

- Provider mock-HTTP test infrastructure differs enough between the three modules (ZIP vs. manifest vs. multipart) that a single shared assertion helper may not fit all three; mitigate by writing one assertion helper per provider rather than forcing a common one prematurely.
- Extending deployment_protocol_retries_upload_and_never_sends_token_to_blob in place risks accidentally removing or weakening its existing token-leak assertion while adding new ones; mitigate by diffing the test's assertion list before/after this task's change and confirming the original token-absence assertion is still present and unmodified.
