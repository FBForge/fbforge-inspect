<script setup lang="ts">
import type { Finding } from './contracts';
defineProps<{ finding: Finding }>();
</script>
<template>
  <details v-if="finding.source || finding.path">
    <summary>Beleg und Feldpfad</summary>
    <p class="mono hint">{{ finding.path }}</p>
    <p class="mono hint">{{ finding.source || 'Lokale Eingabe' }}</p>
  </details>
</template>
