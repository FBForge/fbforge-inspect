<script setup lang="ts">
import type { DiagnosisDraft, DiagnosisView } from '../session';
import type { Page } from '../contracts';
defineProps<{ form: DiagnosisDraft; view: DiagnosisView; page: Page }>();
const emit = defineEmits<{ file: [event: Event] }>();
</script>
<template>
  <details
    v-if="page === 'keys' || page === 'destination'"
    class="certificate-options"
    :open="view.certificatesOpen"
    @toggle="view.certificatesOpen = ($event.target as HTMLDetailsElement).open"
  >
    <summary>
      Zertifikate und Sperrstatus<span
        v-if="form.revocation || form.trustPem"
        class="badge neutral"
        >{{ form.revocation ? 'Onlineprüfung aktiv' : 'CA geladen' }}</span
      >
    </summary>
    <label class="check"
      ><input v-model="form.revocation" type="checkbox" />Öffentliche OCSP-/CRL-Endpunkte
      abrufen</label
    >
    <p class="hint">
      Diese Auswahl erlaubt zusätzlich HTTP und OCSP-POST an öffentliche Zertifikatsendpunkte.
    </p>
    <label
      >Zusätzliche öffentliche CA-Zertifikate (optional, PEM)<textarea
        v-model="form.trustPem"
        rows="3"
        placeholder="-----BEGIN CERTIFICATE-----"
        spellcheck="false"
      />
    </label>
    <label class="file-label"
      >CA-Datei laden<input type="file" accept=".pem,.crt,.cer" @change="emit('file', $event)"
    /></label>
    <p class="hint">
      Die gültigen BSI-Wurzeln und verifizierten Zwischenzertifikate der V-PKI sind mitgeliefert.
      Eigene CA-Zertifikate ergänzen diesen Truststore.
    </p>
  </details>
</template>
