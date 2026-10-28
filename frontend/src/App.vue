<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { pages, type Page, type Report, type EngineResponse } from './contracts';
import { reportForExport, reportHtml } from './reports';
import { containsPrivateMaterial } from './inputs';
import DiagnosisForm from './DiagnosisForm.vue';
import Results from './Results.vue';
import Icon from './Icon.vue';
import help from '../../docs/help.json';
const page = ref<Page>('start'),
  environment = ref('TEST'),
  apiVersion = ref('v3'),
  dark = ref(matchMedia('(prefers-color-scheme: dark)').matches);
const busy = ref(false),
  report = ref<Report | null>(null),
  error = ref(''),
  status = ref('Bereit'),
  preview = ref(false),
  raw = ref(false),
  format = ref('html');
const form = ref<InstanceType<typeof DiagnosisForm>>();
let requestId = 0;
const exportDialog = ref<HTMLDialogElement>();
const active = computed(() => pages.find((p) => p.id === page.value)!);
const exportReport = computed(() =>
  report.value ? reportForExport(report.value, raw.value) : null,
);
const previewText = computed(() =>
  exportReport.value ? JSON.stringify(exportReport.value, null, 2) : '',
);
watch(dark, (value) => (document.documentElement.dataset.theme = value ? 'dark' : 'light'), {
  immediate: true,
});
watch(preview, async (value) => {
  if (value) {
    raw.value = false;
    await nextTick();
    exportDialog.value?.showModal();
  }
});
function clear() {
  report.value = null;
  error.value = '';
  preview.value = false;
  status.value = 'Bereit';
}
watch([page, environment, apiVersion], clear);
watch(page, async () => {
  await nextTick();
  document.querySelector<HTMLElement>('#page-title')?.focus();
});
async function run(operation: string, input: Record<string, unknown>) {
  clear();
  if (!isTauri()) {
    error.value =
      'Die Live-Diagnose läuft in der Desktop-App. Diese Browseransicht zeigt die Oberfläche.';
    return;
  }
  // Reject private material before it crosses IPC. Do not retain an error copy of the input.
  if (
    containsPrivateMaterial(String(input.text), operation === 'keys') ||
    containsPrivateMaterial(String(input.trustPem), true)
  ) {
    error.value = 'Private oder symmetrische Schlüssel sind nicht zulässig.';
    return;
  }
  busy.value = true;
  status.value = 'Öffentliche Daten werden geprüft …';
  const id = ++requestId;
  try {
    const response = await invoke<EngineResponse>('diagnose', {
      id,
      request: {
        protocolVersion: 1,
        operation,
        environment: environment.value,
        apiVersion: apiVersion.value,
        input,
      },
    });
    if (id !== requestId) return;
    if (!response.ok || !response.report)
      throw new Error(response.error || 'Keine Diagnoseantwort.');
    report.value = response.report;
    status.value = 'Prüfung abgeschlossen';
  } catch (e) {
    if (id === requestId) {
      error.value = e instanceof Error ? e.message : String(e);
      status.value = 'Prüfung beendet';
    }
  } finally {
    if (id === requestId) busy.value = false;
  }
}
async function cancel() {
  const id = requestId;
  status.value = 'Prüfung wird abgebrochen …';
  await invoke('cancel', { id });
}
async function openDestination(id: string) {
  page.value = 'destination';
  await nextTick();
  form.value?.setDestination(id);
}
async function openSchema(url: string) {
  page.value = 'schema';
  await nextTick();
  form.value?.setSchema(url);
}
async function save() {
  if (!exportReport.value) return;
  try {
    const content = format.value === 'html' ? reportHtml(exportReport.value) : previewText.value;
    const saved = await invoke<boolean>('save_report', { content, format: format.value });
    if (saved) {
      preview.value = false;
      status.value = 'Bericht lokal gespeichert';
    }
  } catch {
    error.value = 'Bericht konnte nicht gespeichert werden.';
  }
}
</script>
<template>
  <a class="skip-link" href="#main-content">Zum Inhalt</a>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <img src="/inspect.png" alt="" />
        <div>FBForge <strong>Inspect</strong><span>Öffentliche Diagnose</span></div>
      </div>
      <div class="nav-label">ARBEITSBEREICH</div>
      <nav aria-label="Hauptnavigation">
        <button
          v-for="item in pages"
          :key="item.id"
          class="nav-item"
          :class="{ active: page === item.id }"
          :aria-label="item.label"
          :title="item.label"
          :aria-current="page === item.id ? 'page' : undefined"
          :disabled="busy"
          @click="page = item.id"
        >
          <Icon :name="item.icon" /><span>{{ item.label }}</span>
        </button>
      </nav>
      <div class="sidebar-bottom">
        <span class="local-dot"></span>Lokal auf Ihrem Gerät<span class="version">0.1.0</span>
      </div>
    </aside>
    <div class="workspace">
      <header class="topbar">
        <div class="breadcrumb">
          Inspect <span>/</span> <strong>{{ active.label }}</strong>
        </div>
        <div class="toolbar">
          <label
            >Umgebung<select v-model="environment" :disabled="busy">
              <option>TEST</option>
              <option>STAGE</option>
              <option>PROD</option>
            </select></label
          ><label
            >API<select v-model="apiVersion" :disabled="busy">
              <option value="v3">v3</option>
              <option value="v2">v2</option>
            </select></label
          ><button
            class="icon-button"
            :aria-label="dark ? 'Helles Design aktivieren' : 'Dunkles Design aktivieren'"
            @click="dark = !dark"
          >
            <Icon name="sun" />
          </button>
        </div>
      </header>
      <main id="main-content" tabindex="-1">
        <div class="page-heading">
          <div>
            <span class="eyebrow">FBFORGE INSPECT</span>
            <h1 id="page-title" tabindex="-1">
              {{ page === 'start' ? 'Klarheit für Ihre Integration.' : active.label }}
            </h1>
            <p>{{ active.description }}</p>
          </div>
          <button v-if="report" class="button secondary" @click="preview = true">
            Bericht exportieren
          </button>
        </div>
        <template v-if="page === 'start'"
          ><section class="welcome panel">
            <div class="welcome-copy">
              <span class="badge neutral">FIT-Connect v2 &amp; v3</span>
              <h2>Von einer ID<br />zum nachvollziehbaren Befund.</h2>
              <p>
                Destinationen, Routing und Schlüssel prüfen. Quellen sehen, Konfiguration verstehen
                und Ergebnisse lokal weitergeben.
              </p>
              <button class="button primary" @click="page = 'destination'">
                Destination prüfen <Icon name="arrow" />
              </button>
            </div>
            <div class="welcome-art">
              <img src="/inspect.png" alt="FBForge Inspect App-Symbol" />
            </div>
          </section>
          <div class="start-grid">
            <button
              v-for="item in pages.slice(2, 6)"
              :key="item.id"
              class="start-card panel"
              @click="page = item.id"
            >
              <Icon :name="item.icon" />
              <h2>{{ item.label }}</h2>
              <p>{{ item.description }}</p>
              <span class="text-button">Öffnen →</span>
            </button>
          </div>
          <div class="principles">
            <span><i class="local-dot"></i> Ohne Zugangsdaten</span
            ><span><i class="local-dot"></i> Öffentliche Quellen</span
            ><span><i class="local-dot"></i> Keine automatische Speicherung</span>
          </div></template
        >
        <template v-else-if="!['help', 'about'].includes(page)"
          ><div v-if="environment === 'STAGE' && page === 'routing'" class="notice">
            STAGE hat keine eigene Routing-API. Für diese Ansicht wird ausdrücklich PROD-Routing
            verwendet.
          </div>
          <DiagnosisForm
            ref="form"
            :key="page"
            :page="page"
            :busy="busy"
            :api-version="apiVersion"
            @run="run"
            @changed="clear"
          />
          <div v-if="busy" class="busy-panel panel" role="status">
            <span class="spinner" aria-hidden="true"></span>
            <div>
              <strong>Prüfung läuft</strong>
              <p>Öffentliche Antworten werden abgerufen und ausgewertet.</p>
            </div>
            <button class="button secondary" @click="cancel">Abbrechen</button>
          </div>
          <p v-if="error" class="notice error-notice" role="alert">{{ error }}</p>
          <Results
            v-if="report"
            :report="report"
            @destination="openDestination"
            @schema="openSchema"
            @area="
              (id) => {
                form?.setArea(id);
                clear();
              }
            "
          />
          <div v-else-if="!busy && !error" class="empty-state">
            <Icon :name="active.icon" />
            <h2>Bereit für Ihre Prüfung</h2>
            <p>Ergebnis, Befunde und Quellen erscheinen hier nach dem Start.</p>
          </div></template
        >
        <template v-else-if="page === 'help'">
          <nav class="help-index" aria-label="Hilfethemen">
            <a v-for="topic in help" :key="topic.id" :href="`#${topic.id}`">{{ topic.title }}</a>
          </nav>
          <article v-for="topic in help" :id="topic.id" :key="topic.id" class="panel help-topic">
            <h2>{{ topic.title }}</h2>
            <p v-for="text in topic.paragraphs" :key="text">{{ text }}</p>
          </article>
          <p class="hint">
            Grundlagen und aktuelle API-Verträge: docs.fitko.de/fit-connect · pvog.fitko.de
          </p></template
        >
        <section v-else class="panel about">
          <img src="/inspect.png" alt="" width="88" height="88" />
          <h2>FBForge Inspect <span class="muted">0.1.0</span></h2>
          <p>
            Eine eigenständige Open-Source-Desktop-App für die Diagnose öffentlicher FIT-Connect-
            und PVOG-Schnittstellen.
          </p>
          <p>Entwickelt von FBForge. Lizenz: EUPL-1.2.</p>
          <p class="mono">github.com/FBForge/fbforge-inspect</p>
          <p class="hint">
            Kein offizielles Produkt von FITKO. Eine Diagnose ist eine Momentaufnahme und keine
            Zertifizierung.
          </p>
        </section>
      </main>
      <footer class="statusbar">
        <span role="status" aria-live="polite"><i class="local-dot"></i>{{ status }}</span
        ><span
          >{{ environment }} · {{ apiVersion }}<span class="status-divider">|</span>FBForge Inspect
          0.1.0</span
        >
      </footer>
    </div>
  </div>
  <dialog
    ref="exportDialog"
    v-if="preview"
    class="export-dialog"
    aria-labelledby="export-title"
    @cancel="preview = false"
  >
    <div class="section-heading">
      <h2 id="export-title">Bericht vor dem Speichern prüfen</h2>
      <button class="icon-button" aria-label="Schließen" @click="preview = false">×</button>
    </div>
    <p>
      Dieser Umfang wird als lokale Datei gespeichert. Prüfen Sie die Angaben vor dem Weitergeben.
    </p>
    <div class="form-grid">
      <label
        >Format<select v-model="format">
          <option value="html">Eigenständiges HTML</option>
          <option value="json">JSON</option>
        </select></label
      ><label class="check"><input v-model="raw" type="checkbox" />Rohantworten einschließen</label>
    </div>
    <pre class="export-preview">{{ previewText }}</pre>
    <div class="form-footer">
      <button class="button secondary" @click="preview = false">Abbrechen</button
      ><button class="button primary" @click="save">Speicherort wählen</button>
    </div>
  </dialog>
</template>
