import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import AxeBuilder from '@axe-core/playwright';
import { expect, type Page, test } from '@playwright/test';
import { PNG } from 'pngjs';
import { getAssetVersion } from '../scripts/version-assets';

const overviewSuites = [
  { displayName: 'All passing', passed: 12, failed: 0, errors: 0 },
  { displayName: 'Mixed results', passed: 9, failed: 1, errors: 0 },
  { displayName: 'Rounded pass rate', passed: 9999, failed: 1, errors: 0 },
  { displayName: 'All failing', passed: 0, failed: 4, errors: 0 },
  { displayName: 'Engine error', passed: 3, failed: 1, errors: 1 },
  { displayName: 'Empty suite', passed: 0, failed: 0, errors: 0 },
].map((suite) => ({ ...suite, total: suite.passed + suite.failed }));

async function mockOverview(page: Page) {
  await page.route('**/_snapshot/data/manifest.json?*', async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    manifest.suites = overviewSuites.map((suite, index) => ({
      ...manifest.suites[index],
      ...suite,
      id: `summary-${index}`,
    }));
    await route.fulfill({ response, json: manifest });
  });
  await page.goto('/');
}

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
  await expect(page.locator('.metrics > div > span')).toHaveText(['Total tests', 'Passed', 'Failed', 'Pass rate']);
  await expect(page.getByRole('columnheader')).toHaveText(['Test suite', 'Total tests', 'Passed', 'Failed', 'Pass rate']);
  await page.getByRole('columnheader', { name: 'Test suite', exact: true }).getByRole('button').click();
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

test('pass-rate bars distinguish failing suites in light and dark themes', async ({ page }, info) => {
  await mockOverview(page);
  await expect(page.locator('.metrics strong')).toHaveText(['10030', '10023', '7', '99.9%']);
  for (const theme of ['Light', 'Dark']) {
    await page.getByRole('button', { name: `${theme} theme`, exact: true }).click();
    const neutralColor = await page.locator('.section-title .muted').evaluate((element) => getComputedStyle(element).color);
    for (const suite of overviewSuites) {
      const row = page.locator('.results-table tbody tr').filter({
        has: page.getByRole('link', { name: suite.displayName, exact: true }),
      });
      await expect(row.locator('td').nth(1)).toHaveText(String(suite.total));
      await expect(row.locator('td').nth(2)).toHaveText(String(suite.passed));
      await expect(row.locator('td').nth(3)).toHaveText(String(suite.failed));
      const fullPass = suite.total > 0 && suite.failed === 0;
      await expect(row.locator('.result-count.pass')).toHaveCount(fullPass ? 1 : 0);
      await expect(row.locator('.result-count.fail')).toHaveCount(suite.failed ? 1 : 0);
      const progress = row.locator('progress');
      const statusColor = await row.locator('.status-dot').evaluate((element) => getComputedStyle(element).backgroundColor);
      await expect(row.locator('.result-count').first()).toHaveCSS('color', fullPass ? statusColor : neutralColor);
      await expect(row.locator('.result-count').last()).toHaveCSS('color', suite.failed ? statusColor : neutralColor);
      await expect(progress).toHaveAttribute('value', String(suite.passed));
      await expect(progress).toHaveAttribute('max', String(suite.total || 1));
      const image = PNG.sync.read(await progress.screenshot());
      const sampleWidth = suite.passed ? (image.width * suite.passed) / suite.total : image.width;
      const offset = (Math.floor(image.height / 2) * image.width + Math.floor(sampleWidth / 2)) * 4;
      const color = [...image.data.subarray(offset, offset + 3)].join(', ');
      const expectedColor = suite.passed
        ? suite.failed
          ? neutralColor
          : statusColor
        : await row
            .locator('td')
            .first()
            .evaluate((element) => getComputedStyle(element).borderBottomColor);
      expect(`rgb(${color})`, `${suite.displayName} bar in ${theme} theme`).toBe(expectedColor);
    }
    await page.locator('.results-table-wrap').evaluate((element) => element.scrollTo(0, 0));
    await screenshot(page, info.project.name, `pass-rates-${theme.toLowerCase()}`);
  }
});

