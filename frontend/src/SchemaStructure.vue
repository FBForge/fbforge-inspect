<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { SchemaRow, ReportData } from './reportData';
const props = defineProps<{ rows: SchemaRow[]; data: ReportData }>();
const query = ref(''),
  kind = ref('all'),
  limit = ref(100);
watch([query, kind], () => {
  limit.value = 100;
});
watch(
  () => props.rows,
  () => {
    query.value = '';
    kind.value = 'all';
    limit.value = 100;
  },
);
function name(row: SchemaRow) {
  return row.name || row.title || String(row.path).match(/\.([^.[\]]+)$/)?.[1] || row.path;
}
const filtered = computed(() =>
  props.rows.filter((row) => {
    const field =
      /\/(?:element|attribute)(?:\[|$)/.test(row.path) ||
      /\.(?:properties|patternProperties)\.[^.]+$/.test(row.path);
    return (
      (kind.value === 'all' || field) &&
      (!query.value ||
        `${name(row)} ${row.path} ${row.type || row.base || ''} ${row.description || ''}`
          .toLowerCase()
          .includes(query.value.toLowerCase()))
    );
  }),
);
</script>
<template>
  <section class="panel" aria-label="Schemastruktur">
    <h3>
      Struktur <span class="muted">{{ data.dialect || 'Dialekt nicht angegeben' }}</span>
    </h3>
    <p v-if="data.schemaVersion" class="hint">Schemaversion: {{ data.schemaVersion }}</p>
    <p v-if="data.targetNamespace" class="mono hint">{{ data.targetNamespace }}</p>
    <div class="form-grid structure-controls">
      <label
        >Struktur durchsuchen<input
          v-model="query"
          type="search"
          placeholder="Feldname, Pfad oder Typ"
      /></label>
      <label
        >Knoten anzeigen<select v-model="kind">
          <option value="all">Alle Strukturknoten</option>
          <option value="fields">Nur Felder und Attribute</option>
        </select></label
      >
    </div>
    <p class="hint" role="status">
      {{ filtered.length }} von {{ rows.length }} Strukturknoten ·
      {{ Math.min(limit, filtered.length) }} angezeigt
    </p>
    <div class="table-scroll" tabindex="0" role="region" aria-label="Strukturtreffer">
      <table>
        <thead>
          <tr>
            <th>Feld / Element</th>
            <th>Typ / Bedeutung</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(row, i) in filtered.slice(0, limit)" :key="i">
            <td>
              <strong class="mono">{{ name(row) }}</strong>
              <details>
                <summary>Vollständiger Pfad</summary>
                <code>{{ row.path }}</code>
              </details>
            </td>
            <td>
              {{
                Array.isArray(row.type)
                  ? row.type.join(', ')
                  : row.type || row.base || 'Strukturknoten'
              }}
              <p v-if="row.description" class="hint">{{ row.description }}</p>
            </td>
          </tr>
        </tbody>
      </table>
      <p v-if="!filtered.length" class="hint">Keine passenden Strukturknoten.</p>
    </div>
    <button v-if="filtered.length > limit" class="button secondary" @click="limit += 100">
      Weitere 100 Strukturknoten anzeigen
    </button>
  </section>
</template>
