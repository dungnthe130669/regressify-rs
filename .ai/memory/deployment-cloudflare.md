# Cloudflare Deployment

Verified: 2026-09-28

- [CloudflareClient](../../src/desktop/deployment/cloudflare.rs) uses Workers Static
  Assets directly: MIME-aware manifest, missing-content raw/Base64 uploads,
  completion JWT, then assets-only Worker publication. Empty buckets skip uploads
  but not publication. Hashes include Base64 bytes, extension, NUL, and MIME.
- [Targets](../../src/desktop/deployment.rs) store `cloudflareWorkers` plus
  `cloudflare.accountId`/`workerName`; tokens use the shared OS credential store.
  R2 keys are not used. Missing Workers are created; new and legacy targets default
  to public production. `workersDev: false` disables public/preview routes; explicit
  `null` preserves existing routing and creates missing Workers privately. DNS,
  custom domains, and Access remain unmanaged. Account subdomains must exist.
- Workers rejects `/suites/* /index.html 200` with code 100324 (canonical HTML
  redirect loop). The client translates it to `/suites/* / 200` in API metadata,
  leaves Netlify output unchanged, and keeps missing assets as 404s by default.
  With `htmlHandling: none`, retain the `index.html` proxy and verify that path.
- Header/redirect files are API metadata, not public assets. The manifest excludes
  Azure configuration and `.gitignore`. Both Cloudflare and Netlify call the shared
  Azure access-policy guard before contacting their APIs.
- Client limits: 25 MiB per file, 100,000 paths, 2 GiB raw content, 32 MiB manifest,
  8 MiB responses, a shared 50 MiB raw upload budget, and a 55-minute deadline. The
  provider enforces Free's 20,000-file allowance. Uploads never reduce image quality.
  JWT hints select raw per-hash uploads (up to eight concurrent) or multipart
  (three). Size-based upload timeouts cap at 30 minutes. Permits cover reads/retries.
- Saved targets retain validated HTML/not-found modes, compatibility date, and
  exclusion globs. Local preparation rejects unsupported Pages code/ignore files
  and prunes private files. It precedes API calls. Session and route updates retry;
  publication never retries and distinguishes rejection from uncertain outcomes.
- Verification: `cargo test --features desktop --lib desktop::deployment::cloudflare::tests`
  covers content limits, identity, routing, cached uploads, retries, deadlines,
  and secret redaction. Native deployment-dialog tests cover saved fields and
  provider switching. Symlink tests are conditional on Unix.
- Explicitly authorized live publishing verified all staged report bytes,
  suite deep links, security/cache headers, and missing-file 404s. The identical
  second deployment required no asset uploads. The ignored live test validates
  its expected public URL before publication and must not run by default.
- The guarded temporary-Worker test passed creation, public production routing,
  changed-content update, cached redeployment, exact public bytes/MIME (including
  WebVTT and unknown-header omission), and confirmed deletion. It never targets
  a configured Worker and cleans up after verification errors.

See the [Cloudflare skill](../../.github/skills/deployment-cloudflare/SKILL.md)
for maintained procedures and live-test prerequisites.
