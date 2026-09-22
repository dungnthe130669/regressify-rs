import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const require = createRequire(new URL('../target/icon-tools/package.json', import.meta.url));
const { Resvg } = require('@resvg/resvg-js');
const pngToIco = require('png-to-ico');

const source = await readFile(new URL('../assets/icon.svg', import.meta.url));
const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
const temporary = await mkdtemp(join(tmpdir(), 'regressify-icon-'));

try {
  const images = [];
  for (const size of sizes) {
    const renderer = new Resvg(source, {
      fitTo: { mode: 'width', value: size },
      font: { loadSystemFonts: false },
    });
    const path = join(temporary, `${size}.png`);
    const png = renderer.render().asPng();
    await writeFile(path, png);
    if (size === 256) {
      await writeFile(new URL('../assets/icon.png', import.meta.url), png);
    }
    images.push(path);
  }
  await writeFile(new URL('../assets/icon.ico', import.meta.url), await pngToIco(images));
  console.log(`Generated assets/icon.ico (${sizes.join(', ')} px) and assets/icon.png (256 px).`);
} finally {
  await rm(temporary, { recursive: true, force: true });
}
