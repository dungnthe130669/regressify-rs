# Regressify Authoring MCP Worker

This standalone Cloudflare Worker serves versioned public instructions for
authoring Regressify suites, scenarios, actions, workspace configuration, and
predefined files, plus common scenario recipes, lifecycle-hook contracts, and
failure diagnosis. Its complete knowledge bundle is generated from the installed
CLI and embedded in this project. It does not need the Regressify source
repository, Rust, Chromium, an LLM API key, or access to a user's test workspace.

The MCP endpoint is `/mcp` using Streamable HTTP. It exposes three read-only tools:
`get_authoring_guide`, `get_schema`, and `get_example`; discoverable guide/schema
resources; and the `author_regressify` prompt. Agents edit local files using their
own authorized tools. This server cannot execute tests or approve references.
Use the `recipes` topic for complete common-page suites, `hooks` before writing
scripts, and `troubleshooting` for local reports, logs, configuration/capture
failures, and MCP connection problems.
Diagnostic lookups never upload workspace contents or private logs.

For components that expand across related scenarios, capture the same larger
area in both default and expanded states: viewport, or full page when needed.
A header-only default capture can miss a menu incorrectly open before a click.
Keep matching viewport sizes and scroll positions, observe the default state
without first closing or hiding a layer, and treat component close-ups as
supplements. The scenarios guide and navigation recipe explain the pattern.

After creating or editing a suite, agents must run
`regressify ref --test-suite <suite-name>` using their authorized terminal tools.
The CLI is installed alongside the desktop app on each user's `PATH`; no source
checkout or build is needed. Agents verify all intended screenshots in the fresh
run's results and PNGs, not just the exit code or old references. Because `ref`
updates successful references, page and baseline-write authorization still apply.
Blocked verification must be reported as incomplete.

The startup instructions, overview, and authoring prompt tell agents to read and
maintain the consumer project's `.ai/memory/` using their authorized local tools.
Each topic note covers one specific behavior or issue on one site and is at most
60 lines; `.ai/memory/README.md` stays a compact linked index. Notes capture verified
lessons, failed approaches, fixes, and evidence so later work avoids repeating
mistakes. Agents update or split notes instead of accumulating a site-wide diary.
Memory stays in the project that owns the suites, never in this Worker. Do not
store secrets, persisted browser state, personal data, raw logs, or transcripts,
or send project memory to the public service.

## Local Verification

Use Node.js 22.12 or newer and npm. The lockfile pins dependencies. Commands below
operate only on this generated project; the tests start and stop a local Worker.
They also evaluate the generated classic-script examples with stub browser
globals, without launching a browser. Node is development tooling for this
Worker project, not a dependency of Regressify captures or injected scripts.

```sh
npm ci
npm run check
npm test
npm run build
npm run verify
npm run dev
```

`build` is a Wrangler dry run, not a deployment. It writes the single minified
module that `deploy` uploads to `dist/index.js`; source maps are neither generated
nor uploaded, and Wrangler's `dist/README.md` note is never uploaded. `dev` prints
the local URL; connect an MCP client to that URL with `/mcp` appended. The
endpoint is a protocol handler, not a web page. The root URL returns basic
service metadata.

`verify` is the non-publishing deployment gate: dependency audit, TypeScript
check, Worker/protocol tests, then dry-run build. High/critical advisories,
audit-service failures, and any failed check stop the chain. The audit includes
development dependencies because they build and publish the Worker. Do not
automatically force dependency upgrades or suppress advisories to pass the gate.

## Deployment

Review the Worker name in `wrangler.jsonc` before publishing. Do not deploy over
an unrelated Worker with the same name. Authenticate Wrangler locally using
your Cloudflare account or your own CI secret configuration, then run:

```sh
npm run deploy
```

`deploy` always runs `verify` before invoking the live Wrangler deployment.
Neither a failed audit nor a failed test/build can fall through to publication.
The package tests mock npm and Wrangler to verify each failure path without
credentials or real publication. Direct `wrangler deploy` bypasses these gates;
use the package command for the checked workflow.

Wrangler prints the deployed URL. Append `/mcp`, typically
`https://regressify-mcp.<account-subdomain>.workers.dev/mcp`, and configure it as
an HTTP MCP server in your AI client. For VS Code, merge this entry into
`.vscode/mcp.json` in your workspace instead of overwriting existing servers:

```json
{
  "servers": {
    "regressify-mcp": {
      "type": "http",
      "url": "https://regressify-mcp.<account-subdomain>.workers.dev/mcp"
    }
  }
}
```

Ask the agent to read the overview guide before editing Regressify files.
Resources and prompts are available but may not be loaded automatically by the
client. Tool descriptions and the overview explain how to retrieve each topic.
Regressify initialization manages this server name and can refresh its URL to
the installed CLI's bundled endpoint. For an independently hosted endpoint that
must remain unmanaged, choose a distinct client server name and matching agent
instructions, or review the connection after initialization.

## Public Access and Safety

The default deployment is public and unauthenticated. It accepts only small
knowledge lookup requests, not workspace uploads, URLs to fetch, scripts to
execute, or arbitrary filesystem paths. It rejects unknown routes, invalid
topics, oversized bodies, and disallowed browser origins. It performs no outbound
fetches and stores no user state. No KV, R2, D1, or Durable Objects are required.

Do not add real cookies, tokens, passwords, saved state, customer suites, or
private source to the generated content. Cloudflare handles ordinary network
request metadata; the Worker does not log request bodies. CORS/origin checks
are not authentication. Use Cloudflare's MCP-compatible OAuth/Access guidance
before adding private content, and configure rate limits appropriate to your
account. Custom domains/browser clients may require explicit host/origin
allowlists in the handler; do not disable Origin validation.

## Updating the Content

The service reports the generating CLI version. Regenerate with an updated CLI
into a new directory, review the generated differences and dependency changes,
rerun local checks, then redeploy. Do not mix schemas or examples from different
CLI versions. The generator never reads current workspace contents and never
overwrites an existing destination; a failed export may leave an incomplete
directory, so retry into a new path.

JSON Schemas cover document structure, not all runtime semantics. The guides
explain cross-file constraints, inheritance exceptions, saved state, and capture
behavior. Examples use synthetic URLs/selectors and require adaptation to the
user's real application. Never treat an example as authorization to run actions.

The standalone package's version tracks its template format; the Regressify
content version is in `content/catalog.json` and the MCP server identity.
