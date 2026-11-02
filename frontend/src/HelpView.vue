<script setup lang="ts">
import { computed, nextTick, watch } from 'vue';
import ReferenceLink from './ReferenceLink.vue';
import type { DiagnosisView } from './session';
type Topic = {
  id: string;
  title: string;
  summary: string;
  paragraphs: string[];
  steps: string[];
  example: string | null;
};
const props = defineProps<{ topics: Topic[]; view: DiagnosisView }>();
const selected = computed({
  get: () => props.view.helpTopic,
  set: (value) => {
    props.view.helpTopic = value;
  },
});
watch(selected, () => {
  props.view.helpBasicsOpen = false;
  props.view.helpExampleOpen = false;
});
const topic = computed(() => props.topics[selected.value]!);
async function move(offset: number) {
  selected.value += offset;
  await nextTick();
  document.querySelector('main')?.scrollTo({ top: 0 });
}
</script>
<template>
  <label class="help-picker"
    >Hilfethema<select v-model="selected">
      <option v-for="(t, i) in topics" :key="t.id" :value="i">{{ t.title }}</option>
    </select></label
  >
  <article :key="topic.id" :id="topic.id" class="panel help-topic">
    <h2>{{ topic.title }}</h2>
    <p>{{ topic.summary }}</p>
    <ol>
      <li v-for="step in topic.steps" :key="step">{{ step }}</li>
    </ol>
    <details
      :open="view.helpBasicsOpen"
      @toggle="view.helpBasicsOpen = ($event.target as HTMLDetailsElement).open"
    >
      <summary>Grundlagen und Grenzen</summary>
      <p v-for="text in topic.paragraphs" :key="text">{{ text }}</p>
    </details>
    <details
      v-if="topic.example"
      :open="view.helpExampleOpen"
      @toggle="view.helpExampleOpen = ($event.target as HTMLDetailsElement).open"
    >
      <summary>Beispiel anzeigen</summary>
      <pre tabindex="0" class="help-example"><code>{{ topic.example }}</code></pre>
    </details>
  </article>
  <nav class="help-pages" aria-label="Hilfekapitel wechseln">
    <button class="button secondary" :disabled="selected === 0" @click="move(-1)">
      Vorheriges Thema</button
    ><span>{{ selected + 1 }} von {{ topics.length }}</span
    ><button class="button secondary" :disabled="selected === topics.length - 1" @click="move(1)">
      Nächstes Thema
    </button>
  </nav>
  <p class="hint">
    Offizielle Quellen: <ReferenceLink reference="fitko">FIT-Connect-Dokumentation</ReferenceLink> ·
    <ReferenceLink reference="pvog">PVOG</ReferenceLink>
  </p>
</template>
