<script setup lang="ts">
import type { DiagnosisDraft, DiagnosisView } from '../session';
import { regionPattern } from '../areaKeys';
defineProps<{ form: DiagnosisDraft; view: DiagnosisView }>();
const emit = defineEmits<{ search: [] }>();
</script>
<template>
  <div class="form-grid">
    <label
      >LeiKa-Schlüssel<input
        v-model.trim="form.service"
        required
        pattern="[0-9]{14}"
        placeholder="14 Ziffern" /></label
    ><label
      >Gebietsart<select v-model="form.regionKind">
        <option value="ars">ARS · 2, 3, 5, 9 oder 12 Ziffern</option>
        <option value="ags">AGS · 8 Ziffern</option>
        <option value="areaId">Gebiets-ID</option>
      </select></label
    ><label
      >Gebietsschlüssel<input
        v-model.trim="form.region"
        required
        :pattern="regionPattern(form.regionKind)"
        placeholder="ARS, AGS oder ausgewählte ID"
    /></label>
  </div>
  <details
    :open="view.areaSearchOpen"
    @toggle="view.areaSearchOpen = ($event.target as HTMLDetailsElement).open"
  >
    <summary>Gebiet über Ortsnamen finden</summary>
    <label>Ortsname<input v-model.trim="form.areaSearch" placeholder="z. B. Musterstadt" /></label
    ><button
      class="button secondary"
      type="button"
      :disabled="form.areaSearch.length < 2"
      @click="emit('search')"
    >
      Gebiete suchen
    </button>
  </details>
  <label class="check"
    ><input v-model="form.pvog" type="checkbox" />PVOG-Adressierung mit abrufen</label
  >
  <p v-if="form.pvog" class="hint" id="pvog-ars-hint">
    Mit LeiKa und einem ARS mit 2, 3, 5, 9 oder 12 Ziffern sucht Inspect passende
    Leistungsbeschreibungen samt OE-Verweisen. Ein eindeutiger Treffer wird automatisch geprüft. Bei
    mehreren Treffern wählen Sie eine Leistungsbeschreibung aus. Die Ortssuche liefert eine
    Gebiets-ID für die Routingabfrage.
  </p>
  <details v-if="form.pvog">
    <summary>Bekannte Leistungsbeschreibungs-ID verwenden</summary>
    <label
      >PVOG-Leistungsbeschreibungs-ID (optional)<input
        v-model.trim="form.lbId"
        aria-label="PVOG-Leistungsbeschreibungs-ID"
        placeholder="Bekannte ID oder zunächst Vorschläge suchen"
    /></label>
  </details>
  <p class="hint">
    Alle Antwortkontexte bleiben erhalten. Die Diagnose zeigt öffentlich beobachtbare Daten und ihre
    Grenzen.
  </p>
</template>
