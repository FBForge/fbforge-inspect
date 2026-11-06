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
import NetworkResults from './results/NetworkResults.vue';
import AnalysisResults from './results/AnalysisResults.vue';
import SupportPanel from './SupportPanel.vue';
import type { PasteContext } from './portable';
defineProps<{ report: Report }>();
const emit = defineEmits<{
  destination: [id: string];
  schema: [url: string];
  area: [id: string];
  pvog: [id: string];
  keys: [text: string];
  reverse: [id: string];
  context: [context: PasteContext];
  'follow-up': [operation: string, input: Record<string, unknown>];
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
    <RoutingResults
      :report="report"
      @destination="emit('destination', $event)"
      @area="emit('area', $event)"
    />
    <Findings :report="report" />
    <NetworkResults :report="report" @retry="emit('follow-up', 'network', $event)" />
    <AnalysisResults
      :report="report"
      @context="emit('context', $event)"
      @reverse="emit('reverse', $event)"
      @keys="emit('keys', $event)"
    />
    <PvogCandidates :report="report" @pvog="emit('pvog', $event)" />
    <DestinationSummary :report="report" />
    <LimitsResults :report="report" />
    <PvogResults :report="report" @destination="emit('destination', $event)" />
    <KeyResults :report="report" />
    <ServiceResults :report="report" @schema="emit('schema', $event)" />
    <SchemaResults :report="report" @schema="emit('schema', $event)" />
    <details class="panel">
      <summary>Normalisierte Daten und Vergleich</summary>
      <pre tabindex="0">{{ JSON.stringify(report.data, null, 2) }}</pre>
    </details>
    <RequestLog :report="report" />
    <SupportPanel :report="report" />
  </section>
</template>
