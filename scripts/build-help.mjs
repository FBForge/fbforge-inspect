import { readFile, mkdir, writeFile, copyFile, rm } from 'node:fs/promises';
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
const images = {
  start: ['start.png', 'Einstieg von FBForge Inspect mit den öffentlichen Diagnoseaufgaben'],
  destination: [
    'destination.png',
    'Destination-Prüfung mit UUID, optionalem Konfigurationsvergleich und Zertifikatsprüfung',
  ],
  schemas: [
    'schema.png',
    'Ausschnitt eines tatsächlichen lokalen Schema-Diagnoseergebnisses mit fiktiven Beispieldaten',
  ],
};
const sections = topics
  .map((topic) => {
    const steps = topic.steps?.length
      ? `<ol>${topic.steps.map((step) => `<li>${esc(step)}</li>`).join('')}</ol>`
      : '';
    const example = topic.example ? `<pre><code>${esc(topic.example)}</code></pre>` : '';
    const image = images[topic.id];
    const figure = image
      ? `<figure><a href="${image[0]}"><img src="${image[0]}" alt="${esc(image[1])}" loading="lazy"></a><figcaption>Browseraufnahme des Entwicklungsstands mit fiktiven lokalen Daten. Kein Windows- oder NVDA-Abnahmenachweis.</figcaption></figure>`
      : '';
    return `<section id="${esc(topic.id)}"><h2>${esc(topic.title)}</h2>${topic.paragraphs.map((p) => `<p>${esc(p)}</p>`).join('')}${steps}${example}${figure}</section>`;
  })
  .join('');
const html = `<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="theme-color" content="#fafafa"><title>FBForge Inspect · Hilfe 0.1</title><link rel="canonical" href="https://fbforge.de/inspect/hilfe/0.1/"><link rel="icon" href="/assets/fbforge-favicon-v1.svg" type="image/svg+xml"><script src="/theme-v8.js"></script><link rel="preload" href="/assets/fonts/InterVariable.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="/fbforge-v8.css"><link rel="stylesheet" href="/brand-v1.css"><link rel="stylesheet" href="/guide-v1.css"><script src="/fbforge-v7.js" defer></script></head>
<body class="guide-page"><a class="skip-link" href="#inhalt">Zum Inhalt springen</a>${header}
<div class="guide-layout"><nav aria-label="Hilfethemen"><h2>In dieser Anleitung</h2><ul>${topics.map((t) => `<li><a href="#${esc(t.id)}">${esc(t.title)}</a></li>`).join('')}</ul></nav>
<main id="inhalt" tabindex="-1"><p class="edition">IN ENTWICKLUNG</p><h1>FBForge Inspect verstehen und verwenden</h1><p class="intro">Anleitung zum Entwicklungsstand. Noch kein freigegebener Installer. Fachliche Inhalte und Beispiele stehen auch in der Desktop-App offline bereit.</p>${sections}</main></div>${footer}</body></html>`;
const output = 'dist-help/inspect/hilfe/0.1';
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await writeFile(`${output}/index.html`, html);
for (const [file] of Object.values(images))
  await copyFile(`docs/images/${file}`, `${output}/${file}`);
console.log('Statische Hilfe mit gemeinsamer Website-Gestaltung aus docs/help.json erzeugt.');
