<script setup lang="ts">
import { ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import { labels } from './contracts';
import { parseResult, type PortableResult } from './portable';
import SupportPanel from './SupportPanel.vue';
import NetworkResults from './results/NetworkResults.vue';
import { serverNetworkEvidence } from './serverEvidence';
const props = defineProps<{ result: PortableResult | null }>();
const emit = defineEmits<{ imported: [result: PortableResult]; batch: [] }>();
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
    if (!isTauri())
      throw new Error('Das portable CLI-Paket wird aus der installierten Desktop-App exportiert.');
    if (await invoke<boolean>('export_cli'))
      message.value = 'Portables Java-CLI mit Windows- und Linux-Startskript exportiert.';
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
    <ol>
      <li>Unter Sammelprüfung die gewünschten Prüfungen vorbereiten und den Auftrag speichern.</li>
      <li>Das portable CLI exportieren und mit dem Auftrag auf das Zielsystem übertragen.</li>
      <li>
        Auf Windows oder Linux mit Java 25 oder neuer ausführen. Verwenden Sie das Benutzerkonto und
        den Netzwerkweg Ihrer Anwendung.
      </li>
      <li>Die Ergebnisdatei hier öffnen und Befunde sowie Zertifikatsinventar auswerten.</li>
    </ol>
    <div class="action-row">
      <button class="button secondary" @click="emit('batch')">Prüfauftrag vorbereiten</button
      ><button class="button secondary" @click="exportCli">Portables CLI exportieren</button>
    </div>
    <details>
      <summary>Aufrufe und Rückgabecodes</summary>
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
    </details>
    <label>Serverergebnis öffnen<input type="file" accept=".json" @change="open" /></label>
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
      <h3>
        {{ check.id
        }}<template v-if="check.report">
          · {{ check.report.operation }} · {{ check.report.environment }}</template
        >
      </h3>
      <p v-if="check.error" class="error-text">{{ check.error }}</p>
      <template v-if="check.report">
        <NetworkResults :report="serverNetworkEvidence(check.report)" read-only />
        <article v-for="(finding, i) in check.report.findings" :key="i" class="imported-finding">
          <h4>{{ finding.title }} · {{ labels[finding.status] }}</h4>
          <p>{{ finding.detail }}</p>
          <p v-if="finding.nextStep">Nächster Schritt: {{ finding.nextStep }}</p>
          <details>
            <summary>Beleg und Grenze</summary>
            <p>{{ finding.path }}</p>
            <p>{{ finding.source }}</p>
            <p>{{ finding.limitation }}</p>
          </details>
        </article>
        <SupportPanel :report="check.report" :runtime="props.result.runtime" />
        <details>
          <summary>Übertragene Daten anzeigen</summary>
          <pre tabindex="0">{{ JSON.stringify(check.report.data, null, 2) }}</pre>
        </details>
      </template>
    </section>
  </template>
</template>
