# Desktop Git

Verified: 2026-09-24

- [The Git model](../../src/desktop/git.rs) stores active remote selection in
  repository-local `regressify.remote`. Its configuration hash covers that key,
  remotes, and URL rewrites; branch tracking is checked separately. Removing the
  active remote deliberately retains its missing selection to prevent fallback.
- Remote form drafts in [the UI](../../src/desktop/ui/git.rs) keep the opening
  snapshot. A newer periodic status must not silently authorize an old URL edit.
  Named remotes support add/select/update/rename/remove; fetch and push URL edits
  preserve additional configured URLs.
- Snapshot URL details contain only validated credential-free URLs; unsafe
  existing values appear as hidden rather than becoming form defaults. Selected
  remote counts use the matching remote-tracking branch, not another upstream.
- [The process wrapper](../../src/desktop/git/process.rs) streams only diagnostic
  commands through bounded reporting; config/status/diff payloads are excluded.
  CR/LF framing happens before redaction. Authorization/oversized lines suppress
  the remaining stream, while pipes continue draining under cancellation/deadlines.
- [Native tests](../../src/desktop/ui/git/tests.rs) use rendered popup items,
  guarded confirmations, local bare remotes, and compact/wide layout assertions.
  `desktop::git::tests::remote_`, `desktop::git::process::tests`, and
  `desktop::ui::git::tests::remote_` passed with their matching feature gates.
- Native `./ci/desktop-smoke.ps1 -GitFlows -SkipBuild` passed against a current
  binary, using local repositories and light/dark compact/wide screenshots.
  Activity belongs below the diff pane: placing it below both panes collapsed
  comparison rows at 1080x720. The layout test now includes those rows and clicks
  a comparison checkbox with activity expanded. Fixture commits made immediately
  before opening a remote form go through the UI to avoid stale-HEAD races.
- Workflow and native-smoke prerequisites live in the
  [Git skill](../../.github/skills/desktop-git-review/SKILL.md) and
  [desktop docs](../../docs/desktop.md), not in this note.
