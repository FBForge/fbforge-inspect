<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Report } from './contracts';
import { certificateInventory, renewalCalendar } from './inventory';
import { saveFile } from './saveFile';
const props = defineProps<{ reports: Report[] }>();
const emit = defineEmits<{ destination: [id: string, environment: string, apiVersion: string] }>();
const beforeDays = ref(60),
  message = ref('');
const rows = computed(() => certificateInventory(props.reports));
async function calendar() {
  try {
    if (await saveFile(renewalCalendar(rows.value, beforeDays.value), 'ics'))
      message.value = 'Erneuerungstermine gespeichert.';
  } catch {
    message.value = 'Kalender konnte nicht gespeichert werden.';
  }
}
</script>
<template>
  <section class="panel analysis-context">
    <h2>Zertifikate und betroffene Zustellpunkte</h2>
    <p>
      Teilnehmerzertifikate aus den Einzelprüfungen, Sammelläufen und geöffneten Serverergebnissen
      dieser Sitzung. Identische Zertifikate werden anhand ihres SHA-256-Fingerabdrucks
      zusammengefasst.
    </p>
    <p class="hint">
      Die Ansicht umfasst nur die tatsächlich untersuchten Schlüssel. Unveröffentlichte oder nicht
      abgerufene Schlüssel und private Schlüsselbestände sind damit nicht nachgewiesen.
    </p>
    <div v-if="rows.length" class="action-row">
      <label
        >Erinnerung vor Ablauf<select v-model.number="beforeDays">
          <option :value="30">30 Tage</option>
          <option :value="60">60 Tage</option>
          <option :value="90">90 Tage</option>
        </select></label
      >
      <button class="button secondary" @click="calendar">Erneuerungstermine exportieren</button>
    </div>
    <p v-else>
      Prüfen Sie zunächst eine Destination, öffentliche Schlüssel oder eine Zertifikatsdatei. Auch
      Serverergebnisse lassen sich unter Serverdiagnose öffnen.
    </p>
    <p v-if="message" role="status">{{ message }}</p>
  </section>
  <section v-for="row in rows" :key="row.fingerprint" class="panel analysis-context">
    <h3>{{ row.subject }}</h3>
    <p>
      <strong>{{
        row.validity === 'expired'
          ? 'Abgelaufen'
          : row.validity === 'future'
            ? 'Noch nicht gültig'
            : `${row.days} Tage bis Ablauf`
      }}</strong>
      · {{ new Date(row.notAfter).toLocaleDateString('de-DE') }}
    </p>
    <p>Aussteller: {{ row.issuer }}</p>
    <p class="mono">SHA-256: {{ row.fingerprint }}</p>
    <ul>
      <li v-for="(use, index) in row.uses" :key="index">
        <span
          >{{ use.environment }} · {{ use.apiVersion }} · {{ use.destination }} ·
          {{ use.kid || 'Keine Schlüsselkennung' }}</span
        >
        <p>
          {{
            use.active
              ? 'Als aktiver Verschlüsselungsschlüssel veröffentlicht'
              : 'Verwendung als aktiver Verschlüsselungsschlüssel nicht nachgewiesen'
          }}
        </p>
        <button
          v-if="/^[a-f0-9-]{36}$/i.test(use.destination)"
          class="button secondary"
          @click="emit('destination', use.destination, use.environment, use.apiVersion)"
        >
          Zustellpunkt vorbereiten
        </button>
      </li>
    </ul>
    <details>
      <summary>Schlüsselwechsel vorbereiten</summary>
      <ol>
        <li>Neues geeignetes Zertifikat und öffentlichen Schlüssel bereitstellen.</li>
        <li>Gültigkeit und Veröffentlichung des neuen Schlüssels prüfen.</li>
        <li>Aktive Schlüsselkennung an allen betroffenen Zustellpunkten kontrollieren.</li>
        <li>
          Im empfangenden System sicherstellen, dass erforderliche bisherige private Schlüssel für
          laufende Nachrichten erhalten bleiben.
        </li>
      </ol>
      <p class="hint">
        Inspect prüft die öffentliche Konfiguration. Ob private Schlüssel vorhanden sind und
        Nachrichten entschlüsselt werden können, muss im empfangenden System geprüft werden.
      </p>
    </details>
  </section>
</template>
