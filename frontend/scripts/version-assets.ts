import { createHash } from 'node:crypto';
import { type DefaultTreeAdapterMap, parse, serialize } from 'parse5';
import postcss from 'postcss';
import valueParser from 'postcss-value-parser';

// Match src/snapshot/helpers.rs::get_asset_version(content, 20).
export function getAssetVersion(content: Uint8Array | string): string {
  return createHash('sha384').update(content).digest('base64url').slice(0, 20);
}

export function versionBuild(input: Map<string, Uint8Array>): Map<string, Uint8Array> {
  const files = new Map(input);
  const reference = (value: string, host: string) => {
    const url = new URL(value, `https://snapshot.invalid/${host}`);
    const path = decodeURIComponent(url.pathname.slice(1));
    const bytes = files.get(path);
    if (url.origin !== 'https://snapshot.invalid' || !path.startsWith('_snapshot/assets/') || !bytes) {
      throw new Error(`Unbundled snapshot dependency: ${value}`);
    }
    return `/${path}?v=${getAssetVersion(bytes)}`;
  };
  for (const [name, bytes] of files) {
    if (!name.endsWith('.css')) continue;
    const css = postcss.parse(new TextDecoder().decode(bytes), { from: name });
    css.walkDecls((declaration) => {
      const value = valueParser(declaration.value);
      value.walk((node) => {
        if (node.type !== 'function' || node.value.toLowerCase() !== 'url') return;
        const target = node.nodes[0];
        if (!target || (target.type !== 'string' && target.type !== 'word') || node.nodes.length !== 1) throw new Error('Unsupported CSS resource URL.');
        if (!target.value.startsWith('data:') && !target.value.startsWith('#')) target.value = reference(target.value, name);
        return false;
      });
      declaration.value = value.toString();
    });
    files.set(name, new TextEncoder().encode(css.toString()));
  }
  const html = files.get('index.html');
  if (!html) throw new Error('Missing snapshot entry HTML.');
  const document = parse(new TextDecoder().decode(html));
  const visit = (node: DefaultTreeAdapterMap['node']) => {
    if ('tagName' in node && (node.tagName === 'script' || node.tagName === 'link')) {
      for (const attribute of node.attrs) {
        if (attribute.name === 'src' || attribute.name === 'href') attribute.value = reference(attribute.value, 'index.html');
      }
    }
    if ('childNodes' in node) for (const child of node.childNodes) visit(child);
  };
  visit(document);
  files.set('index.html', new TextEncoder().encode(serialize(document)));
  return files;
}
