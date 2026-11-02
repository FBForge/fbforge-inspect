<script setup lang="ts">
// Same appearance control as FBForge Connect, adapted to Inspect's typed preference.
import { ref, useId } from 'vue';
import Icon from './Icon.vue';
defineProps<{ preference: string; dark: boolean }>();
const emit = defineEmits<{ change: [value: 'system' | 'light' | 'dark'] }>();
const id = useId();
const panel = ref<HTMLElement>();
const opener = ref<HTMLButtonElement>();
const choices = [
  { value: 'system', label: 'System' },
  { value: 'light', label: 'Hell' },
  { value: 'dark', label: 'Dunkel' },
] as const;
function choose(value: 'system' | 'light' | 'dark') {
  emit('change', value);
  panel.value?.hidePopover();
  opener.value?.focus();
}
</script>
<template>
  <button
    ref="opener"
    class="icon-button"
    type="button"
    aria-label="Darstellung wählen"
    :popovertarget="id"
  >
    <Icon :name="dark ? 'sun' : 'moon'" />
  </button>
  <div :id="id" ref="panel" popover class="appearance-picker" role="group" aria-label="Darstellung">
    <strong>Darstellung</strong>
    <button
      v-for="item in choices"
      :key="item.value"
      type="button"
      class="button secondary"
      :aria-pressed="preference === item.value"
      @click="choose(item.value)"
    >
      <span v-if="preference === item.value" aria-hidden="true">✓</span>{{ item.label }}
    </button>
  </div>
</template>
