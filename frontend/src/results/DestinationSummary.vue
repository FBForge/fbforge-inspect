<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';

import { list } from '../resultValues';
const props = defineProps<{ report: Report }>();

const destination = computed(() => props.report.data.destination);
</script>
<template>
  <div v-if="destination" class="panel destination-summary">
    <span class="eyebrow">Öffentlicher Zustellpunkt</span>
    <h3 class="mono">{{ destination.destinationId }}</h3>
    <div class="inline-facts">
      <span
        >Status <strong>{{ destination.status }}</strong></span
      ><span
        >Typ <strong>{{ destination.type || 'in v2 nicht veröffentlicht' }}</strong></span
      ><span
        >Metadaten <strong>{{ list(destination.metadataVersions) || 'keine Angabe' }}</strong></span
      >
    </div>
  </div>
</template>
