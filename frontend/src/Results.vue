<script setup lang="ts">
import type { Report } from './contracts';
import Findings from './Findings.vue';
import PvogCandidates from './results/PvogCandidates.vue';
import DestinationSummary from './results/DestinationSummary.vue';
import LimitsResults from './results/LimitsResults.vue';
import RoutingResults from './results/RoutingResults.vue';
import PvogResults from './results/PvogResults.vue';
import KeyResults from './results/KeyResults.vue';
import ServiceResults from './results/ServiceResults.vue';
import SchemaResults from './results/SchemaResults.vue';
import RequestLog from './results/RequestLog.vue';
defineProps<{ report: Report }>();
const emit = defineEmits<{
  destination: [id: string];
  schema: [url: string];
  area: [id: string];
  pvog: [id: string];
}>();
</script>
<template>
  <section class="results" aria-label="Diagnoseergebnis">
    <div class="section-heading">
      <h2>Diagnoseergebnis</h2>
      <span class="muted"
        >{{ report.environment }} · {{ report.apiVersion }} ·
        {{ new Date(report.createdAt).toLocaleTimeString('de-DE') }}</span
      >
    </div>
    <Findings :report="report" />
    <PvogCandidates :report="report" @pvog="emit('pvog', $event)" />
    <DestinationSummary :report="report" />
    <LimitsResults :report="report" />
    <RoutingResults
      :report="report"
      @destination="emit('destination', $event)"
      @area="emit('area', $event)"
    />
    <PvogResults :report="report" @destination="emit('destination', $event)" />
    <KeyResults :report="report" />
    <ServiceResults :report="report" @schema="emit('schema', $event)" />
    <SchemaResults :report="report" @schema="emit('schema', $event)" />
    <details class="panel">
      <summary>Normalisierte Daten und Vergleich</summary>
      <pre tabindex="0">{{ JSON.stringify(report.data, null, 2) }}</pre>
    </details>
    <RequestLog :report="report" />
  </section>
</template>
