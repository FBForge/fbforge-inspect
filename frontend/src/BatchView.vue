<script setup lang="ts">
import { computed, nextTick, ref, useId } from 'vue';
import Findings from './Findings.vue';
import DisclosurePanel from './DisclosurePanel.vue';
import FileButton from './FileButton.vue';
import { resultState, outcomeNames, operationNames, attention } from './workflow';
import FailureGuidance from './FailureGuidance.vue';
import { checksFromText, parseJob, recognizeInput, type PortableJob } from './portable';
import type { NetworkSettings } from './network';
import { saveFile } from './saveFile';
import type { useBatch } from './useBatch';
import type { UnwrapNestedRefs } from 'vue';
const props = defineProps<{
  batch: UnwrapNestedRefs<ReturnType<typeof useBatch>>;
  environment: string;
  apiVersion: string;
  settings: NetworkSettings;
}>();
const emit = defineEmits<{ context: [environment: string, apiVersion: string]; network: [] }>();
const source = ref(''),
  error = ref('');
const pending = ref<PortableJob | null>(null);
const kind = ref('destination'),
  target = ref(''),
  service = ref(''),
  region = ref('');
const filter = ref('all');
const queue = ref<HTMLOListElement | null>(null);
const addButton = ref<HTMLButtonElement | null>(null);
const filterButton = ref<HTMLButtonElement | null>(null);
const expanded = ref<Record<string, boolean>>(Object.create(null));
const resultId = useId();
const counts = computed(() => ({
  error: props.batch.checks.filter((c) => resultState(props.batch.results[c.id]) === 'error')
    .length,
  unknown: props.batch.checks.filter((c) => resultState(props.batch.results[c.id]) === 'unknown')
    .length,
  warning: props.batch.checks.filter((c) => resultState(props.batch.results[c.id]) === 'warning')
    .length,
  pending: props.batch.checks.filter((c) => resultState(props.batch.results[c.id]) === 'pending')
    .length,
  notApplicable: props.batch.checks.filter(
    (c) => resultState(props.batch.results[c.id]) === 'notApplicable',
  ).length,
  confirmed: props.batch.checks.filter(
    (c) => resultState(props.batch.results[c.id]) === 'confirmed',
  ).length,
}));
const countSummary = computed(() => {
  const names = {
    error: 'mit Handlungsbedarf',
    unknown: 'nicht nachgewiesen',
    warning: 'mit Hinweisen',
    confirmed: 'bestätigt',
    pending: 'noch nicht ausgeführt',
    notApplicable: 'nicht anwendbar',
  };
  return Object.entries(counts.value)
    .filter(([, count]) => count > 0)
    .map(([status, count]) => ({ status, count, label: names[status as keyof typeof names] }));
});
const visibleChecks = computed(() =>
  props.batch.checks.filter(
    (c) =>
      filter.value === 'all' ||
      (filter.value === 'attention'
        ? ['error', 'unknown', 'warning'].includes(resultState(props.batch.results[c.id]))
        : resultState(props.batch.results[c.id]) === filter.value),
  ),
);
async function removeCheck(id: string) {
  const index = visibleChecks.value.findIndex((check) => check.id === id);
  if (!props.batch.remove(id)) return;
  delete expanded.value[id];
  await nextTick();
  const buttons = queue.value?.querySelectorAll<HTMLButtonElement>('button[data-remove-check]');
  const next = buttons?.[Math.min(index, buttons.length - 1)];
  (next || (props.batch.checks.length ? filterButton.value : addButton.value))?.focus();
}
function addCheck() {
  try {
    const line =
      kind.value === 'network'
        ? 'network'
        : kind.value === 'routing'
          ? `routing,${service.value.trim()},${region.value.trim()}`
          : target.value.trim();
    const context = recognizeInput(line);
    if (context?.environment && context.environment !== props.environment)
      throw new Error(`Die URL gehört zu ${context.environment}. Zuerst diese Umgebung auswählen.`);
    if (context?.apiVersion && context.apiVersion !== props.apiVersion)
      throw new Error(
        `Die URL verwendet ${context.apiVersion}. Zuerst diese API-Version auswählen.`,
      );
    const added = checksFromText(line);
    if (added.length !== 1) throw new Error('Genau eine Prüfung angeben.');
    if (added[0]!.operation !== kind.value)
      throw new Error(
        'Die Eingabe passt nicht zur ausgewählten Prüfung. Prüfung und Eingabe abgleichen.',
      );
    let number = 1;
    while (props.batch.checks.some((c) => c.id === `check-${number}`)) number++;
    props.batch.append(
      { ...added[0]!, id: `check-${number}` },
      props.environment,
      props.apiVersion,
    );
    error.value = '';
    target.value = '';
    service.value = '';
    region.value = '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Eingabe ungültig.';
  }
}
function prepare() {
  try {
    for (const line of source.value.split(/\r?\n/)) {
      const context = recognizeInput(line);
      if (context?.environment && context.environment !== props.environment)
        throw new Error(
          `Die eingefügte URL gehört zu ${context.environment}. Wählen Sie vor dem Vorbereiten diese Umgebung.`,
        );
      if (context?.apiVersion && context.apiVersion !== props.apiVersion)
        throw new Error(
          `Die eingefügte URL verwendet ${context.apiVersion}. Wählen Sie vor dem Vorbereiten diese API-Version.`,
        );
    }
    props.batch.setJob({
      format: 'fbforge-inspect-job',
      formatVersion: 1,
      environment: props.environment,
      apiVersion: props.apiVersion,
      checks: checksFromText(source.value),
    });
    error.value = '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Eingabe ungültig.';
  }
}
async function file(event: Event) {
  const input = event.target as HTMLInputElement,
    selected = input.files?.[0];
  input.value = '';
  if (!selected) return;
  pending.value = null;
  try {
    if (selected.size > 2 * 1024 * 1024) throw new Error('Prüfauftrag überschreitet 2 MiB.');
    const job = parseJob(await selected.text());
    if (
      job.network &&
      (job.network.mode !== props.settings.mode ||
        (job.network.mode === 'manual' &&
          job.network.proxyUrl !== props.settings.proxyUrl.trim()) ||
        (job.network.httpsCaPem || '') !== props.settings.httpsCaPem)
    ) {
      pending.value = job;
      error.value =
        'Der Auftrag enthält einen abweichenden Verbindungsweg. Prüfen Sie die Angaben vor der Übernahme.';
      return;
    }
    props.batch.setJob(job);
    emit('context', job.environment, job.apiVersion);
    pending.value = null;
    error.value = '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Datei ungültig.';
  }
}
function applyPending() {
  if (!pending.value?.network || props.batch.busy) return;
  props.settings.mode = pending.value.network.mode;
  props.settings.proxyUrl = pending.value.network.proxyUrl || '';
  props.settings.httpsCaPem = pending.value.network.httpsCaPem || '';
  props.settings.username = '';
  props.settings.password = '';
  props.batch.setJob(pending.value);
  emit('context', pending.value.environment, pending.value.apiVersion);
  pending.value = null;
  error.value = '';
}
async function save(kind: 'job' | 'result') {
  try {
    await saveFile(JSON.stringify(kind === 'job' ? props.batch.job() : props.batch.result()));
  } catch {
    error.value = 'Datei konnte nicht gespeichert werden.';
  }
}
function retryIds() {
  return props.batch.checks
    .filter((c) => {
      return ['pending', 'error', 'unknown'].includes(resultState(props.batch.results[c.id]));
    })
    .map((c) => c.id);
}
</script>
<template>
  <section class="panel analysis-context">
    <h2>Mehrere öffentliche Prüfungen vorbereiten</h2>
    <form @submit.prevent="addCheck">
      <fieldset :disabled="batch.busy">
        <label
          >Prüfung auswählen<select v-model="kind">
            <option value="destination">Zustellpunkt</option>
            <option value="routing">Routing</option>
            <option value="network">Netzwerk</option>
          </select></label
        >
        <label v-if="kind === 'destination'"
          >Kennung oder öffentliche URL<input v-model="target" required spellcheck="false"
        /></label>
        <template v-if="kind === 'routing'">
          <label>LeiKa-Schlüssel<input v-model="service" required pattern="[0-9]{14}" /></label>
          <label
            >Vollständiger ARS<input v-model="region" required pattern="(DE)?[0-9]{12}"
          /></label>
        </template>
        <button ref="addButton" class="button secondary" type="submit">Prüfung hinzufügen</button>
      </fieldset>
    </form>
    <DisclosurePanel label="Mehrere Kennungen einfügen und Aufträge öffnen">
      <p>
        Pro Zeile eine Destination-ID, eine unterstützte FIT-Connect-URL oder <code>network</code>.
        Für Routing: <code>routing,LeiKa,ARS</code>. Höchstens 32 Prüfungen.
      </p>
      <fieldset :disabled="batch.busy">
        <label
          >Prüfeingaben<textarea
            v-model="source"
            rows="5"
            spellcheck="false"
            placeholder="00000000-0000-4000-8000-000000000001"
          />
        </label>
        <div class="action-row">
          <button class="button secondary" @click="prepare">Prüfungen vorbereiten</button>
          <FileButton
            label="Prüfauftrag öffnen"
            accept=".json"
            :disabled="batch.busy"
            @change="file"
          />
        </div>
      </fieldset>
      <p class="hint">
        Importierte Aufträge starten keine Verbindung. Es gilt der unter Netzwerk eingerichtete
        Verbindungsweg. Zugangsdaten werden nie exportiert.
      </p>
    </DisclosurePanel>
    <p v-if="error" role="alert" class="error-text">{{ error }}</p>
    <p v-if="!batch.checks.length" role="status">{{ batch.message }}</p>
    <div v-if="pending" class="notice">
      <h3>Verbindungsweg aus dem Auftrag</h3>
      <pre tabindex="0">{{ JSON.stringify(pending.network, null, 2) }}</pre>
      <p>
        Die Übernahme ersetzt den Verbindungsweg und entfernt bisherige Proxy-Anmeldedaten aus der
        Sitzung. Eine neue Anmeldung richten Sie unter Netzwerk ein. Es wird keine Verbindung
        gestartet.
      </p>
      <button class="button secondary" :disabled="batch.busy" @click="applyPending">
        Verbindungsweg und Auftrag übernehmen
      </button>
      <button class="button secondary" :disabled="batch.busy" @click="pending = null">
        Übernahme verwerfen
      </button>
    </div>
  </section>
  <section v-if="batch.checks.length" class="panel analysis-context">
    <h2>Prüfwarteschlange · {{ batch.environment }} · {{ batch.apiVersion }}</h2>
    <p>
      Die Prüfungen laufen einzeln mit getrennten Abfragebudgets. Eine Pause wirkt nach der
      aktuellen Prüfung.
    </p>
    <div class="action-row">
      <button class="button primary" :disabled="batch.busy" @click="batch.start()">
        Sammelprüfung starten
      </button>
      <button
        v-if="batch.busy && !batch.paused"
        class="button secondary"
        @click="batch.paused = true"
      >
        Nach aktueller Prüfung pausieren
      </button>
      <button v-if="batch.busy && batch.paused" class="button secondary" @click="batch.resume()">
        Fortsetzen
      </button>
      <button
        v-if="batch.busy"
        class="button secondary"
        :disabled="batch.stopping"
        @click="batch.stop()"
      >
        Sammelprüfung abbrechen
      </button>
      <button
        v-if="!batch.busy && Object.keys(batch.results).length"
        class="button secondary"
        :disabled="!retryIds().length"
        @click="batch.start(retryIds())"
      >
        Offene und fehlgeschlagene Prüfungen wiederholen
      </button>
      <button class="button secondary" :disabled="batch.busy" @click="save('job')">
        Auftrag für Server speichern
      </button>
      <button
        v-if="Object.keys(batch.results).length"
        class="button secondary"
        :disabled="batch.busy"
        @click="save('result')"
      >
        Ergebnisse speichern
      </button>
    </div>
    <p role="status">{{ batch.message }}</p>
    <div class="queue-summary" role="status">
      <span v-for="item in countSummary" :key="item.status"
        ><strong>{{ item.count }}</strong> {{ item.label }}</span
      >
    </div>
    <p class="hint">
      Die Übersicht bewertet die ausgewählten Einzelprüfungen. Sie ist keine Gesamtfreigabe der
      Anbindung.
    </p>
    <div class="queue-filters" role="group" aria-label="Prüfwarteschlange filtern">
      <button
        ref="filterButton"
        class="button secondary"
        :aria-pressed="filter === 'all'"
        @click="filter = 'all'"
      >
        Alle Prüfungen · {{ batch.checks.length }}
      </button>
      <button
        class="button secondary"
        :aria-pressed="filter === 'attention'"
        @click="filter = 'attention'"
      >
        Offene Befunde und Hinweise · {{ counts.error + counts.unknown + counts.warning }}
      </button>
      <button class="button secondary" :aria-pressed="filter === 'error'" @click="filter = 'error'">
        Handlungsbedarf · {{ counts.error }}
      </button>
      <button
        class="button secondary"
        :aria-pressed="filter === 'unknown'"
        @click="filter = 'unknown'"
      >
        Nicht nachgewiesen · {{ counts.unknown }}
      </button>
    </div>
    <p v-if="!visibleChecks.length">Keine Prüfungen für diesen Filter.</p>
    <ol ref="queue" class="check-queue" role="list">
      <li v-for="check in visibleChecks" :key="check.id">
        <div class="check-heading">
          <h3>
            {{ operationNames[check.operation] || check.operation }}
            <span class="hint">· {{ check.id }}</span>
          </h3>
          <span :class="resultState(batch.results[check.id])" class="badge">
            {{
              batch.activeId === check.id
                ? 'Wird geprüft'
                : outcomeNames[resultState(batch.results[check.id])]
            }}
          </span>
        </div>
        <p class="mono">
          {{ check.input.destinationId || check.input.service || 'Öffentliche Prüfung' }}
        </p>
        <p v-if="batch.results[check.id]?.error">
          {{
            batch.results[check.id]?.error === 'RESULT_SIZE_LIMIT'
              ? 'Das Ergebnis überschreitet das Speicherbudget. Weniger Prüfungen in einem Auftrag ausführen.'
              : 'Die Prüfung konnte nicht abgeschlossen werden. Eingaben und Verbindungsweg prüfen und diesen Eintrag erneut ausführen.'
          }}
        </p>
        <DisclosurePanel
          v-if="batch.results[check.id]?.error"
          label="Technischen Fehlercode anzeigen"
        >
          <code>{{ batch.results[check.id]?.error }}</code>
        </DisclosurePanel>
        <p v-if="attention(batch.results[check.id]?.report)">
          <strong>{{ attention(batch.results[check.id]?.report)!.title }}</strong> ·
          {{ attention(batch.results[check.id]?.report)!.detail }}
        </p>
        <p v-if="attention(batch.results[check.id]?.report)?.nextStep">
          <strong>Nächster Schritt:</strong>
          {{ attention(batch.results[check.id]?.report)!.nextStep }}
        </p>
        <div class="action-row">
          <button
            v-if="batch.results[check.id]?.report"
            class="button secondary"
            :aria-expanded="!!expanded[check.id]"
            :aria-controls="`${resultId}-${check.id}`"
            @click="expanded[check.id] = !expanded[check.id]"
          >
            {{ expanded[check.id] ? 'Befunde ausblenden' : 'Befunde anzeigen' }}
          </button>
          <button
            v-if="batch.results[check.id] && !batch.busy"
            class="button secondary"
            @click="batch.start([check.id])"
          >
            Diese Prüfung wiederholen
          </button>
          <button
            class="button secondary"
            data-remove-check
            :aria-label="`Prüfung entfernen: ${check.id}`"
            :disabled="batch.busy"
            @click="removeCheck(check.id)"
          >
            Prüfung entfernen
          </button>
        </div>
        <div
          v-if="batch.results[check.id]?.report"
          :id="`${resultId}-${check.id}`"
          :hidden="!expanded[check.id]"
          class="check-findings"
        >
          <template v-if="expanded[check.id]">
            <FailureGuidance
              :report="batch.results[check.id]!.report!"
              can-configure
              @network="emit('network')"
            />
            <Findings :report="batch.results[check.id]!.report!" />
          </template>
        </div>
      </li>
    </ol>
  </section>
</template>
