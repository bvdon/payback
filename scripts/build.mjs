// Bundles the UI with esbuild into dist/. `--watch` rebuilds on change and
// serves dist/ on http://localhost:5174 (what `tauri dev` loads).
import * as esbuild from 'esbuild';
import { copyFile, mkdir, rm } from 'node:fs/promises';

const watch = process.argv.includes('--watch');

const options = {
  entryPoints: ['src/main.tsx'],
  bundle: true,
  outdir: 'dist/assets',
  format: 'esm',
  target: ['safari15', 'edge100'],
  jsx: 'automatic',
  minify: !watch,
  sourcemap: watch ? 'inline' : false,
  logLevel: 'info',
  define: { 'process.env.NODE_ENV': JSON.stringify(watch ? 'development' : 'production') },
};

await rm('dist', { recursive: true, force: true });
await mkdir('dist/assets', { recursive: true });
await copyFile('index.html', 'dist/index.html');

if (watch) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
  await ctx.serve({ servedir: 'dist', port: 5174, host: 'localhost' });
  console.log('UI dev server: http://localhost:5174');
} else {
  await esbuild.build(options);
}
