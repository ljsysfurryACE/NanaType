import { copyFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { bundle } from '@remotion/bundler';
import { getCompositions, openBrowser, renderStill } from '@remotion/renderer';

const root = dirname(fileURLToPath(import.meta.url));
const character = process.argv[2] ?? process.env.NANATYPE_COVER_CHARACTER;
if (!character) {
  throw new Error('Provide a local transparent PNG: npm run render -- <character.png> [output-directory]');
}
const output = resolve(process.argv[3] ?? join(root, 'out'));
const staging = await mkdtemp(join(tmpdir(), 'nanatype-cover-'));
let browser;
try {
  const publicDir = join(staging, 'public');
  await mkdir(publicDir);
  await copyFile(resolve(character), join(publicDir, 'character.png'));
  const serveUrl = await bundle({
    entryPoint: join(root, 'index.tsx'), publicDir,
    outDir: join(staging, 'bundle'), enableCaching: false,
  });
  browser = await openBrowser('chrome', { browserExecutable: process.env.REMOTION_BROWSER_EXECUTABLE });
  const compositions = await getCompositions(serveUrl, { puppeteerInstance: browser, timeoutInMilliseconds: 180000 });
  await mkdir(output, { recursive: true });
  for (const composition of compositions) {
    const filename = join(output, `${composition.id}.png`);
    await renderStill({ serveUrl, composition, puppeteerInstance: browser, output: filename, imageFormat: 'png', overwrite: true, timeoutInMilliseconds: 180000 });
    console.log(filename);
  }
} finally {
  try {
    await browser?.close({ silent: true });
  } finally {
    if (dirname(resolve(staging)) !== resolve(tmpdir())) throw new Error('Unexpected staging directory');
    await rm(staging, { recursive: true, force: true });
  }
}
