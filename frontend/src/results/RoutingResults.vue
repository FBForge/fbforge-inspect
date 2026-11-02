<script setup lang="ts">
import { computed } from 'vue';
import { labels, type Report, type Status } from '../contracts';
import { records, routingRows } from '../resultValues';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ destination: [id: string]; area: [id: string] }>();
interface Comparison {
  destinationId?: string;
  routingPath?: string;
  status?: Status;
  detail?: string;
  pvogContexts?: string[];
  routing?: Record<string, unknown>;
}
const comparisons = computed(() => records<Comparison>(props.report.data.routingComparison));
const rows = computed(() => routingRows(props.report.data));
</script>
<template>
  <section v-if="report.operation === 'routing'" class="panel">
    <h3>Routing und PVOG im Vergleich</h3>
    <p>
      {{ rows.length }} Routingkontexte ·
      {{ Array.isArray(report.data.pvogRelations) ? report.data.pvogRelations.length : 0 }}
      PVOG-Zuständigkeitskontexte
    </p>
    <p v-if="report.data.pvogSelectedLbId" class="hint">
      Leistungsbeschreibung{{
        report.data.pvogAutomaticallySelected ? ' automatisch ausgewählt' : ''
      }}: <span class="mono">{{ report.data.pvogSelectedLbId }}</span>
    </p>
    <p v-if="!rows.length">
      Keine Routen in dieser Antwort gefunden. Abruffehler und unvollständige Quellen werden in den
      Befunden angezeigt.
    </p>
    <div v-for="(item, index) in comparisons" :key="index" class="service-group">
      <button class="text-button mono" @click="emit('destination', item.destinationId || '')">
        {{ item.destinationId || 'Ohne Destination-ID' }}
      </button>
      <p>
        <span class="badge" :class="item.status">{{
          item.status ? labels[item.status] : 'Nicht verglichen'
        }}</span>
        {{ item.detail }}
      </p>
      <details>
        <summary>Quellkontexte anzeigen</summary>
        <p class="mono">{{ item.routingPath || 'Keine Routingzeile' }}</p>
        <p v-for="context in item.pvogContexts" :key="context" class="mono">{{ context }}</p>
        <pre v-if="item.routing" tabindex="0">{{ JSON.stringify(item.routing, null, 2) }}</pre>
      </details>
    </div>
    <p class="hint">
      Verglichen werden die signierten Destination-IDs für LeiKa und ARS. Unterschiedliche
      OE-/Leistungskontexte bleiben erhalten. Namen, Anschriften und Onlinedienstparameter werden
      hier nicht als vollständig feldgleich bewertet. Quellenabweichungen erklären keine interne
      Routingentscheidung.
    </p>
  </section>
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
