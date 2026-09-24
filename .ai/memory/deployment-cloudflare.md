# Cloudflare Deployment

Verified: 2026-09-24

- [CloudflareClient](../../src/desktop/deployment/cloudflare.rs) uses Workers Static
  Assets directly: manifest, missing-content Base64 uploads, completion JWT, then
  assets-only Worker publication. Empty buckets skip uploads but not publication.
- [Targets](../../src/desktop/deployment.rs) store `cloudflareWorkers` plus
  `cloudflare.accountId`/`workerName`; tokens use the shared OS credential store.
  R2 Access Key ID and Secret Access Key are not used. An existing workers.dev
  route is required; the deployer does not manage DNS, domains, or Access.
- Workers rejects `/suites/* /index.html 200` with code 100324 (canonical HTML
  redirect loop). The client translates it to `/suites/* / 200` in API metadata,
  leaves Netlify output unchanged, and keeps missing assets as 404s.
- Header/redirect files are API metadata, not public assets. The manifest excludes
  Azure configuration and `.gitignore`. Both Cloudflare and Netlify call the shared
  Azure access-policy guard before contacting their APIs.
- Client limits: 25 MiB per file, 100,000 paths, 2 GiB raw content, 32 MiB manifest,
  8 MiB responses, two 50 MiB raw upload batches, and a 55-minute deadline. The
  provider enforces Free's 20,000-file allowance. Uploads never reduce image quality.
- Verification: `cargo test --features desktop --lib desktop::deployment::cloudflare::tests`
  covers content limits, identity, routing, cached uploads, retries, deadlines,
  and secret redaction. Native deployment-dialog tests cover saved fields and
  provider switching. Symlink tests are conditional on Unix.
- Explicitly authorized live publishing verified all staged report bytes,
  suite deep links, security/cache headers, and missing-file 404s. The identical
  second deployment required no asset uploads. The ignored live test validates
  its expected public URL before publication and must not run by default.

See the [Cloudflare skill](../../.github/skills/deployment-cloudflare/SKILL.md)
for maintained procedures and live-test prerequisites.
