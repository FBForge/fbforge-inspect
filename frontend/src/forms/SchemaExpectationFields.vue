<script setup lang="ts">
import type { DiagnosisDraft } from '../session';
defineProps<{ form: DiagnosisDraft }>();
</script>
<template>
  <label>Schema-URI<input v-model.trim="form.schema" placeholder="Optional" /></label>
  <label
    >MIME-Type<select v-model="form.mime">
      <option value="">Nicht vorgeben</option>
      <option>application/json</option>
      <option>application/xml</option>
    </select></label
  >
</template>
