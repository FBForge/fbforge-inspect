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
  batch: [
    ['batch.png', 'Sammelprüfung mit ausdrücklich vorbereiteten öffentlichen Prüfeingaben'],
    [
      'batch-results.png',
      'Fiktive Sammelprüfung mit Ergebniszählern, Filter Handlungsbedarf und sichtbarem nächsten Schritt',
    ],
    [
      'batch-diagnosis.png',
      'Fiktive Sammelprüfung mit kompakter Diagnosezusammenfassung und Buttons für Einzelbefunde, Wiederholung und Entfernen',
    ],
  ],
  tools: [
    ['tools.png', 'Werkzeuge mit Rückwärtsrouting für einen bekannten Zustellpunkt'],
    [
      'tools-busy.png',
      'Laufende fiktive Rückwärtsprüfung mit Fortschrittsanzeige und erreichbarem Abbrechen',
    ],
  ],
  inventory: [
    ['inventory.png', 'Zertifikatsinventar mit fiktiven öffentlichen Zertifikatszuordnungen'],
  ],
  server: [
    [
      'server.png',
      'Serverdiagnose mit vorbereitetem Prüfauftrag, gemeinsamem CLI-Export und Dateiimport',
    ],
    [
      'server-association.png',
      'Fiktives Serverergebnis mit passendem Auftragsfingerabdruck und ausdrücklich begrenzter Herkunftsaussage',
    ],
    [
      'server-diagnosis.png',
      'Fiktives Serverergebnis mit gemeinsamer Diagnosezusammenfassung und gezielt aufklappbaren bestätigten Einzelprüfungen',
    ],
  ],
  updates: [['updates.png', 'Info und ausdrücklich gestartete integrierte Aktualisierung']],
  start: [['start.png', 'FBForge Inspect mit Endpunktabfragen und lokaler Dateiprüfung']],
  destination: [
    [
      'destination.png',
      'Destination-Prüfung mit UUID, optionalem Konfigurationsvergleich und Zertifikatsprüfung',
    ],
    [
      'diagnosis-summary.png',
      'Ergebnisübersicht mit getrennten Nachweisen für Zustellpunkt, Schlüssel, Zertifikate und Sperrstatus',
    ],
  ],
  routing: [
    [
      'routing.png',
      'Auswählbarer PVOG-Vorschlag zur Routingabfrage mit Leistungsbeschreibungs-ID und OE-Verweisen',
    ],
  ],
  process: [
    [
      'process.png',
      'Öffentliche Prozessnachrichtensuche mit Modell, Nachricht und Region in API v3',
    ],
    ['limits.png', 'Separate Limits-Abfrage mit Absender- und Empfänger-Destination in API v3'],
  ],
  keys: [
    [
      'keys.png',
      'Öffentliche JWK-Prüfung mit einem fiktiven lokal erzeugten öffentlichen RSA-Schlüssel',
    ],
  ],
  revocation: [
    [
      'revocation.png',
      'Zertifikatsoptionen mit aktivierter Online-Sperrprüfung und mitgeliefertem V-PKI-Truststore',
    ],
  ],
  schemas: [
    [
      'schema.png',
      'Ausschnitt eines tatsächlichen lokalen Schema-Diagnoseergebnisses mit fiktiven Beispieldaten',
    ],
  ],
  mapping: [
    ['mapping-inputs.png', 'Lokale Quell- und Zielschemata mit optionalen Excel-Anforderungen'],
    [
      'mapping.png',
      'Automatische Strukturzuordnung mit fiktiven Excel-Anforderungen und offenen Zusammenführungen',
    ],
  ],
  reports: [
    [
      'reports.png',
      'Vollständige HTML-Exportvorschau mit standardmäßig ausgeschlossenen Rohantworten',
    ],
  ],
  limits: [
    ['network.png', 'Netzwerkbereich mit manuellem Proxy und optionaler Anmeldung'],
    [
      'network-failure.png',
      'Fiktive fehlende Proxy-Anmeldung mit nicht nachgewiesenen Folgeschritten und nächster Aktion',
    ],
  ],
};
function workflow(topic) {
  if (!topic.workflow) return '';
  const flow = topic.workflow;
  return `<figure class="guide-workflow" aria-labelledby="${esc(topic.id)}-workflow"><figcaption><h3 id="${esc(topic.id)}-workflow">${esc(flow.title)}</h3><p>${esc(flow.intro)}</p></figcaption><ol class="guide-flow">${flow.steps
    .map(
      (step) =>
        `<li><p class="guide-flow-source">${esc(step.source)}</p><h4>${esc(step.title)}</h4><p>${esc(step.text)}</p>${step.requests?.length ? `<details><summary>Aufrufe und Datenquellen</summary>${step.requests.map((request) => `<pre><code>${esc(request)}</code></pre>`).join('')}</details>` : ''}</li>`,
    )
    .join('')}</ol><p class="guide-flow-note">${esc(flow.note)}</p></figure>`;
}
const sections = topics
  .map((topic) => {
    const steps = topic.steps?.length
      ? `<ol>${topic.steps.map((step) => `<li>${esc(step)}</li>`).join('')}</ol>`
      : '';
    const example = topic.example ? `<pre><code>${esc(topic.example)}</code></pre>` : '';
    const figure = (images[topic.id] || [])
      .map(
        (image) =>
          `<figure><a href="${image[0]}"><img src="${image[0]}" alt="${esc(image[1])}" loading="lazy"></a><figcaption>Browseraufnahme des Entwicklungsstands mit fiktiven lokalen Daten. Kein Windows- oder NVDA-Abnahmenachweis.</figcaption></figure>`,
      )
      .join('');
    return `<section id="${esc(topic.id)}"><h2>${esc(topic.title)}</h2>${topic.summary ? `<p>${esc(topic.summary)}</p>` : ''}${steps}${workflow(topic)}${topic.paragraphs.map((p) => `<p>${esc(p)}</p>`).join('')}${example}${figure}</section>`;
  })
  .join('');
