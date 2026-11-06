<script setup lang="ts">
import { ref } from 'vue';
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
const emit = defineEmits<{ context: [environment: string, apiVersion: string] }>();
const source = ref(''),
  error = ref('');
const pending = ref<PortableJob | null>(null);
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
      const r = props.batch.results[c.id];
      return (
        !r || r.error || r.report?.findings.some((f) => ['error', 'unknown'].includes(f.status))
      );
    })
    .map((c) => c.id);
}
</script>
<template>
  <section class="panel analysis-context">
    <h2>Mehrere öffentliche Prüfungen vorbereiten</h2>
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
        <label class="file-label"
          >Prüfauftrag öffnen<input type="file" accept=".json" @change="file"
        /></label>
      </div>
    </fieldset>
    <p class="hint">
      Importierte Aufträge starten keine Verbindung. Es gilt der unter Netzwerk eingerichtete
      Verbindungsweg. Zugangsdaten werden nie exportiert.
    </p>
    <p v-if="error" role="alert" class="error-text">{{ error }}</p>
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
    <ol class="check-queue">
      <li v-for="check in batch.checks" :key="check.id">
        <strong>{{ check.id }} · {{ check.operation }}</strong>
        <p class="mono">
          {{ check.input.destinationId || check.input.service || 'Öffentliche Prüfung' }}
        </p>
        <p>
          {{
            batch.activeId === check.id
              ? 'Wird geprüft'
              : batch.results[check.id]?.error ||
                (batch.results[check.id]?.report ? 'Abgeschlossen' : 'Noch nicht ausgeführt')
          }}
        </p>
        <details v-if="batch.results[check.id]?.report">
          <summary>Befunde anzeigen</summary>
          <p v-for="(finding, i) in batch.results[check.id]!.report!.findings" :key="i">
            <strong>{{ finding.title }}</strong> · {{ finding.detail }}
          </p>
        </details>
        <button
          v-if="batch.results[check.id] && !batch.busy"
          class="button secondary"
          @click="batch.start([check.id])"
        >
          Diese Prüfung wiederholen
        </button>
      </li>
    </ol>
  </section>
</template>
