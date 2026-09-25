import { z } from "zod";
import { safePageUrl } from "./report";

const count = z.number().int().nonnegative().max(Number.MAX_SAFE_INTEGER);
export const siteStatusSchema = z
  .object({
    version: z.literal(1),
    generatedAt: z.iso.datetime(),
    suites: count,
    total: count,
    passed: count,
    failed: count,
    errors: count,
  })
  .refine(
    (status) => status.passed + status.failed === status.total && status.errors <= status.failed,
  );

export type SiteStatus = z.infer<typeof siteStatusSchema>;

export function siteStatusUrl(value: string): string {
  const safe = safePageUrl(value);
  if (!safe) throw new Error("Invalid site URL.");
  return new URL("/_snapshot/status.json", safe).href;
}

type StatusFetcher = (url: string, options: RequestInit) => Promise<Response>;

export async function fetchSiteStatus(
  site: string,
  signal: AbortSignal,
  request: StatusFetcher = fetch,
): Promise<SiteStatus> {
  const response = await request(siteStatusUrl(site), {
    signal: AbortSignal.any([signal, AbortSignal.timeout(10_000)]),
    credentials: "omit",
    mode: "cors",
    cache: "no-cache",
    redirect: "error",
    referrerPolicy: "no-referrer",
  });
  const limit = 64 * 1024;
  if (
    !response.ok ||
    !response.headers.get("content-type")?.toLowerCase().startsWith("application/json") ||
    Number(response.headers.get("content-length")) > limit ||
    !response.body
  ) {
    await response.body?.cancel();
    throw new Error("Status unavailable.");
  }
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let size = 0;
  let source = "";
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      size += value.byteLength;
      if (size > limit) throw new Error("Status exceeds its size limit.");
      source += decoder.decode(value, { stream: true });
    }
    source += decoder.decode();
    const result = siteStatusSchema.safeParse(JSON.parse(source));
    if (!result.success) throw new Error("Unsupported site status.");
    return result.data;
  } finally {
    await reader.cancel().catch(() => {});
  }
}
