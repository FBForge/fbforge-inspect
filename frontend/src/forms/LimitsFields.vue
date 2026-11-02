<script setup lang="ts">
import type { DiagnosisDraft } from '../session';
defineProps<{ form: DiagnosisDraft; apiVersion: string }>();
</script>
<template>
  <p v-if="apiVersion !== 'v3'" class="notice">
    Die Limits-Abfrage benötigt v3. Bitte oben v3 auswählen.
  </p>
  <p class="hint">
    Öffentliche Größenlimits für dieses Paar. Eine Prozessmodell- oder Nachrichten-ID ist nicht
    erforderlich.
  </p>
  <label
    >Absender-Destination<input v-model.trim="form.fromDestinationId" required placeholder="UUID"
  /></label>
  <label
    >Empfänger-Destination<input v-model.trim="form.toDestinationId" required placeholder="UUID"
  /></label>
</template>
