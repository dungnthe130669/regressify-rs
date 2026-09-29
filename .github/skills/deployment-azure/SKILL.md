---
name: deployment-azure
description: 'Use when changing Azure Static Web Apps deployment, signed blob uploads, deployment tokens, environment/status polling, or Azure response validation.'
---

# Azure Static Web Apps Deployment

## Start Here

[Azure client](../../../src/desktop/deployment/swa.rs), [shared diagnostics](../../../src/desktop/deployment/diagnostics.rs), and [package generation](../../../src/desktop/deployment/package.rs). Use [shared image optimization](../../../src/desktop/deployment/optimization.rs) for reports over the detected tier's storage budget.

## Procedure and Invariants

1. Preserve the provider's token/endpoint validation and staged upload/finalize sequence. Validate response-supplied upload destinations before sending content.
1. Keep tier detection in the Azure client: Free/unknown gets 250 MiB, Standard gets 500 MiB of uncompressed content. Optimize through the shared helper before ZIP packaging, then check the final package content size again. Fitting reports and original captures remain unchanged.
1. Keep signed blob URLs out of messages and persisted public URLs. Validate scheme/host and intended destination; do not blindly follow credentialed redirects.
1. Bound request/response sizes, total deployment time, polling, and retries. Surface terminal deployment failures rather than waiting indefinitely.
1. Distinguish transport, provider, package, and publication failures. Use the shared secret-redacting diagnostics while retaining safe request IDs/status.
1. Do not report success or open a destination until publication is verified and the returned public URL passes validation.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::swa::tests
cargo test --features desktop --lib desktop::deployment::optimization::tests
cargo test --features desktop --lib desktop::deployment::diagnostics::tests
```

Use local fixtures with synthetic deployment tokens and signed URL values. Cover untrusted destinations, invalid JSON, failed/oversized uploads, provider failures, polling exhaustion, and redaction. Live deployment requires explicit authorization.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill when endpoint or workflow contracts change. Comment non-obvious protocol and security decisions.
