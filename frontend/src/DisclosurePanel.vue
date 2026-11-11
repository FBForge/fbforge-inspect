<script setup lang="ts">
import { useId } from 'vue';
defineProps<{ label: string; openLabel?: string }>();
const open = defineModel<boolean>('open', { default: false });
const id = useId();
</script>
<template>
  <div class="disclosure">
    <button
      class="button secondary disclosure-button"
      type="button"
      :aria-expanded="open"
      :aria-controls="id"
      @click="open = !open"
    >
      <svg viewBox="0 0 20 20" fill="none" aria-hidden="true" :class="{ expanded: open }">
        <path d="m7 4 6 6-6 6" stroke="currentColor" stroke-width="1.7" />
      </svg>
      {{ open && openLabel ? openLabel : label }}
    </button>
    <div :id="id" :hidden="!open" class="disclosure-content"><slot v-if="open" /></div>
  </div>
</template>
