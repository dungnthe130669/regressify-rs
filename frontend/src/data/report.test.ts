import { describe, expect, test } from 'bun:test';
import { artifactPath, captureSchema, encodeArtifactPath, filterCaptures, imagePath, manifestSchema, passRate, safePageUrl, suiteReportSchema } from './report';

const image = `_snapshot/images/shop/test/home.png?v=${'a'.repeat(20)}`;
export const capture = captureSchema.parse({
  id: 'home__FINAL.png',
  label: 'Home, desktop',
  scenario: 'Home',
  selector: 'document',
  status: 'fail',
  threshold: 0.1,
  mismatch: 1.25,
  error: null,
  reference: image,
  test: image,
  diff: image,
  referenceUrl: 'https://example.com/',
  testUrl: null,
  dimensions: { same: false, width: -20, height: 30 },
});

describe('snapshot contract', () => {
  test('accepts signed dimension differences and hostile labels as plain data', () => {
    expect(captureSchema.parse({ ...capture, label: '<img src=x onerror="alert(1)">' }).label).toContain('<img');
    expect(capture.dimensions?.width).toBe(-20);
  });
  test('rejects unversioned, remote and traversal images', () => {
    for (const path of ['../../state.json', 'https://example.com/image.png', '/images/x.png', '_snapshot/images/x.png', `${image}?x=1`, `_snapshot/images/../private.png?v=${'a'.repeat(20)}`, `_snapshot/images/%2E%2E/private.png?v=${'a'.repeat(20)}`, `_snapshot/images/shop/%2Fprivate.png?v=${'a'.repeat(20)}`, `_snapshot/images/shop/CON.png?v=${'a'.repeat(20)}`, `_snapshot/images/shop/home.png?v=${'a'.repeat(19)}`]) {
      expect(captureSchema.safeParse({ ...capture, test: path }).success).toBe(false);
    }
  });
  test('decodes canonical paths once and requires a version on data and every image role', () => {
    const path = '_snapshot/images/caf\u00e9 & shop/reference/home #1 %2F.png';
    const url = `${encodeArtifactPath(path)}?v=${'A_-0'.repeat(5)}`;
    expect(artifactPath(url, 'images')).toBe(path);
    expect(artifactPath(url, 'data')).toBeUndefined();
    for (const role of ['reference', 'test', 'diff']) {
      expect(captureSchema.safeParse({ ...capture, [role]: '_snapshot/images/shop/test/home.png' }).success).toBe(false);
    }
    expect(artifactPath(`_snapshot/data/manifest.json?v=${'a'.repeat(20)}`, 'data')).toBe('_snapshot/data/manifest.json');
    expect(artifactPath('_snapshot/data/manifest.json', 'data')).toBeUndefined();
  });
  test('rejects unknown statuses, invalid percentages, duplicate image IDs', () => {
    expect(captureSchema.safeParse({ ...capture, status: 'unknown' }).success).toBe(false);
    expect(captureSchema.safeParse({ ...capture, threshold: 101 }).success).toBe(false);
    expect(captureSchema.safeParse({ ...capture, mismatch: Number.NaN }).success).toBe(false);
    expect(suiteReportSchema.safeParse({ id: 'shop', runId: 'run', tests: [capture, capture] }).success).toBe(false);
  });
  test('validates empty manifests and rejects inconsistent totals and versions', () => {
    const manifest = {
      version: 2,
      title: '',
      heading: '',
      subheading: '',
      generatedAt: '2026-09-23T00:00:00Z',
      suites: [],
    };
    expect(manifestSchema.parse(manifest).suites).toEqual([]);
    expect(manifestSchema.safeParse({ ...manifest, version: 1 }).success).toBe(false);
    const suite = {
      id: 'shop',
      total: 2,
      passed: 1,
      failed: 0,
      errors: 0,
      report: `_snapshot/data/suites/shop.json?v=${'a'.repeat(20)}`,
    };
    expect(manifestSchema.safeParse({ ...manifest, suites: [suite] }).success).toBe(false);
  });
  test('encodes route segments without interpreting names as routes', () => {
    expect(imagePath('caf\u00e9 & shop', 'home #1.png')).toBe('/suites/caf%C3%A9%20%26%20shop/images/home%20%231.png');
  });
  test('allows only uncredentialed http(s) page links', () => {
    for (const url of ['javascript:alert(1)', 'data:text/html,x', 'file:///etc/passwd', '//example.com', 'https://user:secret@example.com', 'bad', null]) {
      expect(safePageUrl(url)).toBeUndefined();
    }
    expect(safePageUrl('https://example.com/a?q=b')).toBe('https://example.com/a?q=b');
  });
  test('combines search and status including engine errors and zero totals', () => {
    const tests = [capture, { ...capture, id: 'error', error: 'Timeout' }, { ...capture, id: 'pass', status: 'pass' as const, label: 'Account' }];
    expect(filterCaptures(tests, 'home', 'fail')).toHaveLength(2);
    expect(filterCaptures(tests, ' HOME ', 'error')).toHaveLength(1);
    expect(filterCaptures(tests, 'account', 'pass')).toHaveLength(1);
    expect(filterCaptures(tests, 'missing', 'all')).toHaveLength(0);
    expect(passRate(0, 0)).toBe('0');
    expect(passRate(2, 3)).toBe('66.7');
  });
});
