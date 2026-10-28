import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
test('exported guide uses the product website shell and preserves every help topic', async () => {
  await import('../scripts/build-help.mjs');
  const product = readFileSync('website/inspect/index.html', 'utf8');
  const guide = readFileSync('dist-help/inspect/hilfe/0.1/index.html', 'utf8');
  for (const expression of [
    /<header class="site-header">[\s\S]*?<\/header>/,
    /<footer class="footer">[\s\S]*?<\/footer>/,
  ]) {
    assert.equal(guide.match(expression)?.[0], product.match(expression)?.[0]);
  }
  for (const path of [
    '/fbforge-v8.css',
    '/brand-v1.css',
    '/theme-v8.js',
    '/assets/fbforge-favicon-v1.svg',
    '/assets/fonts/InterVariable.woff2',
  ]) {
    assert.ok(guide.includes(path), path);
    assert.ok(product.includes(path), path);
  }
  assert.ok(!product.includes('fbforge-v5.css'));
  assert.ok(guide.includes('/guide-v1.css'));
  const topics = JSON.parse(readFileSync('docs/help.json', 'utf8'));
  for (const topic of topics) {
    assert.ok(guide.includes(`id="${topic.id}"`));
    assert.ok(guide.includes(`href="#${topic.id}"`));
  }
  assert.ok(guide.includes('destination.png'));
});
