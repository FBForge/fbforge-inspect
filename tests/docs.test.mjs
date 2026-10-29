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
  for (const file of ['start.png', 'destination.png', 'schema.png'])
    assert.ok(guide.includes(file));
  assert.ok(guide.includes('IN ENTWICKLUNG'));
  assert.ok(guide.includes('<ol>'));
  assert.ok(guide.includes('<pre><code>'));
  assert.ok(!product.includes('releases/download/'));
  const introduction = product.match(/<p class="inspect-lead">([\s\S]*?)<\/p>/)[1];
  assert.ok(introduction.includes('Zustellpunkte in FIT-Connect prüfen.'));
  assert.ok(introduction.includes('FIT-Connect und PVOG'));
  assert.ok(!introduction.includes(';'));
  assert.ok(product.includes('Leistung im FIT-Connect-Zustellpunkt'));
  assert.ok(product.includes('Adressierungssignaturen (Destination Signatures)'));
  assert.ok(product.includes('EfA-Parameter'));
  const schemaTopic = topics.find((topic) => topic.id === 'schemas');
  assert.ok(schemaTopic.paragraphs.some((text) => text.includes('Leistung im Zustellpunkt')));
  const pvogTopic = topics.find((topic) => topic.id === 'routing');
  assert.ok(pvogTopic.paragraphs.some((text) => text.includes('nicht aus PVOG')));
});

test('website export inventories only public files with matching checksums', async () => {
  await import('../scripts/export-website.mjs');
  const { createHash } = await import('node:crypto');
  const manifest = JSON.parse(readFileSync('dist-website/manifest.json', 'utf8'));
  assert.equal(manifest.product, 'FBForge Inspect');
  assert.match(manifest.sourceCommit, /^[a-f0-9]{40}$/);
  assert.ok(Number.isFinite(Date.parse(manifest.sourceCommitDate)));
  assert.ok(manifest.files.length >= 10);
  for (const entry of manifest.files) {
    assert.ok(entry.path.startsWith('inspect/'));
    assert.ok(!entry.path.includes('..'));
    assert.equal(
      createHash('sha256')
        .update(readFileSync(`dist-website/site/${entry.path}`))
        .digest('hex'),
      entry.sha256,
    );
  }
  for (const name of [
    'inspect/index.html',
    'inspect/images/start.png',
    'inspect/images/schema.png',
    'inspect/hilfe/0.1/index.html',
  ]) {
    assert.ok(
      manifest.files.some((file) => file.path === name),
      name,
    );
  }
});
