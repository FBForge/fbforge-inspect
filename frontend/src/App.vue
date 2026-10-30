<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { pages, type Page, type Report, type EngineResponse } from './contracts';
import { reportForExport, reportHtml } from './reports';
import { containsPrivateMaterial } from './inputs';
import DiagnosisForm from './DiagnosisForm.vue';
import Results from './Results.vue';
import Icon from './Icon.vue';
import help from '../../docs/help.json';
import { useTheme } from './useTheme';
import AppearancePicker from './AppearancePicker.vue';
import { createSession } from './session';
import { buildCommit } from './build';
const { appearance, systemDark } = useTheme();
const page = ref<Page>('start'),
  environment = ref('TEST'),
  apiVersion = ref('v3');
const sessions = reactive<Record<string, ReturnType<typeof createSession>>>({});
const sessionKey = computed(() => `${environment.value}/${apiVersion.value}/${page.value}`);
const session = computed(() => {
  if (!sessions[sessionKey.value]) sessions[sessionKey.value] = createSession();
  return sessions[sessionKey.value]!;
});
const report = computed({
  get: () => session.value.report,
  set: (value) => {
    session.value.report = value;
  },
});
const error = computed({
  get: () => session.value.error,
  set: (value) => {
    session.value.error = value;
  },
});
const status = computed({
  get: () => session.value.status,
  set: (value) => {
    session.value.status = value;
  },
});
const busy = ref(false),
  preview = ref(false),
  raw = ref(false),
  format = ref('html');
