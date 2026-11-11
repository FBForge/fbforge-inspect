<script setup lang="ts">
import { ref } from 'vue';
defineProps<{ label: string; accept?: string; disabled?: boolean }>();
const emit = defineEmits<{ change: [event: Event] }>();
const input = ref<HTMLInputElement | null>(null);
</script>
<template>
  <button class="button secondary" type="button" :disabled="disabled" @click="input?.click()">
    {{ label }}
  </button>
  <input
    ref="input"
    hidden
    type="file"
    :aria-label="label"
    :accept="accept"
    :disabled="disabled"
    @change="emit('change', $event)"
  />
</template>
