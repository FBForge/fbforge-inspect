import { readFile, mkdir, writeFile } from 'node:fs/promises';
const topics = JSON.parse(await readFile('docs/help.json', 'utf8'));
const esc = (value) =>
  value.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
  );
const html = `<!doctype html><html lang="de"><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>FBForge Inspect · Hilfe 0.1</title><link rel="stylesheet" href="/fbforge-v5.css"><link rel="stylesheet" href="/inspect/inspect.css"><main class="inspect-help" style="max-width:1000px;margin:40px auto;padding:24px"><a href="/inspect/">← FBForge Inspect</a><h1>Hilfe · Version 0.1</h1>${topics.map((t) => `<section id="${esc(t.id)}"><h2>${esc(t.title)}</h2>${t.paragraphs.map((p) => `<p>${esc(p)}</p>`).join('')}</section>`).join('')}</main></html>`;
await mkdir('dist-help/inspect/hilfe/0.1', { recursive: true });
await writeFile('dist-help/inspect/hilfe/0.1/index.html', html);
console.log('Statische Hilfe aus docs/help.json erzeugt.');
