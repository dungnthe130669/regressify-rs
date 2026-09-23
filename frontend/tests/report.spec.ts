import { resolve } from "node:path";
import AxeBuilder from "@axe-core/playwright";
import { expect, type Page, test } from "@playwright/test";
import { getAssetVersion } from "../scripts/version-assets";

async function assertLayout(page: Page) {
  await expect
    .poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth))
    .toBe(true);
  expect(await page.evaluate(() => document.fonts.check("14px Roboto"))).toBe(true);
}

async function screenshot(page: Page, project: string, name: string) {
  await assertLayout(page);
  await page.screenshot({
    path: resolve(import.meta.dirname, `../.fixtures/screenshots/${project}/${name}.png`),
    fullPage: true,
    animations: "disabled",
  });
}

test("overview is responsive, accessible, sortable, and theme-aware", async ({ page }, info) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/index.html");
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("heading", { name: "A clear view of every change." })).toBeVisible();
  await expect(page.locator(".results-table tbody tr")).toHaveCount(7);
  await page.getByRole("button", { name: "Suite", exact: true }).click();
  await expect(page.locator(".results-table th").first()).toHaveAttribute(
    "aria-sort",
    "descending",
  );
  await screenshot(page, info.project.name, "overview-light");
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.getByRole("button", { name: "Dark theme" }).click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await screenshot(page, info.project.name, "overview-dark");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "dark");
  await page.getByRole("button", { name: "System theme" }).click();
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  expect(errors).toEqual([]);
});

test("overview centers metrics and result columns with aligned suite labels", async ({
  page,
}, info) => {
  await page.goto("/");
  await expect(page.locator(".results-table tbody tr")).toHaveCount(7);
  await page.evaluate(() => document.fonts.ready);
  const offsets = await page.evaluate(() => {
    const center = (bounds: DOMRect) => bounds.left + bounds.width / 2;
    const metrics = Array.from(document.querySelectorAll(".metrics > div")).flatMap((column) =>
      Array.from(column.children).map((content) =>
        Math.abs(center(content.getBoundingClientRect()) - center(column.getBoundingClientRect())),
      ),
    );
    const cells = Array.from(document.querySelectorAll(".results-table tr")).flatMap((row) =>
      Array.from(row.children)
        .slice(1, 4)
        .filter((cell) => cell.getBoundingClientRect().width > 0)
        .map((cell) => {
          const content = cell.querySelector("button, .status-badge");
          const range = document.createRange();
          range.selectNodeContents(cell);
          return Math.abs(
            center(content?.getBoundingClientRect() || range.getBoundingClientRect()) -
              center(cell.getBoundingClientRect()),
          );
        }),
    );
    const heading = document.querySelector(".results-table th:first-child button");
    const names = Array.from(document.querySelectorAll(".suite-name")).map((name) => {
      const text = Array.from(name.childNodes).find(
        (node) => node.nodeType === Node.TEXT_NODE && node.textContent?.trim(),
      );
      if (!text || !heading) throw new Error("Suite label is missing");
      const range = document.createRange();
      range.selectNodeContents(text);
      return Math.abs(
        (range.getClientRects()[0]?.left || 0) - heading.getBoundingClientRect().left,
      );
    });
    return { metrics, cells, names };
  });
  for (const [group, values] of Object.entries(offsets)) {
    expect(values.length).toBeGreaterThan(0);
    for (const offset of values) expect(offset, `${group} alignment offset`).toBeLessThan(1);
  }
  await screenshot(page, info.project.name, "overview-alignment");
});

