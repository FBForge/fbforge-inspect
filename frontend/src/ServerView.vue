<script setup lang="ts">
import { computed, ref } from 'vue';
import { parseJob, type PortableJob } from './portable';
import { jobDigest, operationNames, outcomeNames, resultState } from './workflow';
import FailureGuidance from './FailureGuidance.vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import Findings from './Findings.vue';
import DisclosurePanel from './DisclosurePanel.vue';
import FileButton from './FileButton.vue';
import { parseResult, type PortableResult } from './portable';
import SupportPanel from './SupportPanel.vue';
import NetworkResults from './results/NetworkResults.vue';
import { serverNetworkEvidence } from './serverEvidence';
const props = defineProps<{
  result: PortableResult | null;
  job?: PortableJob | null;
  expectedDigest?: string;
  jobError?: string;
}>();
const emit = defineEmits<{
  imported: [result: PortableResult];
  batch: [];
  prepared: [digest: string];
}>();
const comparisonDigest = ref('');
const invalidReference = ref(false);
let referenceSequence = 0;
const association = computed(() => {
  if (invalidReference.value) return 'Auftragsdatei ungültig. Zuordnung nicht nachgewiesen.';
  const expected = comparisonDigest.value || props.expectedDigest;
  if (!expected)
    return 'Zum Zuordnen den Auftrag exportieren oder die verwendete Auftragsdatei öffnen.';
  if (!props.result?.jobSha256)
    return 'Dieses Ergebnis enthält keinen Auftragsfingerabdruck. Zuordnung nicht nachgewiesen.';
  return props.result.jobSha256 === expected
    ? 'Ergebnis passt zu den Bytes der ausgewählten Auftragsdatei.'
    : 'Das Ergebnis passt nicht zur ausgewählten Auftragsdatei. Auftrag und Ergebnis prüfen.';
});
async function reference(event: Event) {
  const input = event.target as HTMLInputElement,
    file = input.files?.[0];
  input.value = '';
  if (!file) return;
  const sequence = ++referenceSequence;
  invalidReference.value = true;
  try {
    if (file.size > 2 * 1024 * 1024) throw new Error('Auftrag überschreitet 2 MiB.');
    const bytes = new Uint8Array(await file.arrayBuffer());
    parseJob(new TextDecoder().decode(bytes));
    const digest = await jobDigest(bytes);
    if (sequence !== referenceSequence) return;
    comparisonDigest.value = digest;
    invalidReference.value = false;
    error.value = '';
  } catch (e) {
    if (sequence !== referenceSequence) return;
    comparisonDigest.value = '';
    error.value = e instanceof Error ? e.message : 'Auftrag ungültig.';
  }
}
const error = ref(''),
  message = ref('');
