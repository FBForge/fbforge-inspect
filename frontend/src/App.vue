<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { pages, type Page } from './contracts';
import DiagnosisForm from './DiagnosisForm.vue';
import Results from './Results.vue';
import Icon from './Icon.vue';
import help from '../../docs/help.json';
import { useTheme } from './useTheme';
import AppearancePicker from './AppearancePicker.vue';
import { useWorkspace } from './useWorkspace';
import { useDiagnosis } from './useDiagnosis';
import ExportDialog from './ExportDialog.vue';
import HelpView from './HelpView.vue';
import StartView from './StartView.vue';
import InfoView from './InfoView.vue';
const { appearance, systemDark } = useTheme();
const page = ref<Page>('start'),
  environment = ref('TEST'),
  apiVersion = ref('v3');
const {
  sessions,
  processMode,
  schemaTrail,
  sessionKey,
  session,
  report,
  error,
  status,
  prepareSchemaReference,
  moveSchema,
} = useWorkspace(page, environment, apiVersion);
const preview = ref(false);
const mainContent = ref<HTMLElement>();
const exportTrigger = ref<HTMLButtonElement>();
const form = ref<InstanceType<typeof DiagnosisForm>>();
const active = computed(() => pages.find((item) => item.id === page.value)!);
const { busy, clear, run, cancel } = useDiagnosis(session, environment, apiVersion, () => {
  preview.value = false;
});
watch(preview, async (open) => {
  if (!open) {
    await nextTick();
    exportTrigger.value?.focus();
  }
});
function inspectPvog(id: string) {
  if (page.value !== 'routing' || busy.value || !id || id.length > 255) return;
  session.value.form.lbId = id;
  void run('routing', { ...session.value.form });
}
function openNetworkHelp() {
  page.value = 'help';
  session.value.view.helpTopic = help.findIndex((topic) => topic.id === 'limits');
}
watch([page, environment, apiVersion], () => {
  preview.value = false;
});
watch(sessionKey, async (key, previous) => {
  if (previous && sessions[previous])
    sessions[previous]!.scrollTop = mainContent.value?.scrollTop || 0;
  await nextTick();
  if (sessionKey.value !== key) return;
  if (mainContent.value) mainContent.value.scrollTop = session.value.scrollTop;
  document.querySelector<HTMLElement>('#page-title')?.focus({ preventScroll: true });
});
async function openDestination(id: string) {
  page.value = 'destination';
  await nextTick();
  form.value?.setDestination(id);
}
async function openSchema(url: string) {
  prepareSchemaReference();
  page.value = 'schema';
  await nextTick();
  form.value?.setSchema(url);
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
            {{ page === 'process' && processMode === 'limits' ? 'Limits prüfen' : active.label }}
          </h1>
          <p>
            {{
              page === 'process' && processMode === 'limits'
                ? 'Öffentliche Größenlimits zwischen zwei Zustellpunkten vergleichen.'
                : active.description
            }}
          </p>
        </div>
        <button v-if="report" ref="exportTrigger" class="button secondary" @click="preview = true">
          Bericht exportieren
        </button>
      </div>
      <main id="main-content" ref="mainContent" tabindex="0">
        <template v-if="page === 'start'">
          <StartView @navigate="page = $event" @network="openNetworkHelp" />
        </template>
        <template v-else-if="!['help', 'about'].includes(page)"
          ><div v-if="environment === 'STAGE' && page === 'routing'" class="notice">
            STAGE hat keine eigene Routing-API. Für diese Ansicht wird ausdrücklich PROD-Routing
            verwendet.
          </div>
          <div
            v-if="page === 'schema' && schemaTrail.keys.length > 1"
            class="schema-history panel"
            aria-label="Schema-Arbeitsstände"
          >
            <button
              class="button secondary"
              :disabled="busy || schemaTrail.index === 0"
              @click="moveSchema(-1)"
            >
              Vorheriges Schema
            </button>
            <span>Schema {{ schemaTrail.index + 1 }} von {{ schemaTrail.keys.length }}</span>
            <button
              class="button secondary"
              :disabled="busy || schemaTrail.index === schemaTrail.keys.length - 1"
              @click="moveSchema(1)"
            >
              Nächstes Schema
            </button>
          </div>
          <div
            v-if="page === 'process'"
            class="task-switch"
            role="group"
            aria-label="Prozesssuche oder Limits"
          >
            <button
              class="button secondary"
              :aria-pressed="processMode === 'process'"
              :disabled="busy"
              @click="processMode = 'process'"
            >
              Prozessnachricht suchen
            </button>
            <button
              class="button secondary"
              :aria-pressed="processMode === 'limits'"
              :disabled="busy"
              @click="processMode = 'limits'"
            >
              Limits prüfen
            </button>
          </div>
          <DiagnosisForm
            ref="form"
            :key="sessionKey"
            :draft="session.form"
            :view="session.view"
            :operation="page === 'process' ? processMode : page"
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
          <HelpView :topics="help" :view="session.view"
        /></template>
        <InfoView v-else @network="openNetworkHelp" />
        <KeepAlive>
          <Results
            v-if="report && !['start', 'help', 'about'].includes(page)"
            :key="sessionKey"
            :report="report"
            @destination="openDestination"
            @schema="openSchema"
            @pvog="inspectPvog"
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
  <ExportDialog
    v-if="preview && report"
    :report="report"
    @close="preview = false"
    @saved="status = 'Bericht lokal gespeichert'"
  />
</template>
