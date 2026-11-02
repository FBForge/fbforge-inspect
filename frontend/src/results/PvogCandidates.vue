<script setup lang="ts">
import type { Report } from '../contracts';
import type { PvogCandidate } from '../reportData';
import { records, list } from '../resultValues';
defineProps<{ report: Report }>();
const emit = defineEmits<{ pvog: [id: string] }>();
</script>
<template>
  <section
    v-if="Array.isArray(report.data.pvogCandidates)"
    class="panel"
    aria-labelledby="pvog-candidates-title"
  >
    <h3 id="pvog-candidates-title">Leistungsbeschreibung für den PVOG-Abgleich wählen</h3>
    <p class="hint">
      Vorschläge aus der PVOG-Suche zu LeiKa und ARS. OE-Verweise zeigen den veröffentlichten
      Kontext. Erst der anschließende Abgleich prüft die Adressierung.
    </p>
    <p v-if="!report.data.pvogCandidates.length">
      Keine passenden Vorschläge verfügbar. Beachten Sie mögliche Abruffehler. Eine bekannte ID
      können Sie weiterhin im Formular eintragen.
    </p>
    <div
      v-for="candidate in records<PvogCandidate>(report.data.pvogCandidates)"
      :key="candidate.id"
      class="service-group"
    >
      <h4>{{ candidate.name || 'Leistungsbeschreibung' }}</h4>
      <p class="mono">{{ candidate.id }}</p>
      <details>
        <summary>Quelle und Organisationseinheiten</summary>
        <p class="mono hint">{{ candidate.source }}</p>
        <p>ARS: {{ list(candidate.ars) || 'Keine Angabe' }}</p>
        <p>OE: {{ list(candidate.organisationIds) || 'Keine veröffentlichten OE-Verweise' }}</p>
      </details>
      <button class="button secondary" @click="emit('pvog', candidate.id)">
        Mit dieser Leistungsbeschreibung prüfen
      </button>
    </div>
  </section>
</template>
