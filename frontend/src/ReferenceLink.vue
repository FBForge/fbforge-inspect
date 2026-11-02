<script setup lang="ts">
import { ref } from 'vue';
import { isTauri, invoke } from '@tauri-apps/api/core';
const props = defineProps<{ reference: 'repository' | 'fitko' | 'pvog' }>();
const urls = {
  repository: 'https://github.com/FBForge/fbforge-inspect',
  fitko: 'https://docs.fitko.de/fit-connect/',
  pvog: 'https://pvog.fitko.net/',
};
const error = ref('');
async function open(event: MouseEvent) {
  if (!isTauri()) return;
  event.preventDefault();
  error.value = '';
  try {
    await invoke('open_reference', { reference: props.reference });
  } catch {
    error.value = 'Browser konnte nicht geöffnet werden. Die Adresse kann kopiert werden.';
  }
}
</script>
<template>
  <a :href="urls[reference]" target="_blank" rel="noopener noreferrer" @click="open"><slot /></a
  ><span v-if="error" role="alert" class="hint">{{ error }}</span>
</template>
