<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';
import { routingRows } from '../resultValues';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ destination: [id: string]; area: [id: string] }>();
const rows = computed(() => routingRows(props.report.data));
</script>
<template>
  <div v-if="rows.length" class="panel">
    <h3>
      {{ report.operation === 'areas' ? 'Gefundene Gebiete' : 'Gefundene Kontexte' }}
      <span class="muted">({{ rows.length }})</span>
    </h3>
    <div class="table-scroll" tabindex="0" role="region" aria-label="Routing- und Suchergebnisse">
      <table>
        <thead>
          <tr>
            <th>Kontext</th>
            <th>ID</th>
            <th>Quelle / Status</th>
            <th>Aktion</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, index) in rows" :key="index">
            <td>
              {{
                row.destinationName ||
                row.destinationShortName ||
                row.name ||
                `Kontext ${index + 1}`
              }}
            </td>
            <td class="mono">{{ row.destinationId || row.id }}</td>
            <td>{{ row.origin || row.status || row.type || 'Öffentliche API' }}</td>
            <td>
              <button
                v-if="row.destinationId"
                class="text-button"
                @click="emit('destination', row.destinationId)"
              >
                Destination prüfen</button
              ><button v-else-if="row.id" class="text-button" @click="emit('area', row.id)">
                Gebiet übernehmen
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