const form = ref<InstanceType<typeof DiagnosisForm>>();
let requestId = 0;
const exportDialog = ref<HTMLDialogElement>();
const exportTrigger = ref<HTMLButtonElement>();
const exportError = ref('');
const saving = ref(false);
const active = computed(() => pages.find((p) => p.id === page.value)!);
const exportReport = computed(() =>
  report.value ? reportForExport(report.value, raw.value) : null,
);
const previewText = computed(() =>
  exportReport.value ? JSON.stringify(exportReport.value, null, 2) : '',
);
watch(preview, async (value) => {
  if (value) {
    raw.value = false;
    exportError.value = '';
    await nextTick();
    exportDialog.value?.showModal();
  } else {
    await nextTick();
    exportTrigger.value?.focus();
  }
});
function clear() {
  report.value = null;
  error.value = '';
  preview.value = false;
  status.value = 'Bereit';
}
watch([page, environment, apiVersion], () => {
  preview.value = false;
});
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
function trapExportFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab' || event.altKey || event.ctrlKey || event.metaKey) return;
  const dialog = exportDialog.value;
  if (!dialog) return;
  const targets = [
    ...dialog.querySelectorAll<HTMLElement>('button, input, select, textarea, a[href], [tabindex]'),
  ].filter(
    (element) =>
      element.tabIndex >= 0 && !element.matches(':disabled') && element.getClientRects().length > 0,
  );
  const first = targets[0];
  const last = targets.at(-1);
  if (!first || !last) return;
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
async function save() {
  if (!exportReport.value || saving.value) return;
  saving.value = true;
  exportError.value = '';
  try {
    const content = format.value === 'html' ? reportHtml(exportReport.value) : previewText.value;
    const saved = await invoke<boolean>('save_report', { content, format: format.value });
    if (saved) {
      preview.value = false;
      status.value = 'Bericht lokal gespeichert';
    }
  } catch {
    exportError.value = 'Bericht konnte nicht gespeichert werden.';
  } finally {
    saving.value = false;
  }
}
</script>
<template>
  <a class="skip-link" href="#main-content">Zum Inhalt</a>
  <div class="app-shell">
    <header class="topbar">
      <div class="brand">
        <img src="/inspect.png" alt="" />
        <div><strong>FBForge</strong><span>Inspect</span></div>
      </div>
      <div class="topbar-context">
        <span
          class="environment-tag"
          :data-environment="environment"
          :title="`Umgebung ${environment}. Keine Sicherheitsbewertung.`"
          aria-hidden="true"
          >{{ environment }}</span
        >
        <span class="build-tag">0.1.0</span>
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
        >
        <AppearancePicker
          :preference="appearance"
          :dark="appearance === 'dark' || (appearance === 'system' && systemDark)"
          @change="appearance = $event"
        />
      </div>
    </header>
    <aside class="sidebar">
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
      <div class="page-heading">
        <div>
          <h1 id="page-title" tabindex="-1">
            {{ active.label }}
          </h1>
          <p>{{ active.description }}</p>
        </div>
        <button v-if="report" ref="exportTrigger" class="button secondary" @click="preview = true">
          Bericht exportieren
        </button>
      </div>
      <main id="main-content" tabindex="-1">
        <template v-if="page === 'start'">
          <section class="start-task-section">
            <p class="eyebrow">IHRE AUFGABE</p>
            <h2>Was möchten Sie prüfen?</h2>
            <p class="muted">
              Wählen Sie eine Diagnose. Eingaben und Ergebnisse bleiben beim Wechsel erhalten.
            </p>
            <div class="start-grid">
              <div v-for="item in pages.slice(1, 6)" :key="item.id" class="start-card panel">
                <Icon :name="item.icon" />
                <button class="start-task-button" @click="page = item.id">
                  <strong>{{ item.label }}</strong>
                </button>
                <p>{{ item.description }}</p>
                <span class="start-task-action" aria-hidden="true">Aufgabe öffnen</span>
              </div>
            </div>
          </section>
          <p class="hint">
            Ohne Zugangsdaten. Öffentliche Quellen. Prüfstände bleiben nur in dieser Sitzung
            erhalten.
          </p>
        </template>
        <template v-else-if="!['help', 'about'].includes(page)"
          ><div v-if="environment === 'STAGE' && page === 'routing'" class="notice">
            STAGE hat keine eigene Routing-API. Für diese Ansicht wird ausdrücklich PROD-Routing
            verwendet.
          </div>
          <DiagnosisForm
            ref="form"
            :key="sessionKey"
            :draft="session.form"
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

          <div v-if="!report && !busy && !error" class="empty-state">
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
            <ol v-if="topic.steps.length">
              <li v-for="step in topic.steps" :key="step">{{ step }}</li>
            </ol>
            <pre
              tabindex="0"
              v-if="topic.example"
              class="help-example"
            ><code>{{ topic.example }}</code></pre>
          </article>
          <p class="hint">
            Grundlagen und aktuelle API-Verträge: docs.fitko.de/fit-connect · pvog.fitko.de
          </p></template
        >
        <section v-else class="panel about">
          <img src="/inspect.png" alt="" width="88" height="88" />
          <h2>FBForge Inspect <span class="muted">0.1.0</span></h2>
          <p class="mono">
            Build: {{ buildCommit === 'local' ? 'Lokaler Arbeitsstand' : buildCommit }}
          </p>
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
        <KeepAlive>
          <Results
            v-if="report && !['start', 'help', 'about'].includes(page)"
            :key="sessionKey"
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
        </KeepAlive>
      </main>
    </div>
    <footer class="statusbar">
      <span role="status" aria-live="polite"><i class="local-dot"></i>{{ status }}</span
      ><span
        >{{ environment }} · {{ apiVersion }}<span class="status-divider">|</span>FBForge Inspect
        0.1.0</span
      >
    </footer>
  </div>
  <dialog
    ref="exportDialog"
    v-if="preview"
    class="export-dialog"
    aria-labelledby="export-title"
    :aria-busy="saving ? 'true' : undefined"
    @keydown="trapExportFocus"
    @cancel.prevent="!saving && (preview = false)"
  >
    <div class="section-heading">
      <h2 id="export-title">Bericht vor dem Speichern prüfen</h2>
      <button
        class="icon-button"
        aria-label="Schließen"
        :disabled="saving"
        @click="preview = false"
      >
        ×
      </button>
    </div>
    <p>
      Dieser Umfang wird als lokale Datei gespeichert. Prüfen Sie die Angaben vor dem Weitergeben.
    </p>
    <div class="form-grid">
      <label
        >Format<select v-model="format" :disabled="saving">
          <option value="html">Eigenständiges HTML</option>
          <option value="json">JSON</option>
        </select></label
      ><label class="check"
        ><input v-model="raw" type="checkbox" :disabled="saving" />Rohantworten einschließen</label
      >
    </div>
    <p v-if="exportError" class="notice error-notice" role="alert">{{ exportError }}</p>
    <pre tabindex="0" class="export-preview">{{ previewText }}</pre>
    <div class="form-footer">
      <button class="button secondary" :disabled="saving" @click="preview = false">Abbrechen</button
      ><button class="button primary" :disabled="saving" @click="save">Speicherort wählen</button>
    </div>
  </dialog>
</template>
