<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import type { Report } from '../contracts';
import type { ServiceOffer, PublicService, SubmissionSchema, ProcessMessage } from '../reportData';
import { records, list, canOpen } from '../resultValues';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ schema: [id: string] }>();
const destination = computed(() => props.report.data.destination);
const schemaSearch = ref('');
const serviceLimit = ref(10);
watch(
  () => props.report,
  () => {
    schemaSearch.value = '';
    serviceLimit.value = 10;
  },
);
const showAllSchemas = ref<string[]>([]);
watch(schemaSearch, () => {
  showAllSchemas.value = [];
});
const visibleServices = computed(() =>
  services.value
    .filter(
      (s) =>
        !schemaSearch.value ||
        `${s.identifier} ${s.schemas.map((x) => x.schemaUri).join(' ')}`
          .toLowerCase()
          .includes(schemaSearch.value.toLowerCase()),
    )
    .map((s) => ({
      ...s,
      totalSchemas: s.schemas.length,
      schemas:
        !schemaSearch.value ||
        String(s.identifier ?? '')
          .toLowerCase()
          .includes(schemaSearch.value.toLowerCase()) ||
        showAllSchemas.value.includes(s.serviceKey)
          ? s.schemas
          : s.schemas.filter((schema) =>
              String(schema.schemaUri).toLowerCase().includes(schemaSearch.value.toLowerCase()),
            ),
    }))
    .slice(0, serviceLimit.value),
);
const services = computed<ServiceOffer[]>(() => [
  ...records<PublicService>(destination.value?.publicServices).map((s, index) => ({
    serviceKey: `service-${index}`,
    ...s,
    kind: 'Leistung',
    schemas: records<SubmissionSchema>(s.submissionSchemas),
  })),
  ...records<ProcessMessage>(destination.value?.processMessages).map((s, index) => ({
    serviceKey: `process-${index}`,
    ...s,
    kind: 'Prozessnachricht',
    identifier: s.processModelId + ' / ' + s.messageId,
    schemas: records<SubmissionSchema>(s.schemas),
  })),
]);
</script>
<template>
  <section v-if="services.length" class="panel">
    <h3>
      Leistungen und Prozessnachrichten <span class="muted">({{ services.length }})</span>
    </h3>
    <label class="schema-filter"
      >Leistung oder Schema suchen<input v-model="schemaSearch" type="search"
    /></label>
    <details v-for="s in visibleServices" :key="s.serviceKey" class="service">
      <summary>
        <span>{{ s.kind }}</span
        ><span class="mono">{{ s.identifier }}</span
        ><span class="badge neutral"
          >{{ s.schemas.length }} {{ s.schemas.length === 1 ? 'Schema' : 'Schemas'
          }}<template v-if="s.schemas.length < s.totalSchemas">
            von {{ s.totalSchemas }}</template
          ></span
        >
      </summary>
      <button
        v-if="s.schemas.length < s.totalSchemas"
        class="text-button"
        @click="showAllSchemas.push(s.serviceKey)"
      >
        Alle {{ s.totalSchemas }} Schemas anzeigen
      </button>
      <p class="muted">Regionen: {{ list(s.regions) || 'keine Angabe' }}</p>
      <div
        class="service-schemas"
        tabindex="0"
        role="region"
        :aria-label="`Schemas für ${s.identifier}`"
      >
        <div v-for="(schema, j) in s.schemas" :key="j" class="schema-line">
          <span class="mono">{{ schema.schemaUri }}</span
          ><span class="badge neutral">{{ schema.mimeType }}</span>
          <button
            v-if="canOpen(schema.schemaUri)"
            class="text-button"
            @click="emit('schema', schema.schemaUri)"
          >
            Schema ansehen
          </button>
        </div>
      </div>
    </details>
    <button
      v-if="visibleServices.length === serviceLimit && services.length > serviceLimit"
      class="button secondary"
      @click="serviceLimit += 10"
    >
      Weitere Leistungen anzeigen
    </button>
    <p v-if="!visibleServices.length" class="hint">Keine passende Leistung oder Schemareferenz.</p>
  </section>
</template>
