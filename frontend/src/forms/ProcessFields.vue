<script setup lang="ts">
import SchemaExpectationFields from './SchemaExpectationFields.vue';
import type { DiagnosisDraft } from '../session';
defineProps<{ form: DiagnosisDraft; apiVersion: string }>();
</script>
<template>
  <div v-if="apiVersion !== 'v3'" class="notice">
    Prozesssuche ist eine Funktion von v3. Bitte oben v3 auswählen.
  </div>
  <label
    >Prozessmodell-ID<input
      v-model.trim="form.processModelId"
      required
      placeholder="URI oder bpmn:…"
      maxlength="512"
  /></label>
  <div class="form-grid">
    <label>Nachrichten-ID<input v-model.trim="form.messageId" required maxlength="255" /></label
    ><label
      >Region<input v-model.trim="form.region" required pattern="DE[0-9]{0,12}" placeholder="DE…"
    /></label>
  </div>
  <details>
    <summary>Erwartetes Fachdatenschema</summary>
    <SchemaExpectationFields :form="form" />
  </details>
</template>
