<script setup lang="ts">
import { ref } from 'vue';
import type { NetworkSettings } from './network';
const props = defineProps<{
  settings: NetworkSettings;
  certificates: string[];
  busy: boolean;
}>();
const emit = defineEmits<{ run: [input: Record<string, unknown>]; changed: [] }>();
const includeCertificates = ref(true);
function check() {
  emit('run', { certificates: includeCertificates.value ? props.certificates : [] });
}
</script>
<template>
  <form @submit.prevent="check" @change="emit('changed')">
    <section class="panel">
      <h2 id="network-route-title">Verbindungsweg</h2>
      <fieldset :disabled="busy" aria-labelledby="network-route-title">
        <label
          >Verbindung
          <select v-model="settings.mode">
            <option value="direct">Direkt</option>
            <option value="manual">Manueller Proxy</option>
          </select>
        </label>
        <template v-if="settings.mode === 'manual'">
          <label
            >Proxy-Adresse
            <input
              v-model.trim="settings.proxyUrl"
              type="url"
              required
              placeholder="https://proxy.beispiel.de:8080"
              autocomplete="off"
            />
          </label>
          <div class="form-grid">
            <label
              >Proxy-Benutzername (optional)
              <input v-model="settings.username" autocomplete="off" spellcheck="false" />
            </label>
            <label
              >Proxy-Passwort
              <input v-model="settings.password" type="password" autocomplete="off" />
            </label>
          </div>
          <p class="hint">
            Benutzername und Passwort verwenden die Basic-Anmeldung. Sie bleiben nur für diese
            Sitzung im Arbeitsspeicher und werden nicht in Berichte übernommen.
          </p>
          <p v-if="settings.username && settings.proxyUrl.startsWith('http://')" class="hint">
            Bei http:// ist die Proxy-Anmeldung auf dieser Verbindung nicht durch TLS geschützt.
          </p>
        </template>
        <p class="hint">
          Der Verbindungsweg gilt auch für Destinationen, Routing sowie OCSP und CRL.
        </p>
      </fieldset>
    </section>
    <section class="panel">
      <h2 id="network-check-title">Öffentliche Verbindungen prüfen</h2>
      <fieldset :disabled="busy" aria-labelledby="network-check-title">
        <p>
          Prüft die von Inspect verwendeten FIT-Connect- und Routing-Verbindungen der gewählten
          Umgebung.
        </p>
        <label class="check">
          <input v-model="includeCertificates" type="checkbox" :disabled="!certificates.length" />
          Zertifikatsendpunkte der letzten Prüfung dieser Umgebung mit prüfen
        </label>
        <p v-if="!certificates.length" class="hint">
          Für konkrete OCSP- und CRL-Endpunkte zuerst eine Destination oder einen öffentlichen
          Schlüssel prüfen.
        </p>
        <p v-else class="hint">
          {{ certificates.length }} öffentliche Zertifikate verfügbar. OCSP wird mit einer passenden
          POST-Anfrage geprüft. CRL per GET.
        </p>
        <button class="button primary" type="submit">Verbindungen prüfen</button>
      </fieldset>
    </section>
  </form>
</template>
