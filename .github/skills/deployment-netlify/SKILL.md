---
name: deployment-netlify
description: 'Use when changing Netlify report deployment, site validation, uploads, deployment status polling, safe published URLs, retries, or provider diagnostics.'
---

# Netlify Deployment

## Start Here

[Netlify client](../../../src/desktop/deployment/netlify.rs),
[shared diagnostics](../../../src/desktop/deployment/diagnostics.rs), and
[target orchestration](../../../src/desktop/deployment.rs).

## Procedure and Invariants

1. Validate the intended site/target before upload. Keep credentials in sensitive
   headers and preserve the production endpoint/redirect policy.
1. Bound response bodies, request deadlines, transient retries, and status polling.
   Treat upload acceptance and completed publication as different states.
1. Classify terminal provider failures versus retryable states. Preserve useful
   request/status context without echoing tokens, signed URLs, or raw response bodies.
1. Validate published URLs before storing or opening them. Cancellation and failed
   publication must not create a misleading success link in saved run activity.
1. Keep provider file/size restrictions aligned with staging and package validation.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::netlify::tests
cargo test --features desktop --lib desktop::deployment::diagnostics::tests
```

Use loopback/mock servers and synthetic tokens, never a live site. Cover malformed
JSON, oversized responses, invalid URLs, auth errors, throttling, failed publication,
retry/deadline exhaustion, and secret-free diagnostics.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill when provider states
or target requirements change. Explain retry and credential-forwarding decisions.
