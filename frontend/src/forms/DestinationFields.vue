<script setup lang="ts">
import SchemaExpectationFields from './SchemaExpectationFields.vue';
import type { DiagnosisDraft, DiagnosisView } from '../session';
defineProps<{ form: DiagnosisDraft; view: DiagnosisView }>();
</script>
<template>
  <label
    >Destination-ID<input
      v-model.trim="form.destinationId"
      required
      placeholder="UUID des Zustellpunkts"
      spellcheck="false"
  /></label>
  <details
    :open="view.expectedOpen"
    @toggle="view.expectedOpen = ($event.target as HTMLDetailsElement).open"
  >
    <summary>Erwartete Konfiguration vergleichen</summary>
    <div class="form-grid">
      <label>Leistung / LeiKa<input v-model.trim="form.service" placeholder="Optional" /></label
      ><label>Region<input v-model.trim="form.region" placeholder="DE…" /></label>
      <SchemaExpectationFields :form="form" />
      <label
        >Metadatenversion<input v-model.trim="form.metadata" placeholder="z. B. 1.2.0"
      /></label>
    </div>
    <p class="hint">Leistung, Region und Schema werden im selben Leistungseintrag geprüft.</p>
  </details>
  <label class="check"
    ><input v-model="form.compare" type="checkbox" />V2 und V3 gegenüberstellen</label
  >
</template>
