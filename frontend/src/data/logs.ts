import { z } from "zod";
import { type ExecutionLog, fetchReport } from "./report";

export const logLevels = ["trace", "debug", "info", "warn", "error"] as const;
export const logStages = [
  "queueSetup",
  "navigation",
  "stateHooks",
  "actionsFrames",
  "screenshotSettling",
  "captureArtifacts",
  "finish",
  "preflight",
  "snapshotStaging",
  "packagingOptimization",
  "apiUploads",
  "retryPolling",
  "completionCleanup",
] as const;
const integer = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
const identity = z.string().regex(/^sha256:[0-9a-f]{64}$/);
const fieldValue = z.discriminatedUnion("type", [
  z.object({ type: z.literal("number"), value: integer }),
  z.object({
    type: z.literal("coordinate"),
    value: z.number().int().min(-2147483648).max(2147483647),
  }),
  z.object({ type: z.literal("boolean"), value: z.boolean() }),
  z.object({ type: z.literal("identity"), value: identity }),
  z.object({
    type: z.literal("kind"),
    value: z.enum([
      "click",
      "hover",
      "check",
      "uncheck",
      "focus",
      "input",
      "press",
      "select",
      "scroll",
      "navigate",
      "save",
      "hide",
      "remove",
      "wait",
      "persist",
      "restore",
      "switchTab",
      "hook",
      "before",
      "ready",
      "exists",
      "visible",
      "stable",
      "enabled",
      "unobscured",
      "fonts",
      "images",
      "lazyLoading",
      "animation",
      "caret",
      "cached",
      "download",
      "browser",
      "page",
      "handler",
      "azure",
      "netlify",
      "cloudflare",
      "manifest",
      "package",
      "upload",
      "poll",
      "publication",
      "preflight",
      "transient",
      "rateLimited",
      "timeout",
      "pending",
      "complete",
      "failed",
      "cancelled",
      "init",
      "install",
      "mcp",
      "version",
      "test",
      "reference",
      "compare",
      "approve",
      "snapshot",
      "optimize",
      "empty",
      "belowMinimum",
      "unchanged",
      "cli",
      "desktop",
      "worker",
      "shell",
      "window",
      "unknown",
    ]),
  }),
]);
const fields = [
  "command",
  "actionIndex",
  "actionType",
  "target",
  "destination",
  "artifact",
  "capture",
  "timeoutMs",
  "elementTimeoutMs",
  "actionabilityTimeoutMs",
  "pollIntervalMs",
  "scenarioTimeoutMs",
  "retryLimit",
  "retryAttempt",
  "backoffMs",
  "queueMs",
  "elapsedMs",
  "completedAnimations",
  "frozenAnimations",
  "width",
  "height",
  "x",
  "y",
  "bytes",
  "bytesBefore",
  "bytesAfter",
  "budgetBytes",
  "fileCount",
  "skippedFiles",
  "batchCount",
  "selectedSuites",
  "statusCode",
  "osErrorCode",
  "requestId",
  "provider",
  "endpoint",
  "condition",
  "reason",
  "state",
  "credentialPresent",
  "compression",
  "deadlineMs",
  "redirectCount",
  "frameDepth",
  "tabCount",
  "infrastructure",
  "cleanupFailed",
] as const;
const category = z.enum([
  "timeout",
  "notFound",
  "permission",
  "navigation",
  "actionability",
  "cancelled",
  "invalidData",
  "storage",
  "provider",
  "unknown",
]);
export const logRecordSchema = z.object({
  schemaVersion: z.literal(1),
  operationId: z.uuid(),
  operationKind: z.literal("capture"),
  captureRunId: z.string().nullable(),
  execution: z
    .object({
      executionId: z.uuid(),
      suiteId: identity,
      scenarioId: identity,
      viewport: z.object({ name: identity, width: integer.positive(), height: integer.positive() }),
      captureRole: z.enum(["reference", "test"]),
      attempt: integer.positive(),
    })
    .nullable(),
  sequence: integer,
  timestampUtcMs: z.number().int().min(-8640000000000000).max(8640000000000000),
  elapsedMs: integer,
  severity: z.enum(logLevels),
  stage: z.enum(logStages),
  eventName: z.enum([
    "queued",
    "started",
    "stageStarted",
    "stageFinished",
    "decision",
    "waiting",
    "retry",
    "heartbeat",
    "captured",
    "compared",
    "published",
    "warning",
    "cancelled",
    "finished",
  ]),
  message: z.string().max(4096),
  fields: z.partialRecord(z.enum(fields), fieldValue),
  spanId: z.uuid(),
  parentSpan: z.uuid().nullable(),
  durationMs: integer.nullable(),
  outcome: z
    .enum(["success", "failure", "cancelled", "cancelledRemoteUnknown", "mismatch", "unknown"])
    .nullable(),
  errorCause: z
    .object({ category, causes: z.array(category).max(8) })
    .strict()
    .nullable(),
});
export const logChunkSchema = z.object({
  version: z.literal(1),
  operationId: z.uuid(),
  records: z.array(logRecordSchema).max(250),
});
export type LogRecord = z.infer<typeof logRecordSchema>;
const chunkCache = new Map<
  string,
  { request: Promise<z.infer<typeof logChunkSchema>>; controller: AbortController }
