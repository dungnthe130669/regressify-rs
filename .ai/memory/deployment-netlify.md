# Netlify Uploads

Verified: 2026-09-23

- [NetlifyClient](../../src/desktop/deployment/netlify.rs) uses asynchronous SHA-1
  file manifests, not ZIP uploads. It uploads only provider-requested known hashes,
  once per distinct content, with up to four file PUTs in flight. Publication still
  waits for the same site's deployment to become ready, including all-cached runs.
- The one-hour deadline covers hashing, transfers, retries, and publication. Each
  file request overrides the shared 180-second HTTP timeout; connection and
  manifest/read timeouts remain 10 and 30 seconds. Client bounds: 25,000 paths,
  2 GiB staged content, 16 MiB manifest, 2 MiB response body.
- File PUTs can retry transient errors within the same deploy; manifest POSTs
  cannot safely be retried automatically. Reqwest can wrap stream read failures
  as transport errors rather than `is_body()`, so an explicit read-failure flag
  prevents retries of changed or unreadable staged files.
- Safe GET retries include reading successful response bodies. In v0.29.0 the
  early return from `read_json` skipped retries after headers arrived. A typed
  `ResponseBodyError` now distinguishes timeout/interruption without retaining
  raw URLs or content; malformed JSON and oversized bodies remain non-retryable.
  `read_requests_retry_interrupted_response_bodies` covers recovery and six-attempt
  exhaustion for both truncated and stalled responses using loopback sockets.
- Each active file is sampled every 60 seconds. Unchanged counts warn without
  canceling. Sent bytes measure local body consumption, not provider acceptance;
  accepted file/byte totals exclude retries. Stop cancels only the local worker.
- Verification: `cargo test --features desktop --lib desktop::deployment::netlify::tests`
  covers reuse, empty/encoded files, retries, boundaries, mutation rejection,
  concurrency, identity checks, and stall warnings. Unix symlink tests are conditional.
- An explicitly authorized live check verified publication and fetched HTML/CSS;
  the second deployment reused all content. `live_publish_netlify_digest` is an
  ignored publishing test and must stay outside the read-only `live_deployment_`
  filter. It replaces site content and uses process credentials; do not log them.

See the [deployment skill](../../.github/skills/deployment-netlify/SKILL.md) and
[desktop docs](../../docs/desktop.md) for the maintained workflow.
