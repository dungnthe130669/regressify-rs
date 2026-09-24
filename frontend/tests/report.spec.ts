import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import AxeBuilder from '@axe-core/playwright';
import { expect, type Page, test } from '@playwright/test';
import { getAssetVersion } from '../scripts/version-assets';

async function assertLayout(page: Page) {
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  expect(await page.evaluate(() => document.fonts.check('14px Roboto'))).toBe(true);
}

async function screenshot(page: Page, project: string, name: string) {
  await assertLayout(page);
  await page.screenshot({
    path: resolve(import.meta.dirname, `../.fixtures/screenshots/${project}/${name}.png`),
    fullPage: true,
    animations: 'disabled',
  });
}

for (const system of ['light', 'dark'] as const) {
  for (const preference of ['light', 'dark', 'system', 'invalid', null, 'blocked'] as const) {
    test(`theme is applied before the app bundle loads (${preference ?? 'unset'}, system ${system})`, async ({ page }) => {
      const errors: string[] = [];
      page.on('pageerror', (error) => errors.push(error.message));
      await page.emulateMedia({ colorScheme: system });
      await page.addInitScript((saved) => {
        if (saved === 'blocked') {
          Object.defineProperty(window, 'localStorage', {
            get() {
              throw new DOMException('Storage is blocked', 'SecurityError');
            },
          });
        } else if (saved === null) {
          localStorage.removeItem('regressify-theme');
        } else {
          localStorage.setItem('regressify-theme', saved);
        }
      }, preference);
      const selected = preference === 'light' || preference === 'dark' ? preference : 'system';
      const resolved = selected === 'system' ? system : selected;
      const appBundle = /\/(?:src\/main\.tsx|_snapshot\/assets\/app\.js)(?:\?|$)/;
      await page.route(appBundle, (route) => route.abort());
      await page.goto('/');
      await expect(page.locator('#root')).toBeEmpty();
      await expect(page.locator('html')).toHaveAttribute('data-theme', resolved);
      await expect(page.locator('html')).toHaveCSS('color-scheme', resolved);
      await expect(page.locator('html')).toHaveCSS('background-color', resolved === 'dark' ? 'rgb(21, 25, 25)' : 'rgb(247, 248, 248)');
      await page.unroute(appBundle);
      await page.reload();
      await expect(page.getByRole('button', { name: `${selected} theme`, exact: false })).toHaveAttribute('aria-pressed', 'true');
      await expect(page.locator('html')).toHaveAttribute('data-theme', resolved);
      if (preference === 'blocked') {
        const next = resolved === 'dark' ? 'light' : 'dark';
        await page.getByRole('button', { name: `${next} theme`, exact: false }).click();
        await expect(page.locator('html')).toHaveAttribute('data-theme', next);
      } else {
        expect(await page.evaluate(() => localStorage.getItem('regressify-theme'))).toBe(preference);
      }
      expect(errors).toEqual([]);
    });
  }
}

