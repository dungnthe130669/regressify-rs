import { expect, test } from "bun:test";
import { fetchSiteStatus, type SiteStatus, siteStatusSchema, siteStatusUrl } from "./landing";

const status: SiteStatus = {
  version: 1,
  generatedAt: "2026-09-24T00:00:00Z",
  suites: 3,
  total: 10,
  passed: 8,
  failed: 2,
  errors: 1,
};

test("landing status uses a remote origin, including links to suite pages", () => {
  expect(siteStatusUrl("https://another-user.example/suites/shop?filter=fail#top")).toBe(
    "https://another-user.example/_snapshot/status.json",
  );
  for (const unsafe of [
    "javascript:alert(1)",
    "file:///private",
    "https://user:secret@example.com",
    "/workspace",
  ]) {
    expect(() => siteStatusUrl(unsafe)).toThrow();
  }
});

test("landing status validates versions and consistent capture counts", () => {
  expect(siteStatusSchema.parse(status)).toEqual(status);
  for (const invalid of [
    { ...status, version: 2 },
    { ...status, passed: 9 },
    { ...status, errors: 3 },
    { ...status, suites: -1 },
    { ...status, total: Number.MAX_SAFE_INTEGER + 1 },
    { ...status, generatedAt: "not a date" },
  ])
    expect(siteStatusSchema.safeParse(invalid).success).toBe(false);
  expect(
    siteStatusSchema.safeParse({ ...status, suites: 0, total: 0, passed: 0, failed: 0, errors: 0 })
      .success,
  ).toBe(true);
});

test("landing fetch is anonymous, bounded, revalidated and independent of local reports", async () => {
  const result = await fetchSiteStatus(
    "https://remote.example/",
    new AbortController().signal,
    async (url, options) => {
      expect(url).toBe("https://remote.example/_snapshot/status.json");
      expect(options.credentials).toBe("omit");
      expect(options.mode).toBe("cors");
      expect(options.cache).toBe("no-cache");
      expect(options.redirect).toBe("error");
      expect(options.referrerPolicy).toBe("no-referrer");
      return Response.json(status);
    },
  );
  expect(result).toEqual(status);
  for (const response of [
    new Response("<html>Old report</html>", { headers: { "Content-Type": "text/html" } }),
    new Response("Missing", { status: 404 }),
    Response.json({ ...status, version: 9 }),
    new Response(" ".repeat(64 * 1024 + 1), { headers: { "Content-Type": "application/json" } }),
    new Response("{}", {
      headers: { "Content-Type": "application/json", "Content-Length": "999999999" },
    }),
  ]) {
    await expect(
      fetchSiteStatus("https://remote.example", new AbortController().signal, async () => response),
    ).rejects.toThrow();
  }
  await expect(
    fetchSiteStatus("https://remote.example", new AbortController().signal, async () => {
      throw new TypeError("CORS blocked");
    }),
  ).rejects.toThrow();
});
