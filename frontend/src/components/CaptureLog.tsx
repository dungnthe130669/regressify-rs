import { ArrowLeft, ArrowRight, Copy, Download, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import {
  fetchLogChunk,
  formatLogRecord,
  type LogRecord,
  logLevels,
  logStages,
  matchesLog,
} from "../data/logs";
import type { ArchiveState, Capture } from "../data/report";
import { IconButton } from "./ui";

const availability: Record<ArchiveState, string> = {
  complete: "Complete capture log",
  partial: "Partial or interrupted log; records may be missing",
  missing: "Expected capture log is missing",
  legacyUnavailable: "Legacy capture: log not available",
  loadingError: "Capture log could not be loaded",
};

export function CaptureLog({
  capture,
  params,
  set,
}: {
  capture: Capture;
  params: URLSearchParams;
  set: (key: string, value: string) => void;
}) {
  const role = params.get("logRole") === "reference" ? "reference" : "test";
  const log = capture.logs?.[role];
  const state = log?.state ?? "legacyUnavailable";
  const requested = Number(params.get("logChunk") || 0);
  const index =
    Number.isSafeInteger(requested) && requested >= 0 && requested < (log?.chunks.length ?? 0)
      ? requested
      : 0;
  const level = params.get("logLevel") || "all";
  const stage = params.get("logStage") || "all";
  const search = params.get("logSearch") || "";
  const [records, setRecords] = useState<LogRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  const [exporting, setExporting] = useState(false);
  const [notice, setNotice] = useState("");
  useEffect(() => {
    const controller = new AbortController();
    setRecords([]);
    setError("");
    setNotice("");
    if (!log || !["complete", "partial"].includes(log.state) || !log.chunks.length) {
      setLoading(false);
      return;
    }
    setLoading(true);
    fetchLogChunk(log, index, controller.signal, revision)
      .then((records) => {
        if (!controller.signal.aborted) setRecords(records);
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted)
          setError(error instanceof Error ? error.message : "Capture log could not be loaded");
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [log, index, revision]);
  const filtered = records.filter((record) => matchesLog(record, level, stage, search));
  const execution = records.find((record) => record.execution)?.execution;
  const shownState = error ? "loadingError" : state;
  async function download() {
    if (!log || state !== "complete" || error || exporting) return;
    setExporting(true);
    setNotice("");
    try {
      const parts: string[] = [];
      let count = 0;
      let bytes = 0;
      for (let chunk = 0; chunk < log.chunks.length; chunk += 1) {
        const entries = await fetchLogChunk(log, chunk);
        for (const record of entries) {
          if (record.sequence !== count) throw new Error("Log is incomplete; export stopped.");
          count += 1;
          if (matchesLog(record, level, stage, search)) {
            const text = `${formatLogRecord(record)}\n`;
            bytes += new TextEncoder().encode(text).length;
            if (bytes > 128 * 1024 * 1024)
              throw new Error(
                "Filtered log exceeds the 128 MiB browser export limit. Narrow the filters.",
              );
            parts.push(text);
          }
        }
      }
      if (count !== log.records)
        throw new Error("Log record count does not match; export stopped.");
      const url = URL.createObjectURL(new Blob(parts, { type: "text/plain;charset=utf-8" }));
      const link = document.createElement("a");
      link.href = url;
      link.download = `${capture.id}-${role}-log.txt`;
      link.click();
      URL.revokeObjectURL(url);
    } catch (error) {
      setNotice(error instanceof Error ? error.message : "Log export failed");
    } finally {
      setExporting(false);
    }
  }
  return (
    <section className="capture-log" aria-label="Capture Log">
      <div className="log-toolbar">
        <fieldset className="segmented" aria-label="Log execution">
          {(["reference", "test"] as const).map((value) => (
            <button
              type="button"
              key={value}
              aria-pressed={role === value}
              onClick={() => set("logRole", value)}
            >
              {value === "reference" ? "Reference" : "Test"}
            </button>
          ))}
        </fieldset>
        <label>
          Severity
          <select
            aria-label="Log severity"
            value={level}
            onChange={(event) => set("logLevel", event.target.value)}
          >
            <option value="all">All levels</option>
            {logLevels.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </label>
        <label>
          Stage
          <select
            aria-label="Log stage"
            value={stage}
            onChange={(event) => set("logStage", event.target.value)}
          >
            <option value="all">All stages</option>
            {logStages.map((value) => (
              <option key={value} value={value}>
                {value}
              </option>
            ))}
          </select>
        </label>
        <label className="log-search">
          Search
          <input
            type="search"
            aria-label="Search capture log"
            value={search}
            onChange={(event) => set("logSearch", event.target.value)}
          />
        </label>
        <IconButton
          label="Copy displayed log page"
          disabled={shownState !== "complete" || loading}
          onClick={() => {
            navigator.clipboard
              .writeText(filtered.map(formatLogRecord).join("\n"))
              .catch(() => setNotice("Clipboard is unavailable"));
          }}
        >
          <Copy size={16} />
        </IconButton>
        <IconButton
          label={
            shownState === "complete"
              ? "Download filtered capture log"
              : `Export unavailable: ${availability[shownState]}`
          }
          disabled={shownState !== "complete" || loading || exporting}
          onClick={download}
        >
          <Download size={16} />
        </IconButton>
      </div>
      <div className="log-availability" data-log-state={shownState} role="status">
        <span>{availability[shownState]}</span>
        {execution && (
          <span className="mono">
            {execution.viewport.width} x {execution.viewport.height} / Attempt {execution.attempt}
          </span>
        )}
        {log?.missingRecords ? <span>{log.missingRecords} missing records</span> : null}
      </div>
      {notice && <p role="alert">{notice}</p>}
      {error && (
        <p role="alert">
          {error}{" "}
          <IconButton label="Retry capture log" onClick={() => setRevision(revision + 1)}>
            <RefreshCw size={16} />
          </IconButton>
        </p>
      )}
      {loading ? (
        <p role="status">Loading capture log...</p>
      ) : (
        <ol className="log-records" aria-label="Execution timeline">
          {filtered.map((record) => (
            <li key={`${record.operationId}-${record.sequence}`} data-severity={record.severity}>
              <time dateTime={new Date(record.timestampUtcMs).toISOString()}>
                {new Date(record.timestampUtcMs).toISOString().slice(11, 23)}
              </time>
              <pre>{formatLogRecord(record)}</pre>
            </li>
          ))}
        </ol>
      )}
      <div className="log-pagination">
        <IconButton
          label="Previous log chunk"
          disabled={index === 0 || loading}
          onClick={() => set("logChunk", String(index - 1))}
        >
          <ArrowLeft size={16} />
        </IconButton>
        <span className="mono">
          {log?.chunks.length ? `${index + 1} / ${log.chunks.length}` : "0 / 0"}
        </span>
        <IconButton
          label="Next log chunk"
          disabled={index + 1 >= (log?.chunks.length ?? 0) || loading}
          onClick={() => set("logChunk", String(index + 1))}
        >
          <ArrowRight size={16} />
        </IconButton>
      </div>
    </section>
  );
}