const html = `<!doctype html>
<html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta name="theme-color" content="#fafafa"><title>FBForge Inspect · Hilfe 0.2</title><link rel="canonical" href="https://fbforge.de/inspect/hilfe/0.2/"><link rel="icon" href="/assets/fbforge-favicon-v1.svg" type="image/svg+xml"><script src="/theme-v8.js"></script><link rel="preload" href="/assets/fonts/InterVariable.woff2" as="font" type="font/woff2" crossorigin><link rel="stylesheet" href="/fbforge-v8.css"><link rel="stylesheet" href="/brand-v1.css"><link rel="stylesheet" href="/guide-v1.css"><link rel="stylesheet" href="/inspect/guide-flows-v1.css"><script src="/fbforge-v7.js" defer></script></head>
<body class="guide-page"><a class="skip-link" href="#inhalt">Zum Inhalt springen</a>${header}
<div class="guide-layout"><nav aria-label="Hilfethemen"><h2>In dieser Anleitung</h2><ul>${topics.map((t) => `<li><a href="#${esc(t.id)}">${esc(t.title)}</a></li>`).join('')}</ul></nav>
<main id="inhalt" tabindex="-1"><p class="edition">VERSION 0.2.0</p><h1>FBForge Inspect verstehen und verwenden</h1><p class="intro">Anleitung zu FBForge Inspect 0.2.0. Die App enthält zusätzlich eine durchsuchbare Offline-Anleitung. Diese Website erläutert die Abläufe mit Bildern. Lokale Schlüssel- und Schemainspektion funktionieren ohne Internet.</p>${sections}</main></div>${footer}</body></html>`;
const output = 'dist-help/inspect/hilfe/0.2';
await rm(output, { recursive: true, force: true });
await mkdir(output, { recursive: true });
await writeFile(`${output}/index.html`, html);
for (const [file] of Object.values(images).flat())
  await copyFile(`docs/images/${file}`, `${output}/${file}`);
console.log('Statische Hilfe mit gemeinsamer Website-Gestaltung aus docs/help.json erzeugt.');