>();

export function formatLogRecord(record: LogRecord): string {
  const fields = Object.entries(record.fields)
    .map(([key, field]) => `${key}=${field.value}`)
    .join(" ");
  const outcome = record.outcome ? ` outcome=${record.outcome}` : "";
  const duration = record.durationMs === null ? "" : ` duration=${record.durationMs}ms`;
  const causes = record.errorCause ? ` causes=${record.errorCause.causes.join(", ")}` : "";
  return `[${record.elapsedMs}ms #${record.sequence}] ${record.severity} ${record.stage}: ${record.message}${fields ? ` ${fields}` : ""}${outcome}${duration}${causes}`;
}

export async function fetchLogChunk(
  log: ExecutionLog,
  index: number,
  signal?: AbortSignal,
  revision = 0,
) {
  const path = log.chunks[index];
  if (!path || !log.operationId) throw new Error("Log chunk is missing.");
  const cacheKey = `${path}#${revision}`;
  let cached = chunkCache.get(cacheKey);
  if (!cached) {
    while (chunkCache.size >= 4) {
      const oldest = chunkCache.entries().next().value;
      if (!oldest) break;
      oldest[1].controller.abort();
      chunkCache.delete(oldest[0]);
    }
    const controller = new AbortController();
    const request = fetchReport(
      `/${path}`,
      logChunkSchema,
      AbortSignal.any([controller.signal, AbortSignal.timeout(15000)]),
      revision > 0,
      256 * 1024,
    );
    cached = { request, controller };
    chunkCache.set(cacheKey, cached);
    request.catch(() => {
      if (chunkCache.get(cacheKey)?.request === request) chunkCache.delete(cacheKey);
    });
  }
  const chunk = await cached.request;
  signal?.throwIfAborted();
  if (
    chunk.operationId !== log.operationId ||
    chunk.records.some(
      (record, position) =>
        record.operationId !== log.operationId ||
        (position > 0 && record.sequence !== (chunk.records[position - 1]?.sequence ?? -1) + 1) ||
        (record.execution && log.executionId && record.execution.executionId !== log.executionId),
    )
  )
    throw new Error("Log identity or sequence does not match this capture.");
  return chunk.records;
}

export function matchesLog(
  record: LogRecord,
  level: string,
  stage: string,
  search: string,
): boolean {
  const minimum = logLevels.indexOf(level as (typeof logLevels)[number]);
  return (
    logLevels.indexOf(record.severity) >= minimum &&
    (!stage || stage === "all" || record.stage === stage) &&
    formatLogRecord(record).toLowerCase().includes(search.toLowerCase())
  );
}
