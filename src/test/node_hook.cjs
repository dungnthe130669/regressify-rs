const { createRequire } = require('node:module');
const { pathToFileURL } = require('node:url');
const path = require('node:path');

(async () => {
  let input = '';
  for await (const chunk of process.stdin) input += chunk;
  const args = JSON.parse(input);
  const localRequire = createRequire(path.join(process.cwd(), 'package.json'));
  let playwright;
  try { playwright = localRequire('playwright'); }
  catch { playwright = localRequire('playwright-core'); }
  const browser = await playwright.chromium.connectOverCDP(args.endpoint);
  let target;
  let context;
  for (const candidate of browser.contexts()) {
    for (const page of candidate.pages()) {
      const session = await candidate.newCDPSession(page);
      const info = await session.send('Target.getTargetInfo');
      await session.detach();
      if (info.targetInfo.targetId === args.target) { target = page; context = candidate; }
    }
  }
  if (!target) throw new Error('Scenario page is unavailable');
  const imported = await import(pathToFileURL(args.script).href);
  if (typeof imported.default !== 'function') throw new Error('Node hook must export a function');
  await imported.default(target, args.scenario, args.viewport, args.isReference, context, args.config);
  process.exit(0);
})().catch(error => {
  console.error(error.message);
  process.exit(1);
});