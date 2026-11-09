<script setup lang="ts">
import type { Finding } from './contracts';
import DisclosurePanel from './DisclosurePanel.vue';
defineProps<{ finding: Finding }>();
</script>
<template>
  <DisclosurePanel v-if="finding.source || finding.path" label="Beleg und Feldpfad">
    <p class="mono hint">{{ finding.path }}</p>
    <p class="mono hint">{{ finding.source || 'Lokale Eingabe' }}</p>
  </DisclosurePanel>
</template>
