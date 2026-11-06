<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';
import { records } from '../resultValues';
interface NetworkCheck {
  name?: string;
  host?: string;
  port?: number;
  url?: string;
  method?: string;
  detail?: string;
  status?: string;
  httpStatus?: number;
  durationMs?: number;
  code?: string;
  certificateEndpoint?: boolean;
}
const props = defineProps<{ report: Report; readOnly?: boolean }>();
const emit = defineEmits<{ retry: [input: Record<string, unknown>] }>();
interface TlsEvidence {
  protocol: string;
  cipher: string;
  certificates: { subject: string; issuer: string; notAfter: string; sha256: string }[];
}
interface Trace {
  url: string;
  route: string;
  phases: { stage: string; durationMs: number; outcome: string }[];
  tls?: TlsEvidence;
  proxyTls?: TlsEvidence;
}
const tlsConnections = (trace: Trace) =>
  [
    { label: 'Vorgelegtes Zertifikat des HTTPS-Proxys', evidence: trace.proxyTls },
    { label: 'Vorgelegtes HTTPS-Serverzertifikat', evidence: trace.tls },
  ].filter((item): item is { label: string; evidence: TlsEvidence } => !!item.evidence);
const traces = computed(() => records<Trace>(props.report.data.networkTraces));
const stages: Record<string, string> = {
  dns: 'DNS',
  tcp: 'TCP-Verbindung',
  proxy: 'Proxy-Verbindung und Tunnel',
  proxyDns: 'DNS des Proxys',
  proxyTcp: 'TCP zum Proxy',
  proxyTls: 'TLS zum HTTPS-Proxy',
  proxyConnect: 'Proxy-Tunnel und Anmeldung',
  tls: 'TLS und Serverzertifikat',
  http: 'HTTP-Antwort',
};
const checks = computed(() => records<NetworkCheck>(props.report.data.networkChecks));
const allowlist = computed(() =>
  checks.value
    .map((item) => `${item.method} ${item.url}\nZiel: ${item.host}:${item.port}`)
    .join('\n\n'),
);
</script>
<template>
  <section v-if="checks.length" class="panel">
    <h3>Netzwerkverbindungen</h3>
    <div
      class="table-scroll"
      tabindex="0"
      role="region"
      aria-label="Ergebnisse der Netzwerkprüfung"
    >
      <table>
        <thead>
          <tr>
            <th>Dienst / Ziel</th>
            <th>Ergebnis</th>
            <th>Dauer</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(item, index) in checks" :key="index">
            <td>
              <strong>{{ item.name }}</strong>
              <p class="mono hint">{{ item.method }} {{ item.host }}:{{ item.port }}</p>
            </td>
            <td>
              {{ item.detail }}
              <p v-if="item.code" class="mono hint">{{ item.code }}</p>
              <p v-if="item.httpStatus" class="hint">HTTP {{ item.httpStatus }}</p>
              <button
                v-if="item.url && !readOnly"
                class="button secondary"
                @click="
                  emit('retry', {
                    onlyUrl: item.url,
                    certificates: report.data.networkCertificates || [],
                    certificateEndpoints: item.certificateEndpoint ? [item.url] : [],
                  })
                "
              >
                Dieses Ziel erneut prüfen
              </button>
            </td>
            <td>{{ item.durationMs === undefined ? 'Keine Messung' : `${item.durationMs} ms` }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <details>
      <summary>Freigabeliste für die IT anzeigen</summary>
      <p class="hint">
        Konkrete Zieladressen, Ports und Methoden dieser Prüfung. Proxy-Zugangsdaten sind nicht
        enthalten. Die Liste ist auch Bestandteil des exportierbaren Diagnoseberichts.
      </p>
      <pre tabindex="0">{{ allowlist }}</pre>
    </details>
  </section>
  <details v-if="traces.length" class="panel">
    <summary>Verbindungsphasen und HTTPS-Vertrauen</summary>
    <p v-if="report.data.networkContext">
      {{ (report.data.networkContext as { httpsTrust?: string }).httpsTrust }}
    </p>
    <p class="hint">
      Ein abgebrochener Schritt verhindert spätere Schritte. Nicht erreichte Phasen wurden nicht
      geprüft. HTTPS-Vertrauen und V-PKI-Vertrauen sind getrennte Prüfungen.
    </p>
    <article v-for="(trace, index) in traces" :key="index">
      <h4 class="mono">{{ trace.url }}</h4>
      <p>{{ trace.route === 'manualProxy' ? 'Manueller Proxy' : 'Direkte Verbindung' }}</p>
      <ol class="phase-list">
        <li v-for="(phase, i) in trace.phases" :key="i">
          {{ stages[phase.stage] || phase.stage }} · {{ phase.durationMs }} ms ·
          {{ phase.outcome === 'confirmed' ? 'Abgeschlossen' : phase.outcome }}
        </li>
      </ol>
      <details v-for="connection in tlsConnections(trace)" :key="connection.label">
        <summary>{{ connection.label }}</summary>
        <p>{{ connection.evidence.protocol }} · {{ connection.evidence.cipher }}</p>
        <div v-for="(cert, i) in connection.evidence.certificates" :key="i">
          <p>{{ cert.subject }}</p>
          <p>Aussteller: {{ cert.issuer }}</p>
          <p>Gültig bis {{ cert.notAfter }}</p>
          <p class="mono">SHA-256: {{ cert.sha256 }}</p>
        </div>
      </details>
    </article>
  </details>
</template>
