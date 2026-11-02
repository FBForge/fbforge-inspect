<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';
import type { SchemaReference } from '../reportData';
import { records } from '../resultValues';
import SchemaStructure from '../SchemaStructure.vue';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ schema: [id: string] }>();
const structure = computed(() => props.report.data.structure || []);
const references = computed(() =>
  records<SchemaReference>(props.report.data.references).map((ref) => {
    let url = '';
    try {
      const candidate = new URL(
        String(ref.target),
        typeof props.report.data.schemaSource === 'string'
          ? props.report.data.schemaSource
          : undefined,
      );
      if (candidate.protocol === 'https:') {
        candidate.hash = '';
        url = candidate.toString();
      }
    } catch {
      /* Local references stay local. */
    }
    return { target: ref.target, path: ref.path, url };
  }),
);
</script>
<template>
  <SchemaStructure v-if="structure.length" :rows="structure" :data="report.data" />
  <div v-if="references.length" class="panel">
    <h3>Schemareferenzen</h3>
    <p class="hint">Ein weiterer Abruf erfolgt erst nach Auswahl und erneutem Start der Prüfung.</p>
    <div v-for="(reference, index) in references" :key="index" class="schema-line">
      <span class="mono">{{ reference.target }}</span
      ><button v-if="reference.url" class="text-button" @click="emit('schema', reference.url)">
        Referenz vorbereiten</button
      ><span v-else class="hint">Lokale oder nicht direkt abrufbare Referenz</span>
    </div>
  </div>
</template>