test('theme choices sync across tabs and only system mode follows OS changes', async ({ page, context }) => {
  const other = await context.newPage();
  await page.emulateMedia({ colorScheme: 'light' });
  await other.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  await other.goto('/');
  await expect(page.getByRole('button', { name: 'System theme' })).toHaveAttribute('aria-pressed', 'true');
  await expect(other.getByRole('button', { name: 'System theme' })).toHaveAttribute('aria-pressed', 'true');
  await page.getByRole('button', { name: 'Dark theme' }).click();
  await expect(other.getByRole('button', { name: 'Dark theme' })).toHaveAttribute('aria-pressed', 'true');
  await other.emulateMedia({ colorScheme: 'light' });
  await expect(other.locator('html')).toHaveAttribute('data-theme', 'dark');
  await other.getByRole('button', { name: 'Light theme' }).click();
  await expect(page.getByRole('button', { name: 'Light theme' })).toHaveAttribute('aria-pressed', 'true');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await other.getByRole('button', { name: 'System theme' }).click();
  await expect(page.getByRole('button', { name: 'System theme' })).toHaveAttribute('aria-pressed', 'true');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(other.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('theme storage resets restore system mode without being overwritten', async ({ page, context }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  const other = await context.newPage();
  await other.goto('/');
  for (const reset of ['invalid', 'remove', 'clear'] as const) {
    await page.getByRole('button', { name: 'Dark theme' }).click();
    await expect(other.getByRole('button', { name: 'Dark theme' })).toHaveAttribute('aria-pressed', 'true');
    await other.evaluate((mode) => {
      if (mode === 'clear') localStorage.clear();
      else if (mode === 'remove') localStorage.removeItem('regressify-theme');
      else localStorage.setItem('regressify-theme', 'invalid');
    }, reset);
    await expect(page.getByRole('button', { name: 'System theme' })).toHaveAttribute('aria-pressed', 'true');
    await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
    expect(await page.evaluate(() => localStorage.getItem('regressify-theme'))).toBe(reset === 'invalid' ? 'invalid' : null);
  }
});

test('overview is responsive, accessible, sortable, and theme-aware', async ({ page }, info) => {
  const errors: string[] = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto('/index.html');
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole('heading', { name: 'A clear view of every change.' })).toBeVisible();
  await expect(page.locator('.results-table tbody tr')).toHaveCount(7);
  await page.getByRole('button', { name: 'Suite', exact: true }).click();
  await expect(page.locator('.results-table th').first()).toHaveAttribute('aria-sort', 'descending');
  await screenshot(page, info.project.name, 'overview-light');
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.getByRole('button', { name: 'Dark theme' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await screenshot(page, info.project.name, 'overview-dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByRole('button', { name: 'System theme' }).click();
  await page.emulateMedia({ colorScheme: 'light' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  expect(errors).toEqual([]);
});

test('logo and favicon use the shared desktop artwork', async ({ page }) => {
  await page.goto('/');
  const logo = page.locator('.brand img');
  await expect(logo).toBeVisible();
  const source = await logo.getAttribute('src');
  if (!source) throw new Error('The application logo is missing');
  const artwork = readFileSync(resolve(import.meta.dirname, '../../assets/icon.svg'), 'utf8');
  expect(new URL(source, page.url()).searchParams.get('v')).toBe(getAssetVersion(artwork));
  await expect(page.locator('head link[rel="icon"]')).toHaveAttribute('href', source);
  const response = await page.request.get(source);
  expect(response.ok()).toBe(true);
  expect(await response.text()).toBe(artwork);
  await expect.poll(() => logo.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBeGreaterThan(0);
});

test('page titles follow suite navigation, deep links, and the overview', async ({ page, isMobile }) => {
  await page.goto('/');
  await expect(page.locator('.results-table tbody tr')).toHaveCount(7);
  const reportTitle = await page.title();
  await page.locator('.suite-name').filter({ hasText: 'workspace-overview' }).click();
  await expect(page).toHaveTitle(`workspace-overview | ${reportTitle}`);
  await page.locator('.capture-card').first().click();
  await expect(page).toHaveTitle(`workspace-overview | ${reportTitle}`);
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await page
    .getByRole('navigation', { name: 'Test suites', exact: true })
    .getByRole('link', { name: /orders-and-checkout/ })
    .click();
  await expect(page).toHaveTitle(`orders-and-checkout | ${reportTitle}`);
  await page.reload();
  await expect(page).toHaveTitle(`orders-and-checkout | ${reportTitle}`);
  await page.getByRole('link', { name: 'Regressify overview' }).click();
  await expect(page).toHaveTitle(reportTitle);
});

test('overview centers metrics and result columns with aligned suite labels', async ({ page }, info) => {
  await page.goto('/');
  await expect(page.locator('.results-table tbody tr')).toHaveCount(7);
  await page.evaluate(() => document.fonts.ready);
  const offsets = await page.evaluate(() => {
    const center = (bounds: DOMRect) => bounds.left + bounds.width / 2;
    const metrics = Array.from(document.querySelectorAll('.metrics > div')).flatMap((column) => Array.from(column.children).map((content) => Math.abs(center(content.getBoundingClientRect()) - center(column.getBoundingClientRect()))));
    const cells = Array.from(document.querySelectorAll('.results-table tr')).flatMap((row) =>
      Array.from(row.children)
        .slice(1, 4)
        .filter((cell) => cell.getBoundingClientRect().width > 0)
        .map((cell) => {
          const content = cell.querySelector('button, .status-badge');
          const range = document.createRange();
          range.selectNodeContents(cell);
          return Math.abs(center(content?.getBoundingClientRect() || range.getBoundingClientRect()) - center(cell.getBoundingClientRect()));
        }),
    );
    const heading = document.querySelector('.results-table th:first-child button');
    const names = Array.from(document.querySelectorAll('.suite-name')).map((name) => {
      const text = Array.from(name.childNodes).find((node) => node.nodeType === Node.TEXT_NODE && node.textContent?.trim());
      if (!text || !heading) throw new Error('Suite label is missing');
      const range = document.createRange();
      range.selectNodeContents(text);
      return Math.abs((range.getClientRects()[0]?.left || 0) - heading.getBoundingClientRect().left);
    });
    return { metrics, cells, names };
  });
  for (const [group, values] of Object.entries(offsets)) {
    expect(values.length).toBeGreaterThan(0);
    for (const offset of values) expect(offset, `${group} alignment offset`).toBeLessThan(1);
  }
  await screenshot(page, info.project.name, 'overview-alignment');
});

test('navigation stays in the SPA, preserves filters, and refreshes deep links', async ({ page, isMobile }, info) => {
  await page.goto('/');
  let navigations = 0;
  page.on('request', (request) => {
    if (request.resourceType() === 'document') navigations++;
  });
  await page.locator('.suite-name').filter({ hasText: 'workspace-overview' }).click();
  await expect(page).toHaveURL(/\/suites\/workspace-overview$/);
  await expect(page.locator('.capture-card')).toHaveCount(4);
  await page.getByRole('searchbox', { name: 'Search captures' }).fill('Workspace');
  await expect(page.locator('.capture-card')).toHaveCount(1);
  await page.locator('.capture-card').click();
  await expect(page).toHaveURL(/\/images\/overview__desktop.png\?q=Workspace$/);
  await expect(page.getByRole('region', { name: 'Zoomable comparison images' })).toBeVisible();
  expect(navigations).toBe(0);
  await page.locator('.back-link').click();
  await expect(page.getByRole('searchbox', { name: 'Search captures' })).toHaveValue('Workspace');
  await page.getByRole('searchbox', { name: 'Search captures' }).fill('');
  await screenshot(page, info.project.name, 'suite-grid');
  await page.getByRole('button', { name: 'List view' }).click();
  await expect(page.locator('.capture-list')).toBeVisible();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await page
    .getByRole('navigation', { name: 'Test suites', exact: true })
    .getByRole('link', { name: /orders-and-checkout/ })
    .click();
  await expect(page.getByRole('heading', { name: 'orders-and-checkout', exact: true })).toBeVisible();
  await page.reload();
  await expect(page.locator('.capture-card')).toHaveCount(4);
  if (!isMobile) {
    await page.getByRole('button', { name: 'Toggle suite index' }).click();
    await expect(page.locator('.suite-panel')).toHaveCount(0);
  }
});

test('suite list compares reference, test, and available diff captures side by side', async ({ page }, info) => {
  await page.goto('/suites/workspace-overview?view=list');
  const capture = page.locator('.capture-list .capture-card').first();
  const lanes = capture.locator('.capture-lane');
  await expect(lanes).toHaveCount(3);
  await expect(lanes.locator('figcaption')).toHaveText(['Reference', 'Test', 'Diff']);
  for (const image of await lanes.getByRole('img').all()) {
    await expect(image).toBeVisible();
    await expect(image).toHaveCSS('object-fit', 'contain');
    await expect.poll(() => image.evaluate((element: HTMLImageElement) => element.naturalWidth)).toBeGreaterThan(0);
  }
  const bounds = await lanes.evaluateAll((elements) =>
    elements.map((element) => {
      const { top, left, right, width } = element.getBoundingClientRect();
      return { top, left, right, width };
    }),
  );
  for (const [index, lane] of bounds.entries()) {
    expect(lane.width).toBeGreaterThan(50);
    expect(Math.abs(lane.top - bounds[0].top)).toBeLessThan(1);
    expect(Math.abs(lane.width - bounds[0].width)).toBeLessThan(1);
    if (index > 0) expect(lane.left).toBeGreaterThanOrEqual(bounds[index - 1].right);
  }
  await screenshot(page, info.project.name, 'suite-list');
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await capture.click();
  await expect(page).toHaveURL(/\/images\/overview__desktop.png\?view=list$/);
  await page.locator('.back-link').click();
  await expect(page.getByRole('button', { name: 'List view' })).toHaveAttribute('aria-pressed', 'true');
  await page.goto('/suites/customer-accounts?view=list');
  await expect(lanes).toHaveCount(2);
  await expect(lanes.locator('figcaption')).toHaveText(['Reference', 'Test']);
  await page.getByRole('button', { name: 'Grid view' }).click();
  await expect(page.locator('.capture-grid .capture-card').first().getByRole('img')).toHaveCount(1);
});

test('suite list preserves missing-image lanes and wraps long capture labels', async ({ page }) => {
  const label = 'LongCaptureLabel'.repeat(20);
  let unavailableImage = '';
  await page.route('**/_snapshot/data/suites/**', async (route) => {
    const response = await route.fetch();
    const data = await response.json();
    unavailableImage = new URL(`/${data.tests[0].test}`, route.request().url()).href;
    data.tests[0].reference = null;
    data.tests[0].label = label;
    await route.fulfill({ response, json: data });
  });
  await page.route('**/_snapshot/images/**', (route) => (route.request().url() === unavailableImage ? route.fulfill({ status: 404 }) : route.continue()));
  await page.goto('/suites/workspace-overview?view=list');
  const capture = page.locator('.capture-list .capture-card').first();
  await expect(capture.locator('.capture-lane')).toHaveCount(3);
  await capture.locator('.capture-comparison').scrollIntoViewIfNeeded();
  await expect(capture.getByText('No image', { exact: true })).toBeVisible();
  await expect(capture.getByText('Image unavailable', { exact: true })).toBeVisible();
  await expect(capture.getByRole('img', { name: `${label} diff`, exact: true })).toBeVisible();
  for (const width of [320, 768, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    await assertLayout(page);
    const overflows = await capture.evaluate((element) => [element, ...element.querySelectorAll('h2, .capture-lane-preview')].some((node) => node.scrollWidth > node.clientWidth));
    expect(overflows).toBe(false);
  }
});

test('review navigation lists suites and scenarios on suite and image pages', async ({ page, isMobile }) => {
  await page.goto('/suites/workspace-overview');
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(page.getByRole('navigation', { name: 'Test suites', exact: true })).toBeVisible();
  const scenarios = page.getByRole('navigation', { name: 'Test scenarios', exact: true });
  await expect(scenarios).toBeVisible();
  await expect(scenarios.getByRole('link')).toHaveCount(4);
  await scenarios.getByRole('link', { name: /Scenario 2/ }).click();
  await expect(page).toHaveURL(/\/images\/recent-orders__mobile.png$/);
  await expect(page.getByRole('heading', { name: 'Recent orders / Mobile', exact: true })).toBeVisible();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(scenarios.getByRole('link', { name: /Scenario 2/ })).toHaveAttribute('aria-current', 'page');
  await page.getByRole('searchbox', { name: 'Find a scenario' }).fill('Scenario 3');
  await expect(scenarios.getByRole('link')).toHaveCount(1);
});

test('URL-only scenario names stay friendly while capture routes keep their IDs', async ({ page, isMobile }, info) => {
  await page.goto('/suites/search-and-navigation');
  await expect(page.locator('.capture-card h2')).toHaveText(['Men / Running shoes / Desktop / Full page', 'Women / Running shoes / Mobile / Full page', 'Help center / Desktop / Full page', 'Men / Running shoes / Mobile / Full page']);
  await screenshot(page, info.project.name, 'url-only-scenarios');
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  const scenarios = page.getByRole('navigation', { name: 'Test scenarios', exact: true });
  await expect(scenarios.locator('.scenario-group')).toHaveCount(1);
  await expect(scenarios.locator('summary')).toContainText('Men / Running shoes');
  await expect(scenarios.getByRole('link')).toHaveCount(4);
  const scenarioId = getAssetVersion('https://example.com/men/running-shoes');
  await expect(scenarios).not.toContainText(scenarioId);
  await expect(scenarios).not.toContainText('https://');
  await scenarios.getByRole('link', { name: 'Men / Running shoes / Desktop / Full page' }).click();
  await expect(page).toHaveURL(new RegExp(`/images/${scenarioId}_desktop__FINAL\\.png$`));
  await expect(page.getByRole('heading', { name: 'Men / Running shoes / Desktop / Full page', exact: true })).toBeVisible();
  await screenshot(page, info.project.name, 'url-only-image');
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Men / Running shoes / Desktop / Full page', exact: true })).toBeVisible();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await page.getByRole('searchbox', { name: 'Find a scenario' }).fill('Women / Running shoes');
  await expect(scenarios.getByRole('link')).toHaveCount(1);
  await expect(scenarios.getByRole('link')).toHaveText('Women / Running shoes');
});

test('scenario navigation is restricted to the selected suite and groups its captures', async ({ page, isMobile }) => {
  await page.route('**/_snapshot/data/suites/**', async (route) => {
    const response = await route.fetch();
    const data = await response.json();
    data.tests.forEach((capture: { scenario: string }, index: number) => {
      capture.scenario = `${data.id} / ${index < 2 ? 'First scenario' : 'Second scenario'}`;
    });
    await route.fulfill({ response, json: data });
  });
  await page.goto('/suites/workspace-overview');
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  const scenarios = page.getByRole('navigation', { name: 'Test scenarios', exact: true });
  await expect(scenarios.locator('.scenario-group')).toHaveCount(2);
  await expect(scenarios).toContainText('workspace-overview / First scenario');
  await page
    .getByRole('navigation', { name: 'Test suites', exact: true })
    .getByRole('link', { name: /orders-and-checkout/ })
    .click();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(scenarios).toContainText('orders-and-checkout / First scenario');
  await expect(scenarios).not.toContainText('workspace-overview');
  for (const link of await scenarios.getByRole('link').all()) await expect(link).toHaveAttribute('href', /^\/suites\/orders-and-checkout\/images\//);
  await page
    .getByRole('navigation', { name: 'Test suites', exact: true })
    .getByRole('link', { name: /empty-suite/ })
    .click();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(scenarios).toContainText('No scenarios');
  await expect(scenarios.getByRole('link')).toHaveCount(0);
});

test('review panes resize and scroll independently', async ({ page, isMobile }) => {
  test.skip(isMobile, 'Mobile uses the review navigation drawer.');
  await page.setViewportSize({ width: 1200, height: 520 });
  await page.goto('/suites/workspace-overview');
  await expect(page.locator('.capture-card')).toHaveCount(4);
  const divider = page.getByRole('separator', { name: 'Resize review navigation' });
  const panel = page.locator('.suite-panel');
  const start = await divider.boundingBox();
  const original = await panel.boundingBox();
  if (!start || !original) throw new Error('Review divider is unavailable');
  await page.mouse.move(start.x + start.width / 2, start.y + 100);
  await page.mouse.down();
  await page.mouse.move(start.x + 104, start.y + 100, { steps: 5 });
  await page.mouse.up();
  await expect.poll(async () => (await panel.boundingBox())?.width || 0).toBeGreaterThan(original.width + 70);
  await divider.focus();
  await divider.press('Home');
  await expect(divider).toHaveAttribute('aria-valuenow', '220');
  await divider.press('ArrowRight');
  await expect(divider).toHaveAttribute('aria-valuenow', '236');
  const sidebarScroll = page.locator('.suite-panel-scroll');
  const reviewScroll = page.locator('.review-scroll');
  await sidebarScroll.hover({ position: { x: 60, y: 160 } });
  await page.mouse.wheel(0, 300);
  await expect.poll(() => sidebarScroll.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await reviewScroll.evaluate((element) => element.scrollTop)).toBe(0);
  const sidebarTop = await sidebarScroll.evaluate((element) => element.scrollTop);
  await reviewScroll.hover({ position: { x: 250, y: 200 } });
  await page.mouse.wheel(0, 400);
  await expect.poll(() => reviewScroll.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await sidebarScroll.evaluate((element) => element.scrollTop)).toBe(sidebarTop);
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
  await assertLayout(page);
  await page.reload();
  await expect(divider).toHaveAttribute('aria-valuenow', '236');
  await divider.focus();
  await divider.press('End');
  await expect(divider).toHaveAttribute('aria-valuenow', '480');
  await page.setViewportSize({ width: 980, height: 520 });
  await expect(divider).toHaveAttribute('aria-valuenow', '441');
  await page.setViewportSize({ width: 390, height: 740 });
  await expect(divider).toHaveCount(0);
  await page.setViewportSize({ width: 1200, height: 520 });
  await expect(divider).toHaveAttribute('aria-valuenow', '480');
});

test('suite failure arrows scroll to failed captures without leaving the suite', async ({ page }) => {
  await page.goto('/suites/workspace-overview?view=list');
  const previous = page.getByRole('button', { name: 'Previous failed comparison', exact: true });
  const next = page.getByRole('button', { name: 'Next failed comparison', exact: true });
  await expect(previous).toBeDisabled();
  for (let index = 0; index < 4; index++) {
    await next.click();
    await expect(page.getByLabel('Failed comparison position')).toHaveText(`${index + 1} / 4 failed`);
    const selected = page.locator('.capture-card[data-active-failure="true"]');
    await expect(selected).toBeFocused();
    await expect(selected).toContainText('Failed');
    await expect(page).toHaveURL(/\/suites\/workspace-overview\?view=list$/);
  }
  await expect(next).toBeDisabled();
  await previous.click();
  await expect(page.getByLabel('Failed comparison position')).toHaveText('3 / 4 failed');
  await page.getByRole('searchbox', { name: 'Search captures' }).fill('Recent orders');
  await expect(previous).toBeDisabled();
  await next.click();
  await expect(page.locator('.capture-card[data-active-failure="true"]')).toContainText('Recent orders');
  await expect(next).toBeDisabled();
  await page.goto('/suites/customer-accounts');
  await expect(previous).toBeDisabled();
  await expect(next).toBeDisabled();
});

test('capture information opens on hover or tap and remains selectable', async ({ page, isMobile }, info) => {
  await page.goto('/suites/workspace-overview/images/overview__desktop.png');
  const trigger = page.getByRole('button', { name: 'Capture information', exact: true });
  const bubble = page.getByRole('dialog', { name: 'Capture information', exact: true });
  await expect(bubble).toHaveCount(0);
  if (isMobile) await trigger.tap();
  else await trigger.hover();
  await expect(bubble).toBeVisible();
  await expect(bubble).toContainText('Difference');
  await expect(bubble).toContainText('Threshold');
  await expect(bubble).toContainText('overview__desktop.png');
  if (!isMobile) {
    await bubble.hover();
    await expect(bubble).toBeVisible();
    const text = bubble.getByText('overview__desktop.png', { exact: true });
    const bounds = await text.boundingBox();
    if (!bounds) throw new Error('Copyable capture label is unavailable');
    await page.mouse.move(bounds.x + 2, bounds.y + bounds.height / 2);
    await page.mouse.down();
    await page.mouse.move(bounds.x + Math.min(bounds.width - 2, 175), bounds.y + bounds.height / 2, { steps: 8 });
    await page.mouse.up();
    await expect.poll(() => page.evaluate(() => window.getSelection()?.toString() || '')).not.toBe('');
    const selectedText = await page.evaluate(() => window.getSelection()?.toString() || '');
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.keyboard.press('ControlOrMeta+C');
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe(selectedText);
    await expect(bubble).toBeVisible();
  }
  await screenshot(page, info.project.name, 'capture-information');
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await page.getByRole('button', { name: 'Close capture information' }).click();
  await expect(bubble).toHaveCount(0);
  await trigger.focus();
  await expect(bubble).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(bubble).toHaveCount(0);
  await trigger.click();
  await page.getByRole('heading', { name: 'Workspace overview / Desktop', exact: true }).hover();
  await expect(bubble).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(bubble).toHaveCount(0);
});

test('image modes, zoom, downloads, and previous/next work', async ({ page, isMobile }, info) => {
  await page.goto('/suites/workspace-overview/images/overview__desktop.png');
  await expect(page.getByRole('heading', { name: 'Workspace overview / Desktop', exact: true })).toBeVisible();
  await expect.poll(() => page.locator('.image-world img').evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  await screenshot(page, info.project.name, 'comparison-side');
  await page.getByRole('button', { name: 'Actual size', exact: true }).click();
  await expect(page.getByLabel('Zoom level')).toHaveText('100%');
  await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
  await expect(page.getByLabel('Zoom level')).not.toHaveText('100%');
  await page.getByRole('button', { name: 'Fit images', exact: true }).click();
  const mode = async (value: string, label: string) => {
    if (isMobile) await page.getByRole('combobox', { name: 'Comparison mode' }).selectOption(value);
    else await page.getByRole('button', { name: label, exact: true }).click();
    await expect(page.locator(`.image-world.mode-${value}`)).toBeVisible();
  };
  await mode('wipe', 'Wipe');
  if (!isMobile) {
    const bounds = await page.locator('.image-world').boundingBox();
    if (!bounds) throw new Error('Wipe image is not visible');
    await page.mouse.move(bounds.x + bounds.width * 0.25, bounds.y + bounds.height * 0.5);
    await expect(page.getByRole('slider', { name: 'Wipe position' })).toHaveValue('25');
    await page.mouse.move(bounds.x + bounds.width * 0.75, bounds.y + bounds.height * 0.5);
    await expect(page.getByRole('slider', { name: 'Wipe position' })).toHaveValue('75');
  }
  await page.getByRole('slider', { name: 'Wipe position' }).focus();
  await page.getByRole('slider', { name: 'Wipe position' }).press('Home');
  await expect(page.getByRole('slider', { name: 'Wipe position' })).toHaveValue('0');
  await mode('overlay', 'Overlay');
  await page.getByRole('slider', { name: 'Test image opacity' }).press('End');
  await expect(page.getByRole('slider', { name: 'Test image opacity' })).toHaveValue('100');
  await mode('diff', 'Diff');
  await screenshot(page, info.project.name, 'comparison-diff');
  await mode('reference', 'Reference');
  await mode('test', 'Test');
  const download = page.waitForEvent('download');
  await page.getByRole('link', { name: 'Download test', exact: true }).click();
  expect((await download).suggestedFilename()).toContain('test.png');
  await page.getByRole('link', { name: 'Next image', exact: true }).click();
  await expect(page).toHaveURL(/recent-orders__mobile.png/);
  await page.getByRole('link', { name: 'Previous image', exact: true }).click();
  await expect(page).toHaveURL(/overview__desktop.png/);
  await page.reload();
  await expect(page.locator('.mode-test')).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
});

test('scrolling over images moves only the review page without changing zoom', async ({ page }) => {
  await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
  await expect(page.locator('.comparison-lane img')).toHaveCount(3);
  await expect.poll(() => page.locator('.comparison-lane img').evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  await page.getByRole('button', { name: 'Actual size', exact: true }).click();
  await expect(page.getByLabel('Zoom level')).toHaveText('100%');
  const content = page.locator('.review-scroll');
  await content.evaluate((element) => {
    element.scrollTop = 0;
  });
  await page.locator('.zoom-canvas').hover({ position: { x: 80, y: 80 } });
  await page.mouse.wheel(0, 300);
  await expect.poll(() => content.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  await expect(page.getByLabel('Zoom level')).toHaveText('100%');
  const bottom = await content.evaluate((element) => element.scrollTop);
  await page.mouse.wheel(0, -120);
  await expect.poll(() => content.evaluate((element) => element.scrollTop)).toBeLessThan(bottom);
  await expect(page.getByLabel('Zoom level')).toHaveText('100%');
  expect(await page.locator('.zoom-canvas').evaluate((element) => element.scrollTop)).toBe(0);
  expect(await page.evaluate(() => window.scrollY)).toBe(0);
});

test('side by side includes a third lane only when a diff is available', async ({ page, isMobile }, info) => {
  await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
  await expect(page.locator('.comparison-lane')).toHaveCount(3);
  await expect(page.locator('.suite-panel')).toHaveCount(isMobile ? 0 : 1);
  await expect(page.locator('.review-details')).toHaveCount(0);
  await expect(page.locator('.canvas-labels')).toHaveCount(0);
  await expect(page.getByRole('img', { name: 'Diff: Workspace overview / Desktop', exact: true })).toBeVisible();
  const lanes = await page.locator('.comparison-lane').evaluateAll((elements) =>
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
        const canvas = document.querySelector('.zoom-canvas')?.getBoundingClientRect();
        const lane = document.querySelector('.comparison-lane')?.getBoundingClientRect();
        return canvas && lane ? Math.abs(lane.y - canvas.y) : Number.POSITIVE_INFINITY;
      }),
    )
    .toBeLessThan(1);
  await screenshot(page, info.project.name, 'comparison-three-lanes');
  await page.goto('/suites/customer-accounts/images/overview__desktop.png?mode=side');
  await expect(page.locator('.comparison-lane')).toHaveCount(2);
  await expect(page.locator('.canvas-labels')).toHaveCount(0);
});

test('comparison images are centered without labels and oversized images stay reachable', async ({ page, isMobile }, info) => {
  await page.route('**/_snapshot/data/suites/**', async (route) => {
    const response = await route.fetch();
    const data = await response.json();
    if (data.id === 'workspace-overview') data.tests[0].reference = data.tests[1].reference;
    await route.fulfill({ response, json: data });
  });
  await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
  await expect(page.locator('.comparison-lane img')).toHaveCount(3);
  await expect.poll(() => page.locator('.comparison-lane img').evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  for (const [mode, label] of [
    ['side', 'Side by side'],
    ['reference', 'Reference'],
    ['test', 'Test'],
    ['diff', 'Diff'],
    ['wipe', 'Wipe'],
    ['overlay', 'Overlay'],
  ]) {
    if (isMobile) await page.getByRole('combobox', { name: 'Comparison mode' }).selectOption(mode || 'side');
    else await page.getByRole('button', { name: label, exact: true }).click();
    await page.getByRole('button', { name: 'Zoom out', exact: true }).click();
    await expect(page.locator('.canvas-labels')).toHaveCount(0);
    await expect
      .poll(() =>
        page.locator('.zoom-canvas').evaluate((canvas) => {
          const bounds = canvas.getBoundingClientRect();
          const parent = canvas.closest('.comparison-viewer')?.getBoundingClientRect();
          return parent ? Math.abs(bounds.left + bounds.width / 2 - parent.left - parent.width / 2) : Number.POSITIVE_INFINITY;
        }),
      )
      .toBeLessThan(1);
    if (mode === 'side') {
      for (const lane of await page.locator('.comparison-lane').all()) {
        const difference = await lane.evaluate((element) => {
          const bounds = element.getBoundingClientRect();
          const image = element.querySelector('img')?.getBoundingClientRect();
          return image ? Math.abs(image.left + image.width / 2 - bounds.left - bounds.width / 2) : Number.POSITIVE_INFINITY;
        });
        expect(difference).toBeLessThan(1);
      }
      await screenshot(page, info.project.name, 'comparison-centered');
    }
  }
  if (isMobile) await page.getByRole('combobox', { name: 'Comparison mode' }).selectOption('side');
  else await page.getByRole('button', { name: 'Side by side', exact: true }).click();
  await page.getByRole('button', { name: 'Actual size', exact: true }).click();
  const edges = await page.locator('.zoom-canvas').evaluate((canvas) => ({
    image: canvas.getBoundingClientRect().left,
    viewer: canvas.closest('.comparison-viewer')?.getBoundingClientRect().left,
  }));
  expect(edges.image).toBe(edges.viewer);
});

test('empty results, missing images, errors, and unknown routes are explicit', async ({ page, isMobile }, info) => {
  await page.goto('/');
  await page.getByRole('searchbox', { name: 'Search suites' }).fill('no-such-suite');
  await expect(page.getByRole('heading', { name: 'No matching suites' })).toBeVisible();
  await page.goto('/suites/empty-suite');
  await expect(page.getByRole('heading', { name: 'No matching captures' })).toBeVisible();
  await page.goto('/suites/product-catalog/images/full-page__mobile.png');
  await page.getByRole('button', { name: 'Capture information', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Navigation timed out');
  await page.keyboard.press('Escape');
  await expect(page.getByText('No image', { exact: true })).toBeVisible();
  if (!isMobile) await expect(page.getByRole('button', { name: 'Wipe', exact: true })).toBeDisabled();
  await screenshot(page, info.project.name, 'comparison-error');
  await page.goto('/suites/missing');
  await expect(page.getByRole('heading', { name: 'Suite not found' })).toBeVisible();
  await page.goto('/suites/workspace-overview/images/missing');
  await expect(page.getByRole('heading', { name: 'Image not found' })).toBeVisible();
});

test('invalid report data fails visibly and can be retried', async ({ page }) => {
  await page.route('**/_snapshot/**', async (route) => {
    if (new URL(route.request().url()).pathname.endsWith('.json')) await route.fulfill({ contentType: 'application/json', body: '{"version":99}' });
    else await route.continue();
  });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Snapshot unavailable' })).toBeVisible();
  await page.unroute('**/_snapshot/**');
  await page.getByRole('button', { name: 'Retry', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'A clear view of every change.' })).toBeVisible();
});

test('untrusted Markdown is not executable markup or a remote image request', async ({ page }) => {
  await page.route('**/_snapshot/**', async (route) => {
    if (!new URL(route.request().url()).pathname.endsWith('.json')) return route.continue();
    const response = await route.fetch();
    const data = await response.json();
    if (data.version === 2) data.subheading = '<img src="https://untrusted.invalid/leak" onerror="window.compromised=true"> [Unsafe](javascript:alert(1)) **Safe text**';
    await route.fulfill({ response, json: data });
  });
  await page.goto('/');
  await expect(page.locator('.report-subheading')).toContainText('Safe text');
  await expect(page.locator('.report-subheading img')).toHaveCount(0);
  await expect(page.locator('a[href^="javascript:"]')).toHaveCount(0);
  expect(await page.evaluate(() => 'compromised' in window)).toBe(false);
});

test('empty snapshots and very long report text fit narrow and wide screens', async ({ page }) => {
  await page.route('**/_snapshot/**', async (route) => {
    if (!new URL(route.request().url()).pathname.endsWith('.json')) return route.continue();
    const response = await route.fetch();
    const data = await response.json();
    if (data.version === 2) {
      data.heading = 'LongUnbrokenReportHeading'.repeat(8);
      data.subheading = `**Release** / ${'VeryLongBuildIdentifier'.repeat(8)}`;
      data.suites = [];
    }
    await route.fulfill({ response, json: data });
  });
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'No comparisons yet' })).toBeVisible();
  for (const width of [320, 768, 1920]) {
    await page.setViewportSize({ width, height: 900 });
    await assertLayout(page);
  }
});

test('every snapshot payload has a content version on a stable path', async ({ page }) => {
  const requests = new Set<string>();
  page.on('request', (request) => {
    if (new URL(request.url()).pathname.startsWith('/_snapshot/')) requests.add(request.url());
  });
  await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
  await expect(page.locator('.comparison-lane img')).toHaveCount(3);
  await expect.poll(() => page.locator('.comparison-lane img').evaluateAll((images) => images.every((image) => (image as HTMLImageElement).naturalWidth > 0))).toBe(true);
  expect([...requests].some((url) => new URL(url).pathname.endsWith('/manifest.json'))).toBe(true);
  expect([...requests].filter((url) => new URL(url).pathname.endsWith('.png')).length).toBeGreaterThanOrEqual(3);
  for (const resource of requests) {
    const url = new URL(resource);
    expect(url.searchParams.get('v')).toMatch(/^[A-Za-z0-9_-]{20}$/);
    expect(url.pathname).not.toMatch(/[a-f0-9]{64}\.(json|png)$/);
    const response = await page.request.get(resource);
    expect(response.ok()).toBe(true);
    expect(url.searchParams.get('v')).toBe(getAssetVersion(await response.body()));
  }
});
