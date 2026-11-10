<script setup lang="ts">
import { pages, type Page } from './contracts';
import Icon from './Icon.vue';
const emit = defineEmits<{ navigate: [page: Page]; network: []; mapping: [] }>();
</script>
<template>
  <section class="start-task-section">
    <p class="eyebrow">IHRE AUFGABE</p>
    <h2>Was möchten Sie prüfen?</h2>
    <p class="muted">
      Wählen Sie eine Diagnose. Eingaben und Ergebnisse bleiben beim Wechsel erhalten.
    </p>
    <div class="start-grid">
      <div v-for="item in pages.slice(1, 6)" :key="item.id" class="start-card panel">
        <Icon :name="item.icon" />
        <button class="start-task-button" @click="emit('navigate', item.id)">
          <strong>{{ item.label }}</strong>
        </button>
        <p>{{ item.description }}</p>
        <span class="start-task-action" aria-hidden="true">Aufgabe öffnen</span>
      </div>
      <div class="start-card panel">
        <Icon name="route" />
        <button class="start-task-button" @click="emit('mapping')">
          <strong>Automatisch zuordnen</strong>
        </button>
        <p>Schema oder XDF zuordnen und Excel-Anforderungen als Zusatzquelle nutzen.</p>
        <span class="start-task-action" aria-hidden="true">Aufgabe öffnen</span>
      </div>
    </div>
  </section>
  <p class="hint">
    Ohne Zugangsdaten. Öffentliche Quellen. Prüfstände bleiben nur in dieser Sitzung erhalten.
  </p>
  <button class="button secondary" @click="emit('network')">
    Netzwerk und Verbindungsprobleme
  </button>
</template>
