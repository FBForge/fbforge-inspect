import { readFile, mkdir, writeFile, copyFile } from 'node:fs/promises';
const topics = JSON.parse(await readFile('docs/help.json', 'utf8'));
const esc = (value) =>
  value.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
  );
const html = `<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>FBForge Inspect · Hilfe 0.1</title><link rel="stylesheet" href="guide.css"></head>
<body><a class="skip-link" href="#inhalt">Zum Inhalt</a>
<header><a class="brand" href="/inspect/">FBForge Inspect</a><span>Handbuch · Version 0.1</span></header>
<div class="guide-layout"><nav aria-label="Hilfethemen"><h2>In dieser Anleitung</h2><ul>${topics.map((t) => `<li><a href="#${esc(t.id)}">${esc(t.title)}</a></li>`).join('')}</ul></nav>
<main id="inhalt" tabindex="-1"><p class="eyebrow">ÖFFENTLICHE DIAGNOSE</p><h1>Inspect verstehen und verwenden</h1><p class="lead">Abläufe, Quellen und Beweisgrenzen. Dieselben Inhalte stehen in der Desktop-App offline bereit.</p>${topics.map((t) => `<section id="${esc(t.id)}"><h2>${esc(t.title)}</h2>${t.paragraphs.map((p) => `<p>${esc(p)}</p>`).join('')}${t.id === 'destination' ? '<figure><img src="destination.png" alt="Destination-Prüfung mit Eingabe der UUID, optionalem Konfigurationsvergleich und Zertifikatsprüfung" width="1180" height="867"><figcaption>Browseransicht der Oberfläche. Öffentliche Abfragen laufen in der Desktop-App.</figcaption></figure>' : ''}</section>`).join('')}</main></div>
<footer><span>FBForge Inspect 0.1.0 · EUPL-1.2</span><a href="https://github.com/FBForge/fbforge-inspect">Quellcode und Bauanleitung</a></footer></body></html>`;
const output = 'dist-help/inspect/hilfe/0.1';
await mkdir(output, { recursive: true });
await writeFile(`${output}/index.html`, html);
await copyFile('docs/guide.css', `${output}/guide.css`);
console.log('Eigenständige statische Hilfe aus docs/help.json erzeugt.');
await copyFile('docs/images/destination.png', `${output}/destination.png`);
