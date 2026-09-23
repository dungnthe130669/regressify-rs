# Netlify Uploads

Verified: 2026-09-23

- [NetlifyClient](../../src/desktop/deployment/netlify.rs) explicitly overrides the
  shared client's 180-second HTTP timeout for ZIP uploads with the one-hour
  deployment budget. The outer timeout also bounds packaging and publication;
  connection/read limits remain 10/30 seconds.
- The upload stall monitor compares byte counts every 60 seconds. Unchanged
  samples emit repeated Activity/View Log warnings, without canceling the request.
  Changed samples reset the warning timer; normal progress reporting continues.
- Sent bytes measure local body consumption, not provider acknowledgement. A
  fully sent ZIP can still be awaiting acceptance. Stop cancels only the local
  worker; check Netlify's history before retrying an ambiguous or canceled upload.
- Verification: `cargo test --features desktop --lib desktop::deployment::netlify::tests`.
  The slow-response regression failed before the timeout fix; it also covers
  deadline exhaustion without retries. Synthetic samples cover warning boundaries,
  repeated warnings, resumed transfers, and acceptance waits.

See the [deployment skill](../../.github/skills/deployment-netlify/SKILL.md) and
[desktop docs](../../docs/desktop.md) for the maintained workflow.
