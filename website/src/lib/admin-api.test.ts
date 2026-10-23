import { describe, expect, spyOn, test } from 'bun:test';
import { parseSites, serveSites, type AdminFetch } from './admin-api';
import { routeWebsite } from './website-handler';

const columns = ['url', 'datetime', 'file_count', 'total_bytes', 'duration_ms'];
const row = ['https://demo.example.com/report/', '2026-10-23T10:00:00Z', 1200, 1048576, 12500];
const env = { POSTHOG_API_ENDPOINT: 'https://analytics.example.com/secret-endpoint', POSTHOG_API_TOKEN: 'private-token' };
const request = () => new Request('https://regressify.eshn.dev/admin/api/sites');
const result = (data: unknown): AdminFetch => async () => Response.json(data);

describe('admin site feed', () => {
  test('maps reordered columns and strips extra query data', async () => {
    const response = await serveSites(request(), env, result({
      columns: ['private', ...columns.toReversed()], results: [['hidden', ...row.toReversed()]],
      query: env.POSTHOG_API_TOKEN, metadata: env.POSTHOG_API_ENDPOINT, hasMore: true,
    }));
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(body.sites).toEqual([{ url: row[0], datetime: row[1], file_count: 1200, total_bytes: 1048576, duration_ms: 12500 }]);
    expect(body.hasMore).toBe(true);
    expect(Number.isFinite(Date.parse(body.fetchedAt))).toBe(true);
    expect(JSON.stringify(body)).not.toContain('private');
    expect(JSON.stringify(body)).not.toContain('secret-endpoint');
    expect(response.headers.get('Cache-Control')).toBe('private, no-store');
    expect(response.headers.get('Access-Control-Allow-Origin')).toBeNull();
  });

  test('uses a fixed server endpoint and POST body without forwarding client inputs', async () => {
    let called = false;
    const fetcher = (async (url: string | URL | Request, init?: RequestInit) => {
      called = true;
      expect(url).toBe(env.POSTHOG_API_ENDPOINT);
      expect(init?.method).toBe('POST');
      expect(init?.body).toBe('{}');
      expect(init?.redirect).toBe('manual');
      expect(new Headers(init?.headers).get('Authorization')).toBe(`Bearer ${env.POSTHOG_API_TOKEN}`);
      expect(init?.signal).toBeInstanceOf(AbortSignal);
      return Response.json({ columns, results: [] });
    }) satisfies AdminFetch;
    const response = await serveSites(new Request(`${request().url}?endpoint=https://attacker.example`), env, fetcher);
    expect(response.status).toBe(200);
    expect(called).toBe(true);
  });

  test('accepts empty results and null optional measurements', () => {
    expect(parseSites({ columns, results: [] })).toEqual({ sites: [], hasMore: false });
    expect(parseSites({ columns, results: [[row[0], null, null, 0, null]] }).sites[0]?.total_bytes).toBe(0);
  });

  test.each([
    null, {}, { columns, results: [], error: 'sensitive upstream error' },
    { columns: [...columns, 'URL'], results: [] }, { columns: columns.slice(1), results: [] },
    { columns, results: [[...row, 'extra']] }, { columns, results: [[row[0], 'invalid', 1, 2, 3]] },
    { columns, results: [[row[0], null, -1, 2, 3]] },
    { columns, results: [[row[0], null, '1', 2, 3]] },
    { columns, results: [[row[0], null, Infinity, 2, 3]] },
    { columns, results: Array.from({ length: 10_001 }, () => row) },
    { columns, results: [], hasMore: 'yes' },
  ])('rejects malformed data %#', data => expect(() => parseSites(data)).toThrow());

  test('rejects missing secrets, unsafe configuration, methods and cross-origin callers', async () => {
    const neverFetch: AdminFetch = async () => { throw new Error('Must not fetch'); };
    expect((await serveSites(request(), {}, neverFetch)).status).toBe(503);
    for (const endpoint of ['http://analytics.example.com', 'https://user:pass@example.com', 'invalid']) {
      expect((await serveSites(request(), { ...env, POSTHOG_API_ENDPOINT: endpoint }, neverFetch)).status).toBe(502);
    }
    const method = await serveSites(new Request(request().url, { method: 'POST' }), env, neverFetch);
    expect(method.status).toBe(405);
    expect(method.headers.get('Allow')).toBe('GET');
    for (const headers of [new Headers({ Origin: 'https://attacker.example' }), new Headers({ 'Sec-Fetch-Site': 'cross-site' })]) {
      expect((await serveSites(new Request(request().url, { headers }), env, neverFetch)).status).toBe(403);
    }
  });

  test('sanitizes upstream errors, redirects, invalid JSON and oversized bodies', async () => {
    const responses = [
      new Response(env.POSTHOG_API_TOKEN, { status: 401 }),
      new Response('', { status: 302, headers: { Location: 'https://attacker.example' } }),
      new Response('invalid JSON'), new Response('x'.repeat(2 * 1024 * 1024 + 1)),
    ];
    for (const upstream of responses) {
      const response = await serveSites(request(), env, async () => upstream);
      expect(response.status).toBe(502);
      expect(await response.text()).not.toContain(env.POSTHOG_API_TOKEN);
    }
    const failed: AdminFetch = async () => { throw new Error(env.POSTHOG_API_ENDPOINT); };
    expect(await (await serveSites(request(), env, failed)).text()).not.toContain(env.POSTHOG_API_ENDPOINT);
  });

  test('routes the protected feed, uncached admin assets, public assets and MCP independently', async () => {
    let assetCalls = 0;
    let mcpCalls = 0;
    const bindings = { ASSETS: { async fetch() { assetCalls++; return new Response('asset', { headers: { 'Cache-Control': 'public, max-age=3600' } }); } } };
    const mcp = async () => { mcpCalls++; return new Response('mcp'); };
    const route = (path: string) => routeWebsite(new Request(`https://regressify.eshn.dev${path}`), bindings, mcp);
    const redirect = await route('/admin');
    expect(redirect.status).toBe(308);
    expect(redirect.headers.get('Location')).toBe('/admin/');
    const admin = await route('/admin/');
    expect(await admin.text()).toBe('asset');
    expect(admin.headers.get('Cache-Control')).toBe('private, no-store');
    expect(admin.headers.get('X-Robots-Tag')).toContain('noindex');
    for (const path of ['/admin/api/sites', '/admin/api/sites/']) expect((await route(path)).status).toBe(503);
    expect((await route('/admin/api/unknown')).status).toBe(404);
    expect(assetCalls).toBe(1);
    expect(await (await route('/mcp')).text()).toBe('mcp');
    expect(mcpCalls).toBe(1);
    expect((await route('/docs/')).headers.get('Cache-Control')).toContain('public');
    expect(assetCalls).toBe(2);
  });

  test('logs only safe failure classifications, never exception messages or credentials', async () => {
    const warning = spyOn(console, 'warn').mockImplementation(() => {});
    try {
      await serveSites(request(), env, async () => { throw new TypeError(`${env.POSTHOG_API_ENDPOINT} ${env.POSTHOG_API_TOKEN}`); });
      expect(warning.mock.calls).toEqual([['Admin feed unavailable', { stage: 'request', kind: 'TypeError' }]]);
    } finally {
      warning.mockRestore();
    }
  });
});