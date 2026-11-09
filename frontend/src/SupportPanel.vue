<script setup lang="ts">
import { computed, ref } from 'vue';
import DisclosurePanel from './DisclosurePanel.vue';
import type { Report } from './contracts';
import { supportText, type SupportAudience } from './support';
import { saveFile } from './saveFile';
import type { PortableResult } from './portable';
const props = defineProps<{ report: Report; runtime?: PortableResult['runtime'] }>();
const audience = ref<SupportAudience>('it'),
  sources = ref(false),
  message = ref('');
const preview = computed(() =>
  supportText(props.report, audience.value, sources.value, props.runtime),
);
async function save() {
  try {
    if (await saveFile(preview.value, 'txt')) message.value = 'Supporttext gespeichert.';
  } catch {
    message.value = 'Supporttext konnte nicht gespeichert werden.';
  }
}
</script>
<template>
  <DisclosurePanel class="support-panel" label="Supportanfrage vorbereiten">
    <label
      >Ansprechstelle<select v-model="audience">
        <option value="it">Lokale IT</option>
        <option value="operator">Zustellpunktbetreiber</option>
        <option value="editorial">Datenpflege</option>
        <option value="fitko">FITKO-Support</option>
      </select></label
    >
    <label class="check"
      ><input v-model="sources" type="checkbox" />Quelladressen und Feldpfade aufnehmen</label
    >
    <p class="hint">
      Vorschau vor Weitergabe prüfen. Befundtexte können öffentliche Kennungen und
      Organisationsnamen enthalten. Es wird nichts automatisch versendet.
    </p>
    <pre tabindex="0">{{ preview }}</pre>
    <button class="button secondary" @click="save">Supporttext speichern</button>
    <p v-if="message" role="status">{{ message }}</p>
  </DisclosurePanel>
</template>