test("navigation stays in the SPA, preserves filters, and refreshes deep links", async ({
  page,
  isMobile,
}, info) => {
  await page.goto("/");
  let navigations = 0;
  page.on("request", (request) => {
    if (request.resourceType() === "document") navigations++;
  });
  await page.locator(".suite-name").filter({ hasText: "workspace-overview" }).click();
  await expect(page).toHaveURL(/\/suites\/workspace-overview$/);
  await expect(page.locator(".capture-card")).toHaveCount(4);
  await page.getByRole("searchbox", { name: "Search captures" }).fill("Workspace");
  await expect(page.locator(".capture-card")).toHaveCount(1);
  await page.locator(".capture-card").click();
  await expect(page).toHaveURL(/\/images\/overview__desktop.png\?q=Workspace$/);
  await expect(page.getByRole("region", { name: "Zoomable comparison images" })).toBeVisible();
  expect(navigations).toBe(0);
  await page.locator(".back-link").click();
  await expect(page.getByRole("searchbox", { name: "Search captures" })).toHaveValue("Workspace");
  await page.getByRole("searchbox", { name: "Search captures" }).fill("");
  await screenshot(page, info.project.name, "suite-grid");
  await page.getByRole("button", { name: "List view" }).click();
  await expect(page.locator(".capture-list")).toBeVisible();
  if (isMobile) await page.getByRole("button", { name: "Toggle suite index" }).click();
  await page
    .getByRole("navigation", { name: "Test suites", exact: true })
    .getByRole("link", { name: /orders-and-checkout/ })
    .click();
  await expect(
    page.getByRole("heading", { name: "orders-and-checkout", exact: true }),
  ).toBeVisible();
  await page.reload();
  await expect(page.locator(".capture-card")).toHaveCount(4);
  if (!isMobile) {
    await page.getByRole("button", { name: "Toggle suite index" }).click();
    await expect(page.locator(".suite-panel")).toHaveCount(0);
  }
});

