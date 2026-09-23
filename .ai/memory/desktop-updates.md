# Shared Desktop Updates

Verified: 2026-09-23

- [UpdateFeed::check_shared](../../src/desktop/updates.rs) uses the per-user cache
  and persisted `last_check_at` to make automatic network checks eight hours
  apart. Startup and five-second local polls share that deadline; manual checks
  bypass it only after acquiring ownership. Old state files default to due.
- `UpdateLease` holds `coordinator.lock` before any network check and across
  preparation. Inner `update.lock` operations preserve existing state/hash guards.
  A peer returns Busy without a feed request. Dropping or exiting the owner
  releases the OS lock; another process can reuse metadata and resume partials.
- [UpdatePanel](../../src/desktop/ui/updates.rs) owns a window-scoped cache watcher.
  Each window verifies the completed local installer before displaying the same
  ready banner, without another transfer. Existing banners survive idle polls.
- Explicit pause and preparation errors defer automatic work eight hours without
  consuming checksum retries. Owner shutdown uses `cancel_for_handoff` to permit
  takeover, without overriding an already requested pause. Clear expired local
  retry timers when a peer owns the cycle; rescheduling a due timer can spin.
- Checks: `cargo test --features desktop --lib desktop::updates::tests` and
  `cargo test --features gui-tests --lib desktop::ui::updates::tests -- --test-threads=1`.
  Fixtures cover fake-clock scheduling, child-process contention/release,
  one-transfer reuse, takeover/pause, and two windows showing the ready banner.
  Tests do not download or install a real update.

See the [updater skill](../../.github/skills/desktop-updates/SKILL.md) and
[desktop docs](../../docs/desktop.md) for contracts and user-visible behavior.
