import { expect, test } from 'bun:test';
import { formatSiteValue, metricStyle, safeSiteUrl, selectSites } from './admin-table';
import type { SiteRecord } from './admin-api';

test('formats count grouping, binary MiB, elapsed times and duration carries', () => {
  expect(formatSiteValue('file_count', 1234567)).toBe('1.234.567');
  expect(formatSiteValue('total_bytes', 1048576)).toBe('1.0');
  expect(formatSiteValue('total_bytes', 0)).toBe('0.0');
  expect(formatSiteValue('duration_ms', 59_500)).toBe('1:00');
  expect(formatSiteValue('duration_ms', 3_599_500)).toBe('1:00:00');
  expect(formatSiteValue('duration_ms', 0)).toBe('0:00');
  expect(formatSiteValue('duration_ms', null)).toBe('-');
  const now = Date.parse('2026-10-23T12:00:00Z');
  expect(formatSiteValue('datetime', '2026-10-23T11:00:00Z', now)).toBe('1h');
  expect(formatSiteValue('datetime', '2026-10-23T13:00:00Z', now)).toBe('1h');
  expect(formatSiteValue('datetime', 'invalid', now)).toBe('invalid');
});

test('allows only HTTP(S) destinations without embedded credentials', () => {
  expect(safeSiteUrl('https://demo.example/report/?page=2#image')?.href).toBe('https://demo.example/report/?page=2#image');
  for (const value of ['javascript:alert(1)', 'data:text/html,<script>', '/relative', 'https://user:pass@example.com']) {
    expect(safeSiteUrl(value)).toBeNull();
  }
});

test('filters full URLs, sorts raw values, keeps missing values last and does not mutate the feed', () => {
  const base: SiteRecord = { url: 'https://beta.example/report', datetime: '2026-10-23T10:00:00Z', file_count: 10, total_bytes: 0, duration_ms: null };
  const sites = [base, { ...base, url: 'https://alpha.example/', file_count: 2, datetime: '2026-10-23T12:00:00Z' }, { ...base, file_count: null }];
  expect(selectSites(sites, '', 'file_count', false).map(site => site.file_count)).toEqual([2, 10, null]);
  expect(selectSites(sites, '', 'file_count', true).map(site => site.file_count)).toEqual([10, 2, null]);
  expect(selectSites(sites, ' ALPHA.EXAMPLE ', 'url', false)).toEqual([sites[1]!]);
  expect(selectSites(sites, '/report', 'url', false)).toHaveLength(2);
  expect(selectSites(sites, '', 'datetime', true)[0]).toBe(sites[1]!);
  expect(sites[0]).toBe(base);
});

test('blends metric endpoints, keeps constant columns neutral and includes exact bold boundary', () => {
  expect(metricStyle(0, 0, 100).color).toContain('var(--success)');
  expect(metricStyle(100, 0, 100).color).toContain('var(--danger) 100%');
  expect(metricStyle(75, 0, 100).bold).toBe(true);
  expect(metricStyle(74.99, 0, 100).bold).toBe(false);
  expect(metricStyle(10, 10, 10).color).toContain('var(--danger) 0%');
});