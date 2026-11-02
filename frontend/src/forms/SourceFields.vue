<script setup lang="ts">
import type { DiagnosisDraft, DiagnosisView } from '../session';
import type { Page } from '../contracts';
defineProps<{ form: DiagnosisDraft; view: DiagnosisView; page: Page }>();
const emit = defineEmits<{ file: [event: Event] }>();
</script>
<template>
  <div class="input-modes" role="group" aria-label="Eingabeart">
    <button
      v-for="mode in [
        { id: 'text', label: 'Einfügen' },
        { id: 'url', label: 'HTTPS-URL' },
        { id: 'file', label: 'Datei' },
      ]"
      :key="mode.id"
      type="button"
      class="button secondary"
      :aria-pressed="view.inputMode === mode.id"
      @click="view.inputMode = mode.id as DiagnosisView['inputMode']"
    >
      {{ mode.label }}
    </button>
  </div>
  <label v-show="view.inputMode === 'url'"
    >Öffentliche HTTPS-URL<input
      v-model.trim="form.url"
      type="url"
      :required="view.inputMode === 'url'"
      :disabled="view.inputMode !== 'url'"
      placeholder="https://…"
  /></label>
  <label v-show="view.inputMode === 'text'"
    >{{ page === 'keys' ? 'Öffentliches JWK / JWKS' : 'JSON Schema / XSD'
    }}<textarea
      v-model="form.text"
      rows="4"
      spellcheck="false"
      :disabled="view.inputMode === 'url'"
      :placeholder="
        page === 'keys'
          ? 'Nur öffentliche JWK/JWKS. Keine privaten Schlüssel oder Zugangsdaten.'
          : 'Schema einfügen. Keine Fachdaten erforderlich.'
      "
    />
  </label>
  <label v-show="view.inputMode === 'file'" class="file-label"
    >Datei auswählen<input
      type="file"
      :accept="page === 'keys' ? '.json,.jwk,.jwks' : '.json,.xsd,.xml'"
      @change="emit('file', $event)"
  /></label>
  <p v-if="view.inputMode === 'file' && view.fileText" class="hint">
    Datei eingelesen: {{ view.fileName }}. Der Inhalt ist unter Einfügen einsehbar.
  </p>
  <p v-if="page === 'schema'" class="hint">
    Syntax- und Strukturansicht, keine Instanzvalidierung. Referenzen werden nur auf ausdrücklichen
    Aufruf geladen.
  </p>
</template>
