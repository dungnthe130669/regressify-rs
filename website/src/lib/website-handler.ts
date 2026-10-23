import { serveSites, type AdminEnv } from './admin-api';

export interface WebsiteEnv extends AdminEnv {
  ASSETS: { fetch(request: Request): Promise<Response> };
}

export async function routeWebsite(request: Request, env: WebsiteEnv, mcp: () => Promise<Response>): Promise<Response> {
  const path = new URL(request.url).pathname;
  if (path === '/mcp') return mcp();
  if (path === '/admin/api/sites' || path === '/admin/api/sites/') return serveSites(request, env);
  if (path === '/admin' || path.startsWith('/admin/')) {
    const headers = { 'Cache-Control': 'private, no-store', 'X-Robots-Tag': 'noindex, nofollow' };
    if (path === '/admin') return new Response(null, { status: 308, headers: { ...headers, Location: '/admin/' } });
    if (path !== '/admin/') return new Response('Not found', { status: 404, headers });
    const asset = await env.ASSETS.fetch(request);
    const response = new Response(asset.body, asset);
    for (const [key, value] of Object.entries(headers)) response.headers.set(key, value);
    return response;
  }
  return env.ASSETS.fetch(request);
}