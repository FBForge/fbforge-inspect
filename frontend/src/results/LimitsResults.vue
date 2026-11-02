<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';

import { limitValue } from '../resultValues';
const props = defineProps<{ report: Report }>();

const limits = computed(() => props.report.data.limits);
const attachmentLimits = computed(() => limits.value?.attachments);
</script>
<template>
  <section v-if="limits" class="panel" aria-label="Größenlimits">
    <h3>Größenlimits für Anlagen</h3>
    <p class="hint">
      Veröffentlichte Werte für das gewählte Absender-/Empfänger-Paar. Keine Sendeprobe.
    </p>
    <p class="mono hint">{{ report.data.fromDestinationId }} → {{ report.data.toDestinationId }}</p>
    <div
      v-if="attachmentLimits"
      class="table-scroll"
      tabindex="0"
      role="region"
      aria-label="Anlagenlimits"
    >
      <table>
        <thead>
          <tr>
            <th>Grenze</th>
            <th>Anwendbar</th>
            <th>Plattform</th>
            <th>Absender</th>
            <th>Empfänger</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="field in [
              { id: 'maxNumber', label: 'Anzahl Anlagen' },
              { id: 'maxSizeIndividualBytes', label: 'Größe je Anlage' },
              { id: 'maxSizeAggregateBytes', label: 'Gesamtgröße' },
            ]"
            :key="field.id"
          >
            <th>{{ field.label }}</th>
            <td v-for="scope in ['applicable', 'platform', 'send', 'receive']" :key="scope">
              {{ limitValue(attachmentLimits[scope]?.[field.id], field.id) }}
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </section>
</template>
