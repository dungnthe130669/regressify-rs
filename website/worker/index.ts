import mcp from '../mcp/src/index';
import { routeWebsite, type WebsiteEnv } from '../src/lib/website-handler';

export default {
  fetch(request: Parameters<typeof mcp.fetch>[0], env: Parameters<typeof mcp.fetch>[1] & WebsiteEnv, context: Parameters<typeof mcp.fetch>[2]) {
    return routeWebsite(request, env, () => mcp.fetch(request, env, context));
  },
};