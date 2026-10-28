import { readFile, mkdir, writeFile, copyFile } from 'node:fs/promises';
const product = await readFile('website/inspect/index.html', 'utf8');
const header = product.match(/<header class="site-header">[\s\S]*?<\/header>/)?.[0];
const footer = product.match(/<footer class="footer">[\s\S]*?<\/footer>/)?.[0];
if (!header || !footer) throw new Error('Gemeinsame Website-Kopfzeile oder Footer fehlt.');
const topics = JSON.parse(await readFile('docs/help.json', 'utf8'));
const esc = (value) =>
  value.replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
  );
const html = `<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="theme-color" content="#fafafa"><title>FBForge Inspect · Hilfe 0.1</title><link rel="canonical" href="https://fbforge.de/inspect/hilfe/0.1/"><link rel="icon" href="/assets/fbforge-favicon-v1.svg" type="image/svg+xml"><script src="/theme-v8.js"></script><link rel="preload" href="/assets/fonts/InterVariable.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="/fbforge-v8.css"><link rel="stylesheet" href="/brand-v1.css"><link rel="stylesheet" href="/guide-v1.css"><script src="/fbforge-v7.js" defer></script></head>
<body class="guide-page"><a class="skip-link" href="#inhalt">Zum Inhalt springen</a>${header}
<div class="guide-layout"><nav aria-label="Hilfethemen"><h2>In dieser Anleitung</h2><ul>${topics.map((t) => `<li><a href="#${esc(t.id)}">${esc(t.title)}</a></li>`).join('')}</ul></nav>
<main id="inhalt" tabindex="-1"><p class="edition">FBForge Inspect · Version 0.1</p><h1>FBForge Inspect verstehen und verwenden</h1><p class="intro">Abläufe, Quellen und Beweisgrenzen. Dieselben Inhalte stehen in der Desktop-App offline bereit.</p>${topics.map((t) => `<section id="${esc(t.id)}"><h2>${esc(t.title)}</h2>${t.paragraphs.map((p) => `<p>${esc(p)}</p>`).join('')}${t.id === 'destination' ? '<figure><img src="destination.png" alt="Destination-Prüfung mit Eingabe der UUID, optionalem Konfigurationsvergleich und Zertifikatsprüfung" width="1180" height="867"><figcaption>Browseransicht der Oberfläche. Öffentliche Abfragen laufen in der Desktop-App.</figcaption></figure>' : ''}</section>`).join('')}</main></div>${footer}</body></html>`;
const output = 'dist-help/inspect/hilfe/0.1';
await mkdir(output, { recursive: true });
await writeFile(`${output}/index.html`, html);
console.log('Statische Hilfe mit gemeinsamer Website-Gestaltung aus docs/help.json erzeugt.');
await copyFile('docs/images/destination.png', `${output}/destination.png`);
