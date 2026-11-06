<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from './contracts';
import { records } from './resultValues';
import { serverNetworkEvidence } from './serverEvidence';
const props = defineProps<{ report: Report; canConfigure?: boolean }>();
const emit = defineEmits<{ network: [] }>();
const names: Record<string, string> = {
  dns: 'DNS des Zielservers',
  tcp: 'TCP zum Zielserver',
  proxyDns: 'DNS des Proxys',
  proxyTcp: 'TCP zum Proxy',
  proxyTls: 'TLS zum HTTPS-Proxy',
  proxyConnect: 'Proxy-Tunnel und Anmeldung',
  tls: 'TLS zum Zielserver',
  http: 'HTTP-Antwort',
};
const failures = computed(() =>
  records(serverNetworkEvidence(props.report).data.networkTraces).flatMap((trace) => {
    const failed = records(trace.phases).find((p) => p.outcome && p.outcome !== 'confirmed');
    if (!failed) return [];
    const finding = props.report.findings.find((f) => f.source === trace.url && f.nextStep);
    return [
      {
        url: String(trace.url || ''),
        phase: names[String(failed.stage)] || 'Verbindungsaufbau',
        next:
          finding?.nextStep ||
          'Verbindungsweg unter Netzwerk prüfen und anschließend das betroffene Ziel erneut prüfen.',
      },
    ];
  }),
);
</script>
<template>
  <section v-if="failures.length" class="panel" aria-label="Fehlgeschlagene Verbindungsphasen">
    <h3>Hier wurde die Verbindung unterbrochen</h3>
    <button v-if="canConfigure" class="button secondary" @click="emit('network')">
      Netzwerkeinstellungen öffnen
    </button>
    <article v-for="(failure, i) in failures" :key="i">
      <p class="mono">{{ failure.url }}</p>
      <p>
        <strong>{{ failure.phase }}</strong> konnte nicht abgeschlossen werden.
      </p>
      <p>
        Nachfolgende Schritte dieser Verbindung sind nicht nachgewiesen. Daraus folgt keine Aussage
        über die Gültigkeit eines Teilnehmerzertifikats.
      </p>
      <p><strong>Nächster Schritt:</strong> {{ failure.next }}</p>
    </article>
  </section>
</template>
