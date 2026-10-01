# Logging Overhead Benchmark

Measured on Windows on 2026-10-01 with cached managed Chromium and the debug
CLI built by `cargo test`. The user confirmed a **10% maximum mean wall-time
overhead** budget for this workload after reviewing the results. The observed
**7.48%** overhead passes that gate. This is a local workload result, not a
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
- Separate startup journals in local application data are outside the output-byte
  inventory. Their creation/finalization is included in the process wall-time
  measurement; do not interpret output bytes as all writes made by the process.
- An initial run with a 2-second actionability fixture failed its final capture.
  That entire series was discarded, not treated as a successful timing sample.

## Raw Samples

| Repetition | Archive Off Seconds | Archive On Seconds | Journal Bytes On | Total Bytes Off | Total Bytes On |
| ---------- | ------------------- | ------------------ | ---------------- | --------------- | -------------- |
| 1          | 9.0797521           | 9.6155380          | 463265           | 733752          | 1197025        |
| 2          | 9.0602044           | 9.0730624          | 463285           | 733752          | 1197045        |
| 3          | 9.5347556           | 10.3410926         | 463296           | 733752          | 1197056        |
| 4          | 8.9232884           | 9.8837094          | 463287           | 733752          | 1197047        |
| 5          | 8.9536431           | 10.0441115         | 463277           | 733752          | 1197037        |
| Mean       | 9.11032872          | 9.79150278         | 463282           | 733752          | 1197042        |

Every enabled sample produced 482 records in 27 journal files: one browser
preparation operation plus eight capture operations, each with metadata, a segment,
and an index. Disabled samples produced zero journal files/bytes.

Wall-time overhead is `(9.79150278 - 9.11032872) / 9.11032872 * 100 = 7.4769%`.
Total output-byte overhead is `(1197042 - 733752) / 733752 * 100 = 63.1399%`.
The journal-only percentage versus zero baseline bytes is undefined; the absolute
increase is 463282 bytes per eight-capture run, about 57.9 KB per capture when
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