test("image modes, zoom, downloads, and previous/next work", async ({ page, isMobile }, info) => {
  await page.goto("/suites/workspace-overview/images/overview__desktop.png");
  await expect(
    page.getByRole("heading", { name: "Workspace overview / Desktop", exact: true }),
  ).toBeVisible();
  await expect
    .poll(() =>
      page
        .locator(".image-world img")
        .evaluateAll((images) =>
          images.every((image) => (image as HTMLImageElement).naturalWidth > 0),
        ),
    )
    .toBe(true);
  await screenshot(page, info.project.name, "comparison-side");
  await page.getByRole("button", { name: "Actual size", exact: true }).click();
  await expect(page.getByLabel("Zoom level")).toHaveText("100%");
  await page.getByRole("button", { name: "Zoom in", exact: true }).click();
  await expect(page.getByLabel("Zoom level")).not.toHaveText("100%");
  await page.getByRole("button", { name: "Fit images", exact: true }).click();
  const mode = async (value: string, label: string) => {
    if (isMobile) await page.getByRole("combobox", { name: "Comparison mode" }).selectOption(value);
    else await page.getByRole("button", { name: label, exact: true }).click();
    await expect(page.locator(`.image-world.mode-${value}`)).toBeVisible();
  };
  await mode("wipe", "Wipe");
  if (!isMobile) {
    const bounds = await page.locator(".image-world").boundingBox();
    if (!bounds) throw new Error("Wipe image is not visible");
    await page.mouse.move(bounds.x + bounds.width * 0.25, bounds.y + bounds.height * 0.5);
    await expect(page.getByRole("slider", { name: "Wipe position" })).toHaveValue("25");
    await page.mouse.move(bounds.x + bounds.width * 0.75, bounds.y + bounds.height * 0.5);
    await expect(page.getByRole("slider", { name: "Wipe position" })).toHaveValue("75");
  }
  await page.getByRole("slider", { name: "Wipe position" }).focus();
  await page.getByRole("slider", { name: "Wipe position" }).press("Home");
  await expect(page.getByRole("slider", { name: "Wipe position" })).toHaveValue("0");
  await mode("overlay", "Overlay");
  await page.getByRole("slider", { name: "Test image opacity" }).press("End");
  await expect(page.getByRole("slider", { name: "Test image opacity" })).toHaveValue("100");
  await mode("diff", "Diff");
  await screenshot(page, info.project.name, "comparison-diff");
  await mode("reference", "Reference");
  await mode("test", "Test");
  const download = page.waitForEvent("download");
  await page.getByRole("link", { name: "Download test", exact: true }).click();
  expect((await download).suggestedFilename()).toContain("test.png");
  await page.getByRole("link", { name: "Next image", exact: true }).click();
  await expect(page).toHaveURL(/recent-orders__mobile.png/);
  await page.getByRole("link", { name: "Previous image", exact: true }).click();
  await expect(page).toHaveURL(/overview__desktop.png/);
  await page.reload();
  await expect(page.locator(".mode-test")).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test("mouse wheel zoom changes incrementally without jumping to scale limits", async ({ page }) => {
  await page.goto("/suites/workspace-overview/images/overview__desktop.png?mode=side");
  await expect(page.locator(".comparison-lane img")).toHaveCount(3);
  await expect
    .poll(() =>
      page
        .locator(".comparison-lane img")
        .evaluateAll((images) =>
          images.every((image) => (image as HTMLImageElement).naturalWidth > 0),
        ),
    )
    .toBe(true);
  const zoomLevel = async () => Number.parseFloat(await page.getByLabel("Zoom level").innerText());
  await page.getByRole("button", { name: "Actual size", exact: true }).click();
  await expect(page.getByLabel("Zoom level")).toHaveText("100%");
  await page.locator(".zoom-canvas").hover({ position: { x: 80, y: 80 } });
  for (const delta of [-120, -120, 120, -1200, 1200, -1, 1]) {
    const before = await zoomLevel();
    await page.mouse.wheel(0, delta);
    if (delta < 0) {
      await expect.poll(zoomLevel).toBeGreaterThan(before);
      expect(await zoomLevel()).toBeLessThan(before * 1.3);
    } else {
      await expect.poll(zoomLevel).toBeLessThan(before);
      expect(await zoomLevel()).toBeGreaterThan(before * 0.7);
    }
  }
  await page.getByRole("button", { name: "Fit images", exact: true }).click();
  await page.getByRole("button", { name: "Actual size", exact: true }).click();
  await expect(page.getByLabel("Zoom level")).toHaveText("100%");
});

test("side by side includes a third lane only when a diff is available", async ({ page }, info) => {
  await page.goto("/suites/workspace-overview/images/overview__desktop.png?mode=side");
  await expect(page.locator(".comparison-lane")).toHaveCount(3);
  await expect(page.locator(".suite-panel")).toHaveCount(0);
  await expect(page.locator(".review-details")).not.toHaveAttribute("open", "");
  await expect(page.locator(".canvas-labels")).toContainText("Diff");
  await expect(
    page.getByRole("img", { name: "Diff: Workspace overview / Desktop", exact: true }),
  ).toBeVisible();
  const lanes = await page.locator(".comparison-lane").evaluateAll((elements) =>
    elements.map((element) => {
      const bounds = element.getBoundingClientRect();
      return { x: bounds.x, y: bounds.y, width: bounds.width };
    }),
  );
  expect(lanes[0]?.y).toBe(lanes[2]?.y);
  expect(lanes[0]?.width).toBeCloseTo(lanes[2]?.width || 0);
  expect(lanes[0]?.x).toBeLessThan(lanes[1]?.x || 0);
  expect(lanes[1]?.x).toBeLessThan(lanes[2]?.x || 0);
  await expect
    .poll(() =>
      page.evaluate(() => {
        const canvas = document.querySelector(".zoom-canvas")?.getBoundingClientRect();
        const lane = document.querySelector(".comparison-lane")?.getBoundingClientRect();
        return canvas && lane ? Math.abs(lane.y - canvas.y - 1) : Number.POSITIVE_INFINITY;
      }),
    )
    .toBeLessThan(1);
  await screenshot(page, info.project.name, "comparison-three-lanes");
  await page.goto("/suites/customer-accounts/images/overview__desktop.png?mode=side");
  await expect(page.locator(".comparison-lane")).toHaveCount(2);
  await expect(page.locator(".canvas-labels")).not.toContainText("Diff");
});

test("empty results, missing images, errors, and unknown routes are explicit", async ({
  page,
  isMobile,
}, info) => {
  await page.goto("/");
  await page.getByRole("searchbox", { name: "Search suites" }).fill("no-such-suite");
  await expect(page.getByRole("heading", { name: "No matching suites" })).toBeVisible();
  await page.goto("/suites/empty-suite");
  await expect(page.getByRole("heading", { name: "No matching captures" })).toBeVisible();
  await page.goto("/suites/product-catalog/images/full-page__mobile.png");
  await expect(page.getByRole("alert")).toContainText("Navigation timed out");
  await expect(page.getByText("No image", { exact: true })).toBeVisible();
  if (!isMobile)
    await expect(page.getByRole("button", { name: "Wipe", exact: true })).toBeDisabled();
  await screenshot(page, info.project.name, "comparison-error");
  await page.goto("/suites/missing");
  await expect(page.getByRole("heading", { name: "Suite not found" })).toBeVisible();
  await page.goto("/suites/workspace-overview/images/missing");
  await expect(page.getByRole("heading", { name: "Image not found" })).toBeVisible();
});

test("invalid report data fails visibly and can be retried", async ({ page }) => {
  await page.route("**/_snapshot/**", async (route) => {
    if (new URL(route.request().url()).pathname.endsWith(".json"))
      await route.fulfill({ contentType: "application/json", body: '{"version":99}' });
    else await route.continue();
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Snapshot unavailable" })).toBeVisible();
  await page.unroute("**/_snapshot/**");
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(page.getByRole("heading", { name: "A clear view of every change." })).toBeVisible();
});

test("untrusted Markdown is not executable markup or a remote image request", async ({ page }) => {
  await page.route("**/_snapshot/**", async (route) => {
    if (!new URL(route.request().url()).pathname.endsWith(".json")) return route.continue();
    const response = await route.fetch();
    const data = await response.json();
    if (data.version === 2)
      data.subheading =
        '<img src="https://untrusted.invalid/leak" onerror="window.compromised=true"> [Unsafe](javascript:alert(1)) **Safe text**';
    await route.fulfill({ response, json: data });
  });
  await page.goto("/");
  await expect(page.locator(".report-subheading")).toContainText("Safe text");
  await expect(page.locator(".report-subheading img")).toHaveCount(0);
  await expect(page.locator('a[href^="javascript:"]')).toHaveCount(0);
  expect(await page.evaluate(() => "compromised" in window)).toBe(false);
});

test("empty snapshots and very long report text fit narrow and wide screens", async ({ page }) => {
  await page.route("**/_snapshot/**", async (route) => {
    if (!new URL(route.request().url()).pathname.endsWith(".json")) return route.continue();
    const response = await route.fetch();
    const data = await response.json();
    if (data.version === 2) {
      data.heading = "LongUnbrokenReportHeading".repeat(8);
      data.subheading = `**Release** / ${"VeryLongBuildIdentifier".repeat(8)}`;
      data.suites = [];
    }
    await route.fulfill({ response, json: data });
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "No comparisons yet" })).toBeVisible();
  for (const width of [320, 768, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    await assertLayout(page);
  }
});

test("every snapshot payload has a content version on a stable path", async ({ page }) => {
  const requests = new Set<string>();
  page.on("request", (request) => {
    if (new URL(request.url()).pathname.startsWith("/_snapshot/")) requests.add(request.url());
  });
  await page.goto("/suites/workspace-overview/images/overview__desktop.png?mode=side");
  await expect(page.locator(".comparison-lane img")).toHaveCount(3);
  await expect
    .poll(() =>
      page
        .locator(".comparison-lane img")
        .evaluateAll((images) =>
          images.every((image) => (image as HTMLImageElement).naturalWidth > 0),
        ),
    )
    .toBe(true);
  expect([...requests].some((url) => new URL(url).pathname.endsWith("/manifest.json"))).toBe(true);
  expect(
    [...requests].filter((url) => new URL(url).pathname.endsWith(".png")).length,
  ).toBeGreaterThanOrEqual(3);
  for (const resource of requests) {
    const url = new URL(resource);
    expect(url.searchParams.get("v")).toMatch(/^[A-Za-z0-9_-]{20}$/);
    expect(url.pathname).not.toMatch(/[a-f0-9]{64}\.(json|png)$/);
    const response = await page.request.get(resource);
    expect(response.ok()).toBe(true);
    expect(url.searchParams.get("v")).toBe(getAssetVersion(await response.body()));
  }
});