async function open(event: Event) {
  const input = event.target as HTMLInputElement,
    selected = input.files?.[0];
  input.value = '';
  if (!selected) return;
  try {
    if (selected.size > 16 * 1024 * 1024) throw new Error('Ergebnis überschreitet 16 MiB.');
    const result = parseResult(await selected.text());
    emit('imported', result);
    error.value = '';
    message.value = 'Serverergebnis geöffnet. Es wurde keine Verbindung gestartet.';
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Ergebnisdatei ungültig.';
  }
}
async function exportCli() {
  try {
    if (props.jobError) throw new Error(props.jobError);
    if (!isTauri())
      throw new Error('Das portable CLI-Paket wird aus der installierten Desktop-App exportiert.');
    const content = props.job ? JSON.stringify(parseJob(JSON.stringify(props.job))) : null;
    if (await invoke<boolean>('export_cli', { jobContent: content })) {
      error.value = '';
      if (content) {
        const digest = await jobDigest(new TextEncoder().encode(content));
        comparisonDigest.value = '';
        invalidReference.value = false;
        referenceSequence++;
        emit('prepared', digest);
      }
      message.value = content
        ? 'CLI und auftrag.json gemeinsam exportiert. Startbefehle stehen unten und in README.txt.'
        : 'Portables Java-CLI mit Windows- und Linux-Startskript exportiert.';
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}
</script>
<template>
  <section class="panel analysis-context">
    <h2>Auf dem tatsächlichen Anwendungssystem prüfen</h2>
    <p>
      Ein erfolgreicher Abruf auf Ihrem Arbeitsplatz sagt nichts über den Netzwerkweg des
      Fachverfahrensservers aus. Führen Sie denselben öffentlichen Prüfauftrag direkt dort aus.
    </p>
    <p class="hint">
      Der Export legt einen neuen CLI-Ordner an. Vorhandene Ordner werden nicht überschrieben.
    </p>
    <p v-if="jobError" role="alert">{{ jobError }}</p>
    <p v-if="job">
      <strong>Auftrag bereit:</strong> {{ job.checks.length }}
      {{ job.checks.length === 1 ? 'Prüfung' : 'Prüfungen' }} · {{ job.environment }} ·
      {{ job.apiVersion }}. Der CLI-Export nimmt auftrag.json mit.
    </p>
    <p v-else>
      Bereiten Sie zunächst einen Auftrag vor. Das CLI lässt sich auch ohne Auftrag exportieren.
    </p>
    <ol class="server-steps" role="list">
      <li>
        <h3>Auftrag vorbereiten</h3>
        <p>Zustellpunkte, Routing oder Netzwerk in der Sammelprüfung auswählen.</p>
        <button class="button secondary" @click="emit('batch')">Prüfauftrag vorbereiten</button>
      </li>
      <li>
        <h3>Auf dem Server ausführen</h3>
        <p>CLI und Auftrag auf das Zielsystem übertragen. Dort mit Java 25 oder neuer starten.</p>
        <button class="button secondary" @click="exportCli" :disabled="!!jobError">
          Portables CLI exportieren
        </button>
      </li>
      <li>
        <h3>Ergebnis auswerten</h3>
        <p>Die Ergebnisdatei vom Server hier öffnen. Der Import startet keine Verbindung.</p>
        <FileButton label="Serverergebnis öffnen" accept=".json" @change="open" />
      </li>
    </ol>
    <DisclosurePanel label="Aufrufe und Rückgabecodes">
      <p>
        Voraussetzung: Java 25 oder neuer. Prüfen Sie auf dem Zielsystem zuerst
        <code>java -version</code>. Bei einer älteren Laufzeit eine passende portable Java-Laufzeit
        separat entpacken und deren java direkt aufrufen. Die Laufzeit der Fachanwendung muss dafür
        nicht geändert werden.
      </p>
      <p>
        Jeder Lauf benötigt einen neuen Ergebnisdateinamen. Bestehende Dateien werden nicht
        überschrieben. Ein erfolgreicher Lauf mit dieser Java-Laufzeit belegt nicht die
        TLS-Konfiguration einer anderen Anwendung.
      </p>
      <p>PowerShell auf Windows:</p>
      <pre tabindex="0">
.\inspect.ps1 --job auftrag.json --output ergebnis.inspect.json
$LASTEXITCODE</pre>
      <p>Linux:</p>
      <pre tabindex="0">
sh ./inspect.sh --job auftrag.json --output ergebnis.inspect.json
echo $?</pre>
      <p>Alternativ auf beiden Plattformen:</p>
      <pre tabindex="0">
java -jar inspect-engine.jar --job auftrag.json --output ergebnis.inspect.json</pre>
      <dl>
        <dt>0</dt>
        <dd>
          Abgeschlossen ohne Fehlerbefund oder ungeklärten Prüfpunkt. Hinweise können vorhanden
          sein.
        </dd>
        <dt>2</dt>
        <dd>Mindestens ein Fehlerbefund.</dd>
        <dt>3</dt>
        <dd>Mindestens eine Prüfung fehlgeschlagen oder nicht vollständig nachgewiesen.</dd>
        <dt>64</dt>
        <dd>Ungültige Eingabe oder Optionen.</dd>
        <dt>70</dt>
        <dd>Interner Fehler.</dd>
        <dt>74</dt>
        <dd>Dateifehler, Größenlimit oder Ergebnisdatei bereits vorhanden.</dd>
      </dl>
      <p>
        Proxy und zusätzliche HTTPS-CA lassen sich ausdrücklich konfigurieren. Proxy-Zugangsdaten
        werden über benannte Umgebungsvariablen eingelesen. <code>--help</code> zeigt die Optionen.
      </p>
    </DisclosurePanel>
    <div class="action-row">
      <FileButton label="Auftragsdatei zur Zuordnung öffnen" accept=".json" @change="reference" />
    </div>
    <p class="hint">
      Die Datei ist ein übertragener Prüfbeleg und kein Herkunftsnachweis des Servers. Inspect führt
      daraus keine Programme aus und startet keine automatischen Folgeabrufe.
    </p>
    <p v-if="message" role="status">{{ message }}</p>
    <p v-if="error" role="alert" class="error-text">{{ error }}</p>
  </section>
  <template v-if="props.result">
    <section class="panel analysis-context">
      <h2>Übertragenes Serverergebnis</h2>
      <p role="status">{{ association }}</p>
      <p class="hint">
        Der Fingerabdruck ordnet Dateien einander zu. Er bestätigt keine Serveridentität.
      </p>
      <p>
        {{ props.result.runtime.os }} · {{ props.result.runtime.architecture }} · Java
        {{ props.result.runtime.javaVersion }}
      </p>
      <p>
        Inspect {{ props.result.appVersion }} · {{ props.result.createdAt }} · Exitcode
        {{ props.result.exitCode }}
      </p>
      <p v-if="props.result.jobSha256" class="mono">
        Auftrag SHA-256: {{ props.result.jobSha256 }}
      </p>
    </section>
    <section v-for="check in props.result.checks" :key="check.id" class="panel analysis-context">
      <div class="check-heading">
        <h3>
          {{
            check.report
              ? operationNames[check.report.operation] || check.report.operation
              : 'Prüfung'
          }}
          <span class="hint">· {{ check.id }}</span>
        </h3>
        <span class="badge" :class="resultState(check)">{{
          outcomeNames[resultState(check)]
        }}</span>
      </div>
      <p v-if="check.report" class="hint">
        {{ check.report.environment }} · {{ check.report.apiVersion }}
      </p>
      <p v-if="check.error" class="error-text">{{ check.error }}</p>
      <template v-if="check.report">
        <FailureGuidance :report="check.report" />
        <NetworkResults :report="serverNetworkEvidence(check.report)" read-only />
        <Findings :report="check.report" />
        <SupportPanel :report="check.report" :runtime="props.result.runtime" />
        <DisclosurePanel label="Übertragene Daten anzeigen">
          <pre tabindex="0">{{ JSON.stringify(check.report.data, null, 2) }}</pre>
        </DisclosurePanel>
      </template>
    </section>
  </template>
</template>
