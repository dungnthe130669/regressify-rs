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

export const captureSchema = z.object({
  id: segment,
  label: z.string(),
  scenario: z.string(),
  selector: z.string(),
  status: z.enum(["pass", "fail"]),
  threshold: percentage,
  mismatch: percentage.nullable(),
  error: z.string().nullable(),
  reference: artifact("images").nullable(),
  test: artifact("images").nullable(),
  diff: artifact("images").nullable(),
  referenceUrl: z.string().nullable(),
  testUrl: z.string().nullable(),
  dimensions: z
    .object({ same: z.boolean(), width: z.number().int(), height: z.number().int() })
    .nullable(),
});

export const suiteReportSchema = z
  .object({
    id: segment,
    runId: z.string(),
    tests: z.array(captureSchema),
  })
  .refine((suite) => new Set(suite.tests.map((test) => test.id)).size === suite.tests.length);

export type Manifest = z.infer<typeof manifestSchema>;
export type SuiteSummary = z.infer<typeof suiteSummarySchema>;
export type SuiteReport = z.infer<typeof suiteReportSchema>;
export type Capture = z.infer<typeof captureSchema>;
export type StatusFilter = "all" | "fail" | "pass" | "error";

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
): Promise<T> {
  const response = await fetch(url, {
    signal,
    credentials: "omit",
    cache: reload ? "reload" : "default",
  });
  if (!response.ok) throw new Error(`Report request failed (${response.status}).`);
  const parsed = schema.safeParse(await response.json());
  if (!parsed.success) throw new Error("This report is invalid or uses an unsupported format.");
  return parsed.data;
}
