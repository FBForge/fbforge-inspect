<script setup lang="ts">
import { computed, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import topics from '../../docs/help.json';
const search = ref(''),
  message = ref('');
const visible = computed(() =>
  topics.filter((topic) =>
    `${topic.title} ${topic.paragraphs.join(' ')}`
      .toLocaleLowerCase('de')
      .includes(search.value.toLocaleLowerCase('de')),
  ),
);
async function open(reference: 'help' | 'status') {
  try {
    if (isTauri()) await invoke('open_reference', { reference });
    else
      window.open(
        reference === 'help'
          ? 'https://fbforge.de/inspect/hilfe/0.2/'
          : 'https://uptime.fit-connect.dev/status/fit-connect',
        '_blank',
        'noopener,noreferrer',
      );
  } catch {
    message.value = 'Die öffentliche Website konnte nicht geöffnet werden.';
  }
}
</script>
<template>
  <section class="panel">
    <h2>Anleitung auf diesem Gerät</h2>
    <p>
      Diese Erklärungen sind ohne Internet verfügbar. Öffentliche Live-Prüfungen benötigen eine
      Verbindung.
    </p>
    <label>Hilfe durchsuchen<input v-model="search" type="search" /></label>
    <div class="action-row">
      <button class="button secondary" @click="open('help')">Bebilderte Anleitung im Browser</button
      ><button class="button secondary" @click="open('status')">
        Offiziellen FIT-Connect-Betriebsstatus öffnen
      </button>
    </div>
    <p v-if="message" role="alert">{{ message }}</p>
  </section>
  <details v-for="topic in visible" :key="topic.id" class="panel">
    <summary>{{ topic.title }}</summary>
    <p v-for="(paragraph, i) in topic.paragraphs" :key="i">{{ paragraph }}</p>
    <ol v-if="topic.steps?.length">
      <li v-for="(step, i) in topic.steps" :key="i">{{ step }}</li>
    </ol>
  </details>
  <p v-if="!visible.length" role="status">Keine passende Erklärung gefunden.</p>
</template>
