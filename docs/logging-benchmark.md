# Logging Overhead Benchmark

Measured on Windows on 2026-10-01 with cached managed Chromium and the debug
CLI built by `cargo test`. The user confirmed a **10% maximum mean wall-time
overhead** budget for this workload after reviewing the results. The observed
**5.30%** overhead passes that gate. This is a local workload result, not a
universal performance guarantee.

## Method

- Four suites, one scenario each, two viewports (800 x 600 and 375 x 667): eight
  independent browser captures per run.
- Concurrency five, no launch ramp; loopback HTML with input and click actions,
  viewport screenshots, 5-second element/actionability budgets, and 30-second
  scenario timeout. Capture settling/animation freezing and optional screenshot
  delays are disabled equally in both modes to expose overhead.
- One warmup, then five repetitions per mode, alternating the order. Each sample
  uses a fresh temporary workspace. Every sample requires all eight captures to
  succeed and every enabled journal to verify complete.
- Both modes use `--no-print` and `--skip-compare`. Archive-off means
  `REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE=true`; instrumentation and the existing
  live pipeline are still compiled and execute. This measures incremental durable
  archive cost, not the cost versus a pre-revamp binary.
- Times include process startup, captures, journal finalization, and automatic
  empty snapshot assets. Bytes are regular-file lengths under the output root,
  with `_diagnostics` counted separately. OS write-call counts, filesystem metadata
  writes, cache effects, and physical disk traffic were **not** measured. Record
  counts and file counts are logical I/O proxies, not syscall counts.
- An initial run with a 2-second actionability fixture failed its final capture.
  That entire series was discarded, not treated as a successful timing sample.

## Raw Samples

| Repetition | Archive Off Seconds | Archive On Seconds | Journal Bytes On | Total Bytes Off | Total Bytes On |
| ---------- | ------------------- | ------------------ | ---------------- | --------------- | -------------- |
| 1          | 8.0478992           | 8.3828119          | 463275           | 733752          | 1197035        |
| 2          | 8.8366970           | 8.7628291          | 463271           | 733752          | 1197031        |
| 3          | 8.3804143           | 9.0176111          | 463273           | 733752          | 1197033        |
| 4          | 8.4193785           | 9.0937188          | 463257           | 733752          | 1197017        |
| 5          | 8.4246391           | 9.0825174          | 463263           | 733752          | 1197023        |
| Mean       | 8.42180562          | 8.86789766         | 463267.8         | 733752          | 1197027.8      |

Every enabled sample produced 482 records in 27 journal files: one browser
preparation operation plus eight capture operations, each with metadata, a segment,
and an index. Disabled samples produced zero journal files/bytes.

Wall-time overhead is `(8.86789766 - 8.42180562) / 8.42180562 * 100 = 5.2969%`.
Total output-byte overhead is `(1197027.8 - 733752) / 733752 * 100 = 63.1379%`.
The journal-only percentage versus zero baseline bytes is undefined; the absolute
increase is 463267.8 bytes per eight-capture run, about 57.9 KB per capture when
amortizing browser preparation. Tiny screenshots make the percentage disk increase
large; do not extrapolate it to full-page production images.

## Reproduce

Requires an already cached managed Chromium. The fixture never fetches a browser
or accesses production pages.

```sh
cargo test --locked --features gui --test desktop_worker logging_overhead_benchmark -- --ignored --exact --nocapture
```

The existing [worker integration module](../tests/desktop_worker.rs) owns the
fixture. Optional `REGRESSIFY_LOG_BENCHMARK_OUTPUT` names a local JSON result file;
the test emits the same structured samples to stdout. Do not run other browser
loads concurrently when comparing results. Repeat on CI and representative user
hardware before treating a single-machine result as a capacity estimate.

## Decision And Limits

Keep the 512 KiB segment, 128 MiB per-operation budget, and bounded worker queue.
No field reduction or retention pruning is justified by this passing measurement.
Disk growth remains explicit: logs are retained with source history, and provider
package limits include public log chunks. A slow/unavailable disk can still degrade
diagnostics without changing capture results. If future representative measurements
exceed 10%, examine batching and safe-field volume, and use the documented archive
kill-switch as an operational fallback rather than silently weakening completeness.