test('overview sorts every column in both directions and preserves filters', async ({ page }) => {
  await mockOverview(page);
  const links = page.locator('.results-table .suite-name');
  for (const { key, label } of [
    { key: 'name', label: 'Test suite' },
    { key: 'total', label: 'Total tests' },
    { key: 'passed', label: 'Passed' },
    { key: 'failed', label: 'Failed' },
    { key: 'rate', label: 'Pass rate' },
  ] as const) {
    const header = page.getByRole('columnheader', { name: label, exact: true });
    for (const direction of ['ascending', 'descending'] as const) {
      if (key !== 'name' || direction !== 'ascending') await header.getByRole('button').click();
      await expect(header).toHaveAttribute('aria-sort', direction);
      await expect(page.locator('.results-table th:not([aria-sort="none"])')).toHaveCount(1);
      const expected = [...overviewSuites].sort((first, second) => {
        const order = key === 'name' ? first.displayName.localeCompare(second.displayName) : key === 'rate' ? first.passed / (first.total || 1) - second.passed / (second.total || 1) : first[key] - second[key];
        return direction === 'ascending' ? order : -order;
      });
      await expect(links).toHaveText(expected.map((suite) => suite.displayName));
    }
  }
  await page.getByRole('searchbox', { name: 'Search suites' }).fill('e');
  await page
    .getByRole('group', { name: 'Filter results' })
    .getByRole('button', { name: /^Failed/ })
    .click();
  const passedHeader = page.getByRole('columnheader', { name: 'Passed', exact: true });
  await passedHeader.getByRole('button').click();
  await expect(passedHeader).toHaveAttribute('aria-sort', 'ascending');
  await expect(links).toHaveText(['Engine error', 'Mixed results', 'Rounded pass rate']);
  await passedHeader.getByRole('button').press('Enter');
  await expect(passedHeader).toHaveAttribute('aria-sort', 'descending');
  await page.reload();
  await expect(links).toHaveText(['Rounded pass rate', 'Mixed results', 'Engine error']);
  const params = new URL(page.url()).searchParams;
  expect(params.get('q')).toBe('e');
  expect(params.get('status')).toBe('fail');
  expect(params.get('sort')).toBe('passed');
  expect(params.get('direction')).toBe('desc');
  await expect(page.locator('.metrics strong')).toHaveText(['10030', '10023', '7', '99.9%']);
  await passedHeader.getByRole('button').press('Enter');
  await expect(passedHeader).toHaveAttribute('aria-sort', 'ascending');
  await expect(links).toHaveText(['Engine error', 'Mixed results', 'Rounded pass rate']);
  await assertLayout(page);
});

test('footer stays at the bottom on short and empty pages', async ({ page }, info) => {
  const viewport = page.viewportSize();
  if (!viewport) throw new Error('The viewport is not configured');
  await page.setViewportSize({ ...viewport, height: 1200 });
  for (const { path, name, content } of [
    { path: '/', name: 'overview', content: '.results-table tbody tr' },
    { path: '/suites/workspace-overview', name: 'suite', content: '.capture-card' },
    { path: '/suites/workspace-overview/not-found', name: 'not-found', content: null },
    { path: '/suites/not-a-suite', name: 'suite-not-found', content: null },
  ]) {
    await page.goto(path);
    if (content) {
      await expect(page.locator(content).first()).toBeVisible();
      await page.locator('#main').getByRole('searchbox').fill('footer-empty-results');
      await expect(page.locator(content)).toHaveCount(0);
    } else {
      await expect(page.locator('#main .state-panel')).toBeVisible();
    }
    await page.evaluate(() => document.fonts.ready);
    const footer = page.getByRole('contentinfo');
    await expect(footer).toBeVisible();
    await expect.poll(() => footer.evaluate((element) => Math.abs(element.getBoundingClientRect().bottom - window.innerHeight))).toBeLessThanOrEqual(1);
    const contentBottom = await page.locator('#main').evaluate((element) => element.getBoundingClientRect().bottom);
    expect(await footer.evaluate((element) => element.getBoundingClientRect().top)).toBeGreaterThanOrEqual(contentBottom - 1);
    await screenshot(page, info.project.name, `footer-${name}`);
  }
});

