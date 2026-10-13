import { z } from "zod";

const count = z.number().int().nonnegative();
const percentage = z.number().finite().min(0).max(100);
const segment = z
  .string()
  .min(1)
  .refine(
    (value) =>
      value !== "." &&
      value !== ".." &&
      !/[\\/]/.test(value) &&
      !Array.from(value).some((character) => {
        const code = character.charCodeAt(0);
        return code < 32 || (code >= 127 && code <= 159);
      }),
  );

export function encodeArtifactPath(path: string): string {
  return path
    .split("/")
    .map((part) =>
      encodeURIComponent(part).replace(
        /[!'()*]/g,
        (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`,
      ),
    )
    .join("/");
}

export function artifactPath(value: string, kind: "data" | "images"): string | undefined {
  const [path, version, extra] = value.split("?v=");
  if (
    !path ||
    !version ||
    extra !== undefined ||
    !/^[A-Za-z0-9_-]{20}$/.test(version) ||
    !path.startsWith(`_snapshot/${kind}/`) ||
    !path.toLowerCase().endsWith(kind === "data" ? ".json" : ".png")
  )
    return undefined;
  try {
    const parts = path.split("/").map(decodeURIComponent);
    if (
      parts.some(
        (part) =>
          !segment.safeParse(part).success ||
          /[\\/:<>"|?*]/.test(part) ||
          /[. ]$/.test(part) ||
          /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part),
      )
    )
      return undefined;
    const decoded = parts.join("/");
    return encodeArtifactPath(decoded) === path ? decoded : undefined;
  } catch {
    return undefined;
  }
}

const artifact = (kind: "data" | "images") =>
  z.string().refine((value) => artifactPath(value, kind) !== undefined);

export const suiteSummarySchema = z
  .object({
    id: segment,
    displayName: z.string().optional(),
    total: count,
    passed: count,
    failed: count,
    errors: count,
    report: artifact("data"),
  })
  .refine((suite) => suite.passed + suite.failed === suite.total && suite.errors <= suite.failed);

export const manifestSchema = z
  .object({
    version: z.literal(2),
    title: z.string(),
    heading: z.string(),
    subheading: z.string(),
    generatedAt: z.iso.datetime(),
    suites: z.array(suiteSummarySchema),
  })
  .refine(
    (manifest) => new Set(manifest.suites.map((suite) => suite.id)).size === manifest.suites.length,
  );

export const archiveStateSchema = z.enum([
  "complete",
  "partial",
  "missing",
  "legacyUnavailable",
  "loadingError",
]);

export const executionLogSchema = z.object({
  state: archiveStateSchema,
  operationId: z.uuid().nullable(),
  executionId: z.uuid().nullable(),
  chunks: z.array(artifact("data")).max(16384),
  records: count.max(Number.MAX_SAFE_INTEGER),
  missingRecords: count.max(Number.MAX_SAFE_INTEGER).nullable(),
  build: z
    .object({ version: z.string(), sourceCommit: z.string(), dirty: z.boolean().nullable() })
    .nullable(),
});

export type ExecutionLog = z.infer<typeof executionLogSchema>;
export type ArchiveState = z.infer<typeof archiveStateSchema>;

const imageSizeSchema = z.object({
  width: z.number().int().positive().max(0xffffffff),
  height: z.number().int().positive().max(0xffffffff),
});

const verticalGapSchema = z.object({
  start: count.max(0xffffffff),
  height: z.number().int().positive().max(0xffffffff),
});
type VerticalGap = z.infer<typeof verticalGapSchema>;

export const captureSchema = z
  .object({
    id: segment,
    label: z.string(),
    scenario: z.string(),
    selector: z.string(),
    status: z.enum(["pass", "fail"]),
    threshold: percentage,
    mismatch: percentage.nullable(),
    error: z.string().nullable(),
    comparisonError: z.string().nullable().optional(),
    reference: artifact("images").nullable(),
    test: artifact("images").nullable(),
    diff: artifact("images").nullable(),
    alignment: z
      .object({ reference: verticalGapSchema.nullable(), test: verticalGapSchema.nullable() })
      .refine((alignment) => !alignment.reference || !alignment.test)
      .optional(),
    imageSizes: z
      .object({
        reference: imageSizeSchema.nullable(),
        test: imageSizeSchema.nullable(),
        diff: imageSizeSchema.nullable(),
      })
      .optional(),
    previews: z
      .object({
        reference: artifact("images").nullable(),
        test: artifact("images").nullable(),
        diff: artifact("images").nullable(),
        grid: artifact("images").nullable(),
      })
      .optional(),
    referenceUrl: z.string().nullable(),
    testUrl: z.string().nullable(),
    dimensions: z
      .object({ same: z.boolean(), width: z.number().int(), height: z.number().int() })
      .nullable(),
    logs: z
      .object({ reference: executionLogSchema.nullable(), test: executionLogSchema.nullable() })
      .optional(),
  })
  .transform((capture) => {
    if (
      capture.error !== null &&
      (capture.mismatch !== null ||
        capture.error === "Reference image not found" ||
        capture.error.startsWith("Comparison error: "))
    ) {
      return {
        ...capture,
        comparisonError: capture.comparisonError ?? capture.error,
        error: null,
      };
    }
    return capture;
  });

export const suiteReportSchema = z
  .object({
    id: segment,
    displayName: z.string().optional(),
    runId: z.string(),
    tests: z.array(captureSchema),
  })
  .refine((suite) => new Set(suite.tests.map((test) => test.id)).size === suite.tests.length);

export type Manifest = z.infer<typeof manifestSchema>;
export type SuiteSummary = z.infer<typeof suiteSummarySchema>;
export type SuiteReport = z.infer<typeof suiteReportSchema>;
export type Capture = z.infer<typeof captureSchema>;
export type StatusFilter = "all" | "fail" | "pass" | "error";

export function imageAlignmentGap(
  gap: VerticalGap | null | undefined,
  imageHeight: number,
  diffHeight: number | undefined,
): VerticalGap | null {
  return gap && gap.start <= imageHeight && imageHeight + gap.height === diffHeight ? gap : null;
}

export function imageRowToAlignedRow(row: number, gap: VerticalGap | null): number {
  return gap && row >= gap.start ? row + gap.height : row;
}

export function alignedRowToImageRow(row: number, gap: VerticalGap | null): number | null {
  if (!gap || row < gap.start) return row;
  return row < gap.start + gap.height ? null : row - gap.height;
}

export function suiteLabel(suite: { id: string; displayName?: string }): string {
  return suite.displayName?.trim() ? suite.displayName : suite.id;
}

export function safePageUrl(value: string | null): string | undefined {
  if (!value) return undefined;
  try {
    const url = new URL(value);
    return (url.protocol === "http:" || url.protocol === "https:") && !url.username && !url.password
      ? url.href
      : undefined;
  } catch {
    return undefined;
  }
}

export function suitePath(id: string): string {
  return `/suites/${encodeURIComponent(id)}`;
}

export function imagePath(suite: string, capture: string): string {
  return `${suitePath(suite)}/images/${encodeURIComponent(capture)}`;
}

export function passRate(passed: number, total: number): string {
  return total === 0
    ? "0"
    : ((passed / total) * 100).toLocaleString("en", { maximumFractionDigits: 1 });
}

export function filterCaptures(tests: Capture[], query: string, status: StatusFilter): Capture[] {
  const needle = query.trim().toLocaleLowerCase();
  return tests.filter(
    (test) =>
      (status === "all" || (status === "error" ? test.error !== null : test.status === status)) &&
      `${test.label} ${test.scenario} ${test.selector} ${test.id}`
        .toLocaleLowerCase()
        .includes(needle),
  );
}

export async function fetchReport<T>(
  url: string,
  schema: z.ZodType<T>,
  signal?: AbortSignal,
  reload = false,
  maxBytes?: number,
): Promise<T> {
  const response = await fetch(url, {
    signal,
    credentials: "omit",
    cache: reload ? "reload" : "default",
  });
  if (!response.ok) throw new Error(`Report request failed (${response.status}).`);
  let value: unknown;
  if (maxBytes !== undefined) {
    const reader = response.body?.getReader();
    if (!reader) throw new Error("Report response has no content.");
    const chunks: Uint8Array[] = [];
    let length = 0;
    try {
      while (true) {
        const item = await reader.read();
        if (item.done) break;
        length += item.value.byteLength;
        if (length > maxBytes) throw new Error("Log chunk exceeds its size limit.");
        chunks.push(item.value);
      }
    } finally {
      await reader.cancel();
      reader.releaseLock();
    }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) {
      bytes.set(chunk, offset);
      offset += chunk.length;
    }
    const expected = new URL(url, window.location.origin).searchParams.get("v");
    const digest = new Uint8Array(await crypto.subtle.digest("SHA-384", bytes));
    const actual = btoa(String.fromCharCode(...digest))
      .replaceAll("+", "-")
      .replaceAll("/", "_")
      .replace(/=+$/, "")
      .slice(0, 20);
    if (expected !== actual) throw new Error("Log chunk version does not match its content.");
    value = JSON.parse(new TextDecoder().decode(bytes));
  } else {
    value = await response.json();
  }
  const parsed = schema.safeParse(value);
  if (!parsed.success) throw new Error("This report is invalid or uses an unsupported format.");
  return parsed.data;
}
