---
name: deployment-netlify
description: 'Use when changing Netlify report deployment, site validation, uploads, deployment status polling, safe published URLs, retries, or provider diagnostics.'
---

# Netlify Deployment

## Start Here

[Netlify client](../../../src/desktop/deployment/netlify.rs), [shared diagnostics](../../../src/desktop/deployment/diagnostics.rs), and [target orchestration](../../../src/desktop/deployment.rs).

## Procedure and Invariants

1. Validate the intended site/target before upload. Keep credentials in sensitive headers and preserve the production endpoint/redirect policy.
1. Bound response bodies, request deadlines, transient retries, and status polling. Treat upload acceptance and completed publication as different states.
1. Create an asynchronous file-digest deployment with every staged path and its SHA-1. Validate the deploy/site identity and required digest list before uploading. Poll preparing states; upload only requested, known hashes, deduplicating identical content across paths. Wait for ready before reporting success, including when nothing needs uploading.
1. Bound file uploads to four concurrent requests. Retry idempotent file PUTs only for transport/transient HTTP failures, with fresh body streams and bounded backoff/Retry-After. Never retry deployment creation automatically. Revalidate regular-file paths and content; do not retry changed or unreadable staged files as network failures.
1. Keep file request timeouts aligned with the one-hour deployment deadline; retain 10-second connections and 30-second manifest/read timeouts. Sample each active file's sent bytes every 60 seconds and warn without canceling or retrying merely because progress stopped. Distinguish stalled transfers from fully sent files awaiting acceptance. Stop only cancels the local worker, not any deployment already accepted by Netlify.
1. Classify terminal provider failures versus retryable states. Preserve useful request/status context without echoing tokens, signed URLs, or raw response bodies.
1. Validate published URLs before storing or opening them. Cancellation and failed publication must not create a misleading success link in saved run activity.
1. Preserve client limits of 25,000 paths, 2 GiB staged content, 16 MiB manifests, and 2 MiB API responses. Encode each upload URL path segment and never let remote digests choose local paths or credential destinations. Azure still uses ZIP packages; Netlify does not.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::netlify::tests
cargo test --features desktop --lib desktop::deployment::diagnostics::tests
```

Use loopback/mock servers and synthetic tokens by default. Cover async preparation, all-cached/partially cached content, duplicate digests, empty files, encoded paths, changed files, symlinks, bounded concurrency, malformed JSON, oversized responses, unknown hashes, wrong deploy/site IDs, unsafe URLs, auth errors, throttling, failed publication, slow file PUTs, retry/deadline exhaustion, stalled/resumed transfers, and redaction. Keep timing tests short with injected deadlines and synthetic progress samples; Unix symlink coverage is conditional.

Live checks require explicit authorization. `live_deployment_` filters select preflight checks only. The ignored `live_publish_netlify_digest` test publishes twice to `NETLIFY_SITE_ID` using `NETLIFY_PAT`, replaces current site content, and verifies the page/asset. Use a dedicated test site and load credentials into the process privately; never print environment-file contents or tokens. Do not add publishing tests under the preflight filter.

## Maintain

Update [desktop docs](../../../docs/desktop.md) and this skill when provider states or target requirements change. Explain retry and credential-forwarding decisions.