test('footer follows long content in document and review scrolling', async ({ page }) => {
  const viewport = page.viewportSize();
  if (!viewport) throw new Error('The viewport is not configured');
  await page.setViewportSize({ ...viewport, height: 480 });
  for (const { path, content } of [
    { path: '/', content: '.results-table tbody tr' },
    { path: '/suites/workspace-overview', content: '.capture-card' },
  ]) {
    await page.goto(path);
    await expect(page.locator(content).first()).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    const footer = page.getByRole('contentinfo');
    await expect.poll(() => footer.evaluate((element) => element.getBoundingClientRect().top)).toBeGreaterThanOrEqual(480);
    await footer.scrollIntoViewIfNeeded();
    await expect.poll(() => footer.evaluate((element) => element.getBoundingClientRect().bottom)).toBeLessThanOrEqual(481);
    const contentBottom = await page.locator('#main').evaluate((element) => element.getBoundingClientRect().bottom);
    expect(await footer.evaluate((element) => element.getBoundingClientRect().top)).toBeGreaterThanOrEqual(contentBottom - 1);
    if (path !== '/') expect(await page.evaluate(() => window.scrollY)).toBe(0);
  }
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
  await page.getByRole('button', { name: /^Test suite:/ }).click();
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

test('suite display names label navigation without changing routes', async ({ page, isMobile }, info) => {
  const displayName = 'Customer checkout / desktop <review>';
  await page.route('**/_snapshot/data/manifest.json?*', async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    for (const suite of manifest.suites) {
      if (suite.id === 'orders-and-checkout') suite.displayName = displayName;
    }
    await route.fulfill({ response, json: manifest });
  });
  await page.goto('/');
  await expect(page.locator('.results-table tbody tr')).toHaveCount(7);
  const reportTitle = await page.title();
  await page.getByRole('searchbox', { name: 'Search suites' }).fill('Customer checkout');
  await expect(page.locator('.suite-name')).toHaveCount(1);
  await expect(page.locator('.suite-name')).toHaveText(displayName);
  await page.locator('.suite-name').click();
  await expect(page).toHaveURL(/\/suites\/orders-and-checkout$/);
  await expect(page.getByRole('heading', { name: displayName, exact: true })).toBeVisible();
  await expect(page).toHaveTitle(`${displayName} | ${reportTitle}`);
  await screenshot(page, info.project.name, 'suite-display-name');
  await page.locator('.capture-card').first().click();
  await expect(page).toHaveURL(/\/suites\/orders-and-checkout\/images\//);
  await page.getByRole('button', { name: 'Capture information', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Capture information' }).locator("dd[title='orders-and-checkout']")).toHaveText(displayName);
  await page.getByRole('button', { name: 'Close capture information' }).click();
  await expect(page.getByRole('navigation', { name: 'Breadcrumb' }).getByRole('link', { name: displayName })).toBeVisible();
  await expect(page).toHaveTitle(`${displayName} | ${reportTitle}`);
  await page.reload();
  await expect(page).toHaveTitle(`${displayName} | ${reportTitle}`);
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  const selector = page.getByRole('button', { name: `Test suite: ${displayName}` });
  await expect(selector).toContainText(displayName);
  await selector.click();
  await page.getByRole('searchbox', { name: 'Find a suite' }).fill('Customer checkout');
  const navigation = page.getByRole('navigation', { name: 'Test suites', exact: true });
  await expect(navigation.getByRole('link')).toHaveCount(1);
  await expect(navigation.getByRole('link')).toContainText(displayName);
  await expect(navigation.getByRole('link')).toHaveAttribute('href', '/suites/orders-and-checkout');
  await expect(page.locator('review')).toHaveCount(0);
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
  await page.getByRole('button', { name: /^Test suite:/ }).click();
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

test('review navigation shows the current suite in a dropdown without shifting scenarios', async ({ page, isMobile }, info) => {
  await page.goto('/suites/workspace-overview');
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  const suites = page.getByRole('navigation', { name: 'Test suites', exact: true });
  const suiteSelector = page.getByRole('button', { name: 'Test suite: workspace-overview' });
  await expect(suiteSelector).toBeVisible();
  await expect(suiteSelector).toContainText('workspace-overview');
  await expect(suiteSelector).toHaveAttribute('aria-expanded', 'false');
  await expect(suites).toBeHidden();
  const scenarios = page.getByRole('navigation', { name: 'Test scenarios', exact: true });
  await expect(scenarios).toBeVisible();
  await expect(scenarios.getByRole('link')).toHaveCount(4);
  const scenarioTop = await scenarios.evaluate((element) => element.getBoundingClientRect().top);
  await screenshot(page, info.project.name, 'navigation-selector');
  await suiteSelector.click();
  await expect(suiteSelector).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('dialog', { name: 'Test suites', exact: true })).toBeVisible();
  await expect(suites).toBeVisible();
  await expect(suites.getByRole('link', { name: /workspace-overview/ })).toHaveAttribute('aria-current', 'page');
  await expect(page.getByRole('searchbox', { name: 'Find a suite' })).toBeFocused();
  expect(await scenarios.evaluate((element) => element.getBoundingClientRect().top)).toBe(scenarioTop);
  await screenshot(page, info.project.name, 'navigation-dropdown');
  await page.keyboard.press('Escape');
  await expect(suites).toBeHidden();
  await expect(suiteSelector).toBeFocused();
  await expect(scenarios).toBeVisible();
  await scenarios.getByRole('link', { name: /Scenario 2/ }).click();
  await expect(page).toHaveURL(/\/images\/recent-orders__mobile.png$/);
  await expect(page.getByRole('heading', { name: 'Recent orders / Mobile', exact: true })).toBeVisible();
  await expect(page.getByRole('region', { name: 'Zoomable comparison images' })).toBeVisible();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(suites).toBeHidden();
  await expect(scenarios.getByRole('link', { name: /Scenario 2/ })).toHaveAttribute('aria-current', 'page');
  await page.getByRole('searchbox', { name: 'Find a scenario' }).fill('Scenario 3');
  await expect(scenarios.getByRole('link')).toHaveCount(1);
  const scenario = scenarios.getByRole('link');
  await expect(scenario).toHaveAttribute('href', /^\/suites\/workspace-overview\/images\//);
  const comparisonUrl = new URL((await scenario.getAttribute('href')) || '', page.url()).href;
  await scenario.click();
  await expect(page).toHaveURL(comparisonUrl);
  await expect(page.getByRole('region', { name: 'Zoomable comparison images' })).toBeVisible();
  await page.reload();
  await expect(page).toHaveURL(comparisonUrl);
  await expect(page.getByRole('region', { name: 'Zoomable comparison images' })).toBeVisible();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(suiteSelector).toBeVisible();
  await expect(suites).toBeHidden();
  await expect(scenarios).toBeVisible();
});

test('review navigation dropdown searches, dismisses, and switches suites with the keyboard', async ({ page, isMobile }) => {
  await page.goto('/suites/workspace-overview');
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  const selector = page.getByRole('button', { name: /^Test suite:/ });
  const suites = page.getByRole('navigation', { name: 'Test suites', exact: true });
  const search = page.getByRole('searchbox', { name: 'Find a suite' });
  await selector.focus();
  await selector.press('Enter');
  await expect(search).toBeFocused();
  await search.fill('no-such-suite');
  await expect(suites.getByRole('link')).toHaveCount(0);
  await expect(suites.getByRole('status')).toHaveText('No matching suites');
  await search.press('ArrowDown');
  await expect(search).toBeFocused();
  await search.fill('orders-and-checkout');
  await expect(suites.getByRole('link')).toHaveCount(1);
  await search.press('ArrowDown');
  const destination = suites.getByRole('link', { name: /orders-and-checkout/ });
  await expect(destination).toBeFocused();
  await destination.press('Enter');
  await expect(page).toHaveURL(/\/suites\/orders-and-checkout$/);
  await expect(suites).toBeHidden();
  if (isMobile) {
    await expect(page.getByRole('dialog', { name: 'Suite index', exact: true })).toBeHidden();
    await page.getByRole('button', { name: 'Toggle suite index' }).click();
  }
  await expect(selector).toHaveAccessibleName('Test suite: orders-and-checkout');
  await selector.click();
  await expect(search).toHaveValue('');
  await expect(suites.getByRole('link')).toHaveCount(7);
  await expect(suites.getByRole('link', { name: /orders-and-checkout/ })).toHaveAttribute('aria-current', 'page');
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);
  await search.fill('workspace');
  await page.getByRole('heading', { name: 'Test suite', exact: true }).click();
  await expect(suites).toBeHidden();
  await selector.click();
  await expect(search).toHaveValue('');
  await expect(suites.getByRole('link')).toHaveCount(7);
  await selector.click();
  await expect(suites).toBeHidden();
});

test('review navigation dropdown contains long suite names and scrolls independently', async ({ page, isMobile }, info) => {
  const displayName = 'LongSuiteName'.repeat(8);
  await page.route('**/_snapshot/data/manifest.json?*', async (route) => {
    const response = await route.fetch();
    const manifest = await response.json();
    const current = manifest.suites.find((suite: { id: string }) => suite.id === 'workspace-overview');
    manifest.suites = Array.from({ length: 30 }, (_, index) => ({
      ...current,
      id: index === 0 ? current.id : `suite-${index}`,
      displayName: index === 0 ? displayName : `Suite ${index}`,
    }));
    await route.fulfill({ response, json: manifest });
  });
  await page.setViewportSize({ width: isMobile ? 320 : 1280, height: 720 });
  await page.goto('/suites/workspace-overview');
  await page.getByRole('button', { name: 'Dark theme' }).click();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  const selector = page.getByRole('button', { name: `Test suite: ${displayName}` });
  await expect(selector).toContainText(displayName);
  expect(await selector.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  await selector.click();
  const dropdown = page.getByRole('dialog', { name: 'Test suites', exact: true });
  const suites = dropdown.getByRole('navigation', { name: 'Test suites', exact: true });
  await expect(suites.getByRole('link')).toHaveCount(30);
  await expect(suites.locator('[aria-current="page"] .suite-dropdown-check')).toBeVisible();
  const bounds = await dropdown.boundingBox();
  if (!bounds) throw new Error('The suite dropdown is unavailable');
  expect(bounds.x).toBeGreaterThanOrEqual(0);
  expect(bounds.y).toBeGreaterThanOrEqual(0);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(isMobile ? 320 : 1280);
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(720);
  expect(await suites.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true);
  const sidebar = page.locator('.suite-panel-scroll:visible');
  const sidebarTop = await sidebar.evaluate((element) => element.scrollTop);
  await suites.getByRole('link').last().scrollIntoViewIfNeeded();
  expect(await suites.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  expect(await sidebar.evaluate((element) => element.scrollTop)).toBe(sidebarTop);
  expect(await page.locator('.review-scroll').evaluate((element) => element.scrollTop)).toBe(0);
  await expect(page.getByRole('searchbox', { name: 'Find a suite' })).toBeVisible();
  await screenshot(page, info.project.name, 'navigation-dropdown-long-names-dark');
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
  await page.getByRole('button', { name: /^Test suite:/ }).click();
  await page
    .getByRole('navigation', { name: 'Test suites', exact: true })
    .getByRole('link', { name: /orders-and-checkout/ })
    .click();
  if (isMobile) await page.getByRole('button', { name: 'Toggle suite index' }).click();
  await expect(scenarios).toContainText('orders-and-checkout / First scenario');
  await expect(scenarios).not.toContainText('workspace-overview');
  for (const link of await scenarios.getByRole('link').all()) await expect(link).toHaveAttribute('href', /^\/suites\/orders-and-checkout\/images\//);
  await page.getByRole('button', { name: /^Test suite:/ }).click();
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
  await page.setViewportSize({ width: 1200, height: 420 });
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
  await expect(page.locator('.comparison-crosshair')).toHaveCount(3);
  await page.mouse.wheel(0, 300);
  await expect.poll(() => content.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  await expect(page.locator('.comparison-crosshair')).toHaveCount(0);
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

for (const differentSizes of [false, true]) {
  test(`side by side crosshairs track image pixels (${differentSizes ? 'different' : 'equal'} sizes)`, async ({ page }, info) => {
    if (differentSizes) {
      await page.route('**/_snapshot/data/suites/**', async (route) => {
        const response = await route.fetch();
        const data = await response.json();
        if (data.id === 'workspace-overview') data.tests[0].reference = data.tests[1].reference;
        await route.fulfill({ response, json: data });
      });
    }
    await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
    const images = page.locator('.comparison-lane img');
    const crosshairs = page.locator('.comparison-crosshair');
    await expect(images).toHaveCount(3);
    await expect.poll(() => images.evaluateAll((elements) => elements.every((element) => (element as HTMLImageElement).naturalWidth > 0))).toBe(true);
    const commonSize = await images.evaluateAll((elements) => ({
      width: Math.min(...elements.map((element) => (element as HTMLImageElement).naturalWidth)),
      height: Math.min(...elements.map((element) => (element as HTMLImageElement).naturalHeight)),
    }));
    await expect(crosshairs).toHaveCount(0);
    for (const zoomIn of [false, true]) {
      if (zoomIn) await page.getByRole('button', { name: 'Zoom in', exact: true }).click();
      for (const [index, image] of (await images.all()).entries()) {
        await image.locator('..').hover();
        const bounds = await image.boundingBox();
        if (!bounds) throw new Error('Comparison image is not visible');
        const { horizontal, vertical } = await image.evaluate(
          (element, position) => ({
            horizontal: position.x / (element as HTMLImageElement).naturalWidth,
            vertical: position.y / (element as HTMLImageElement).naturalHeight,
          }),
          { x: commonSize.width * (0.3 + index * 0.1), y: commonSize.height * (0.2 + index * 0.1) },
        );
        await page.mouse.move(bounds.x + bounds.width * horizontal, bounds.y + bounds.height * vertical);
        await expect(crosshairs).toHaveCount(3);
        await expect
          .poll(() =>
            page.locator('.comparison-lane').evaluateAll(
              (lanes, position) => {
                const source = lanes[position.index]?.querySelector('img');
                if (!source) return Number.POSITIVE_INFINITY;
                return Math.max(
                  ...lanes.map((lane) => {
                    const target = lane.querySelector('img');
                    const horizontalGuide = lane.querySelector('.crosshair-horizontal')?.getBoundingClientRect();
                    const verticalGuide = lane.querySelector('.crosshair-vertical')?.getBoundingClientRect();
                    if (!target || !horizontalGuide || !verticalGuide) return Number.POSITIVE_INFINITY;
                    const bounds = target.getBoundingClientRect();
                    return Math.max(Math.abs(verticalGuide.x + verticalGuide.width / 2 - bounds.x - (source.naturalWidth * position.horizontal * bounds.width) / target.naturalWidth), Math.abs(horizontalGuide.y + horizontalGuide.height / 2 - bounds.y - (source.naturalHeight * position.vertical * bounds.height) / target.naturalHeight));
                  }),
                );
              },
              { index, horizontal, vertical },
            ),
          )
          .toBeLessThan(1);
        for (const guide of await crosshairs.all()) {
          await expect(guide).toHaveAttribute('aria-hidden', 'true');
          await expect(guide).toHaveCSS('pointer-events', 'none');
          expect((await guide.locator('.crosshair-horizontal').boundingBox())?.height).toBeCloseTo(1);
          expect((await guide.locator('.crosshair-vertical').boundingBox())?.width).toBeCloseTo(1);
        }
      }
    }
    await screenshot(page, info.project.name, `comparison-crosshairs${differentSizes ? '-different-sizes' : ''}`);
    if (differentSizes) {
      const testImage = images.nth(1);
      await testImage.locator('..').hover();
      const bounds = await testImage.boundingBox();
      if (!bounds) throw new Error('Test image is not visible');
      await page.mouse.move(bounds.x + bounds.width * 0.9, bounds.y + bounds.height * 0.2);
      await expect(crosshairs).toHaveCount(2);
      await expect(page.locator('.comparison-lane').first().locator('.comparison-crosshair')).toHaveCount(0);
    }
    await page.getByRole('button', { name: 'Fit images', exact: true }).click();
    const firstLane = await page.locator('.comparison-lane').first().boundingBox();
    const secondLane = await page.locator('.comparison-lane').nth(1).boundingBox();
    if (!firstLane || !secondLane) throw new Error('Comparison lanes are not visible');
    await images.first().locator('..').hover();
    await expect(crosshairs).toHaveCount(3);
    await page.mouse.move((firstLane.x + firstLane.width + secondLane.x) / 2, firstLane.y + 10);
    await expect(crosshairs).toHaveCount(0);
    await page.getByRole('button', { name: 'Fit images', exact: true }).hover();
    await expect(crosshairs).toHaveCount(0);
    await page.goto('/suites/customer-accounts/images/overview__desktop.png?mode=side');
    await expect(images).toHaveCount(2);
    await images.first().locator('..').hover();
    await expect(crosshairs).toHaveCount(0);
  });
}

test('side by side crosshairs clear for view changes, touch, and canceled pointers', async ({ page, isMobile }) => {
  await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
  const image = page.locator('.comparison-image').first();
  const crosshairs = page.locator('.comparison-crosshair');
  for (const event of ['pointerdown', 'pointercancel', 'pointermove'] as const) {
    await image.hover();
    await expect(crosshairs).toHaveCount(3);
    await image.dispatchEvent(event, { pointerType: 'touch' });
    await expect(crosshairs).toHaveCount(0);
  }
  await image.hover();
  await expect(crosshairs).toHaveCount(3);
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(crosshairs).toHaveCount(0);
  for (const [mode, label] of [
    ['wipe', 'Wipe'],
    ['side', 'Side by side'],
  ]) {
    if (isMobile) await page.getByRole('combobox', { name: 'Comparison mode' }).selectOption(mode || 'side');
    else await page.getByRole('button', { name: label, exact: true }).click();
    await expect(crosshairs).toHaveCount(0);
  }
  await image.hover();
  await expect(crosshairs).toHaveCount(3);
  await page.getByRole('link', { name: 'Next image', exact: true }).click();
  await expect(page).toHaveURL(/recent-orders__mobile.png/);
  await expect(crosshairs).toHaveCount(0);
});

for (const missing of [true, false]) {
  test(`side by side crosshairs exclude ${missing ? 'missing' : 'unavailable'} images`, async ({ page }) => {
    await page.route('**/_snapshot/data/suites/**', async (route) => {
      const response = await route.fetch();
      const data = await response.json();
      if (data.id === 'workspace-overview') {
        if (missing) data.tests[0].reference = null;
        else await page.route(`**/${data.tests[0].reference}`, (image) => image.abort());
      }
      await route.fulfill({ response, json: data });
    });
    await page.goto('/suites/workspace-overview/images/overview__desktop.png?mode=side');
    await expect(page.getByText(missing ? 'No image' : 'Image unavailable', { exact: true })).toBeVisible();
    const lanes = page.locator('.comparison-image');
    await lanes.nth(1).hover();
    await expect(page.locator('.comparison-crosshair')).toHaveCount(2);
    await expect(lanes.first().locator('.comparison-crosshair')).toHaveCount(0);
    await lanes.first().hover();
    await expect(page.locator('.comparison-crosshair')).toHaveCount(0);
  });
}

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
