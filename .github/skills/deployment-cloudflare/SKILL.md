---
name: deployment-cloudflare
description: 'Use when changing Cloudflare Workers report deployment, static asset manifests, upload-session JWTs, multipart batches, workers.dev routing, or Cloudflare API diagnostics.'
---

# Cloudflare Workers Deployment

## Start Here

[Cloudflare client](../../../src/desktop/deployment/cloudflare.rs),
[target model and staging](../../../src/desktop/deployment.rs), and
[native deployment dialog](../../../src/desktop/ui/deployment.rs).

## Procedure and Invariants

1. Use Workers Static Assets, not R2, Pages, or a Wrangler subprocess. Require an
   existing dedicated Worker with its workers.dev route enabled. Validate Account
   ID and Worker name before building API paths. Do not create Workers or change
   route, DNS, domain, or Access configuration as a side effect.
1. Keep the API token in the shared credential store. Use account-scoped Workers
   Scripts Write permission; R2 access keys are not deployment credentials. Never
   persist or log API tokens, upload JWTs, completion JWTs, or credentialed URLs.
1. Stage only the selected report dependencies. Reject Azure access restrictions
   through the shared guard before API calls. Use the existing artifact checks,
   reject symlinks/non-regular files, and revalidate bytes before uploading.
1. Hash Base64 content plus the extension with SHA-256, taking 32 hexadecimal
   characters. Deduplicate identical hash/extension content. Validate every returned
   bucket hash against the manifest; remote hashes must never choose local paths.
1. Enforce 25 MiB per asset, 100,000 paths, 2 GiB client content, 32 MiB manifests,
   and 8 MiB responses. Cloudflare enforces the account allowance (20,000 Free or
   100,000 Paid). Limit raw batches to 50 MiB and two concurrent uploads.
1. Send MIME types on each Base64 multipart part, authorized with the upload JWT.
   An empty bucket list already contains a completion token. Otherwise require a
   completion token after all requested content is accepted.
1. Publish an assets-only Worker using metadata and the completion token. Exclude
   `_headers`, `_redirects`, Azure configuration, and `.gitignore` from public
   assets. Supply policies in `assets.config`, translate index-page proxies to `/`,
   and keep `not_found_handling: none` so missing JSON/images return 404.
1. Bound the operation to 55 minutes, below one-hour temporary-token expiry. Retry
   safe GETs and identical upload bytes up to six times with bounded backoff.
   Never automatically retry session creation or publication. Cancellation is
   local and cannot undo an already accepted deployment.
1. Verify returned Worker identity and public HTTPS HTML/asset bytes and MIME
   types before emitting success. Never send authorization to the public URL.
   This version requires public workers.dev verification; do not bypass Access.
1. Redact every known request/body token before truncating diagnostics; retain
   bounded provider messages, numeric codes, and safe cf-ray IDs. Do not attach
   raw reqwest errors or response bodies to error chains.

## Checks

```sh
cargo test --features desktop --lib desktop::deployment::cloudflare::tests
cargo test --features desktop --lib desktop::deployment::tests
cargo test --features gui-tests --lib desktop::ui::deployment::tests -- --test-threads=1
```

Use synthetic tokens and loopback servers. Cover manifest limits, mutation,
symlinks, bucket validation, cached deployments, MIME/policy metadata, retry
exhaustion, deadlines, redaction, rejected redirects, and failed publication.

`live_publish_cloudflare_reports` is ignored and replaces the selected Worker's
content twice. Run only with explicit authorization and private process variables:
`CLOUDFLARE_API_TOKEN`, `CLOUDFLARE_ACCOUNT_ID`, `CLOUDFLARE_WORKER_NAME`,
`CLOUDFLARE_EXPECTED_URL`, `CLOUDFLARE_REPORT_ROOT`, and comma-separated
`CLOUDFLARE_SUITE_IDS`. It regenerates snapshots from existing comparisons and
checks every public asset, deep links, headers, and missing-file 404s. Load local
environment files privately, for example with Bun; never print their contents.

## Maintain

Update [desktop docs](../../../docs/desktop.md), [hosting docs](../../../frontend/README.md),
and [shared notes](../../../.ai/memory/deployment-cloudflare.md) with verified
protocol or setup changes. Live publishing tests must remain explicitly ignored.
