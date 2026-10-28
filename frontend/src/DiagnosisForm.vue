<script setup lang="ts">
import { reactive, watch, ref } from 'vue';
import type { Page } from './contracts';
import { containsPrivateMaterial } from './inputs';
const props = defineProps<{ page: Page; busy: boolean; apiVersion: string }>();
const emit = defineEmits<{
  run: [operation: string, input: Record<string, unknown>];
  changed: [];
}>();
const form = reactive({
  destinationId: '',
  service: '',
  region: '',
  regionKind: 'ars',
  schema: '',
  mime: '',
  metadata: '',
  compare: false,
  revocation: false,
  trustPem: '',
  processModelId: '',
  messageId: '',
  fromDestinationId: '',
  toDestinationId: '',
  text: '',
  url: '',
  areaSearch: '',
  pvog: false,
  lbId: '',
});
const fileError = ref('');
const reading = ref(false);
watch(form, () => emit('changed'));
function submit(op: string = props.page) {
  if (props.busy || reading.value) return;
  fileError.value = '';
  emit('run', op, { ...form });
}
async function file(event: Event, trust = false) {
  const element = event.target as HTMLInputElement,
    selected = element.files?.[0];
  if (!selected) return;
  if (selected.size > 1500000) {
    fileError.value = 'Datei überschreitet 1,5 MB.';
    element.value = '';
    return;
  }
  reading.value = true;
  try {
    const text = await selected.text();
    element.value = '';
    if (containsPrivateMaterial(text, props.page === 'keys' || trust)) {
      fileError.value = 'Private oder symmetrische Schlüssel sind nicht zulässig. Datei verworfen.';
      return;
    }
    if (trust) form.trustPem = text;
    else {
      form.text = text;
      form.url = '';
    }
    fileError.value = '';
  } catch {
    fileError.value = 'Datei konnte nicht gelesen werden.';
  } finally {
    reading.value = false;
  }
}
defineExpose({
  setDestination: (id: string) => {
    form.destinationId = id;
  },
  setSchema: (url: string) => {
    form.url = url;
    form.text = '';
  },
  setArea: (id: string) => {
    form.regionKind = 'areaId';
    form.region = id;
  },
});
</script>
<template>
  <form class="panel input-panel" @submit.prevent="submit()">
    <div class="section-heading">
      <h2>Prüfung vorbereiten</h2>
      <span class="muted">{{
        page === 'process'
          ? 'Submission API v3'
          : page === 'routing'
            ? 'Routing API v2'
            : 'Öffentliche Daten'
      }}</span>
    </div>
    <fieldset :disabled="busy || reading">
      <template v-if="page === 'destination'">
        <label
          >Destination-ID<input
            v-model.trim="form.destinationId"
            required
            placeholder="UUID des Zustellpunkts"
            spellcheck="false"
        /></label>
        <details>
          <summary>Erwartete Konfiguration vergleichen</summary>
          <div class="form-grid">
            <label
              >Leistung / LeiKa<input v-model.trim="form.service" placeholder="Optional" /></label
            ><label>Region<input v-model.trim="form.region" placeholder="DE…" /></label>
            <label>Schema-URI<input v-model.trim="form.schema" placeholder="Optional" /></label
            ><label
              >MIME-Type<select v-model="form.mime">
                <option value="">Nicht vorgeben</option>
                <option>application/json</option>
                <option>application/xml</option>
              </select></label
            >
            <label
              >Metadatenversion<input v-model.trim="form.metadata" placeholder="z. B. 1.2.0"
            /></label>
          </div>
          <p class="hint">Leistung, Region und Schema werden im selben Leistungseintrag geprüft.</p>
        </details>
        <label class="check"
          ><input v-model="form.compare" type="checkbox" />V2 und V3 gegenüberstellen</label
        >
      </template>
      <template v-if="page === 'routing'">
        <div class="form-grid">
          <label
            >LeiKa-Schlüssel<input
              v-model.trim="form.service"
              required
              pattern="[0-9]{14}"
              placeholder="14 Ziffern" /></label
          ><label
            >Gebietsart<select v-model="form.regionKind">
              <option value="ars">ARS · 12 Ziffern</option>
              <option value="ags">AGS · 8 Ziffern</option>
              <option value="areaId">Gebiets-ID</option>
            </select></label
          ><label
            >Gebietsschlüssel<input
              v-model.trim="form.region"
              required
              placeholder="ARS, AGS oder ausgewählte ID"
          /></label>
        </div>
        <details>
          <summary>Gebiet über Ortsnamen finden</summary>
          <label
            >Ortsname<input v-model.trim="form.areaSearch" placeholder="z. B. Musterstadt" /></label
          ><button
            class="button secondary"
            type="button"
            :disabled="form.areaSearch.length < 2"
            @click="submit('areas')"
          >
            Gebiete suchen
          </button>
        </details>
        <label class="check"
          ><input v-model="form.pvog" type="checkbox" />PVOG-Adressierung mit abrufen</label
        >
        <label v-if="form.pvog"
          >PVOG-Leistungsbeschreibungs-ID<input
            v-model.trim="form.lbId"
            required
            placeholder="ID der Leistungsbeschreibung"
        /></label>
        <p class="hint">
          Alle Antwortkontexte bleiben erhalten. Die Diagnose zeigt öffentlich beobachtbare Daten
          und ihre Grenzen.
        </p>
      </template>
      <template v-if="page === 'process'">
        <div v-if="apiVersion !== 'v3'" class="notice">
          Prozesssuche ist eine Funktion von v3. Bitte oben v3 auswählen.
        </div>
        <label
          >Prozessmodell-ID<input
            v-model.trim="form.processModelId"
            required
            placeholder="URI oder bpmn:…"
            maxlength="512"
        /></label>
        <div class="form-grid">
          <label
            >Nachrichten-ID<input v-model.trim="form.messageId" required maxlength="255" /></label
          ><label
            >Region<input
              v-model.trim="form.region"
              required
              pattern="DE[0-9]{0,12}"
              placeholder="DE…"
          /></label>
        </div>
        <details>
          <summary>Erwartetes Fachdatenschema</summary>
          <label>Schema-URI<input v-model.trim="form.schema" placeholder="Optional" /></label
          ><label
            >MIME-Type<select v-model="form.mime">
              <option value="">Nicht vorgeben</option>
              <option>application/json</option>
              <option>application/xml</option>
            </select></label
          >
        </details>
        <details>
          <summary>Limits zwischen zwei Zustellpunkten</summary>
          <label
            >Absender-Destination<input
              v-model.trim="form.fromDestinationId"
              placeholder="UUID" /></label
          ><label
            >Empfänger-Destination<input v-model.trim="form.toDestinationId" placeholder="UUID"
          /></label>
        </details>
      </template>
      <template v-if="page === 'keys' || page === 'schema'">
        <label
          >Öffentliche HTTPS-URL<input
            v-model.trim="form.url"
            type="url"
            placeholder="https://…"
            :disabled="!!form.text"
        /></label>
        <div class="or-divider">oder lokal einlesen</div>
        <label
          >{{ page === 'keys' ? 'Öffentliches JWK / JWKS' : 'JSON Schema / XSD'
          }}<textarea
            v-model="form.text"
            rows="8"
            spellcheck="false"
            :disabled="!!form.url"
            :placeholder="
              page === 'keys'
                ? 'Nur öffentliche Schlüssel. Keine privaten Schlüssel oder Zugangsdaten.'
                : 'Schema einfügen. Keine Fachdaten erforderlich.'
            "
          />
        </label>
        <label class="file-label"
          >Datei auswählen<input
            type="file"
            :accept="page === 'keys' ? '.json,.jwk,.jwks' : '.json,.xsd,.xml'"
            @change="file($event)"
        /></label>
        <p class="hint" v-if="page === 'schema'">
          Strukturansicht ohne Instanzvalidierung. Referenzen werden angezeigt und nicht automatisch
          geladen.
        </p>
      </template>
      <details v-if="page === 'keys' || page === 'destination'" class="certificate-options">
        <summary>Zertifikate und Sperrstatus</summary>
        <label class="check"
          ><input v-model="form.revocation" type="checkbox" />Öffentliche OCSP-/CRL-Endpunkte
          abrufen</label
        >
        <p class="hint">
          Diese Auswahl erlaubt zusätzlich HTTP und OCSP-POST an öffentliche Zertifikatsendpunkte.
        </p>
        <label
          >Unabhängig gewählte öffentliche CA-Zertifikate (PEM)<textarea
            v-model="form.trustPem"
            rows="3"
            placeholder="-----BEGIN CERTIFICATE-----"
            spellcheck="false"
          />
        </label>
        <label class="file-label"
          >CA-Datei laden<input type="file" accept=".pem,.crt,.cer" @change="file($event, true)"
        /></label>
        <p class="hint">
          Die x5c-Wurzel wird nicht automatisch vertraut. Zugehörigkeit der geladenen CA zur V-PKI
          separat prüfen.
        </p>
      </details>
    </fieldset>
    <p v-if="fileError" class="error-text" role="alert">{{ fileError }}</p>
    <div class="form-footer">
      <span class="hint">{{
        page === 'keys' || page === 'schema'
          ? 'Lokale Eingabe oder öffentlicher Abruf'
          : 'Keine Anmeldung erforderlich'
      }}</span
      ><button
        class="button primary"
        :disabled="
          busy ||
          reading ||
          (page === 'process' && apiVersion !== 'v3') ||
          ((page === 'keys' || page === 'schema') && !form.text && !form.url)
        "
        type="submit"
      >
        {{ busy ? 'Prüfung läuft …' : 'Prüfung starten' }}<span aria-hidden="true">→</span>
      </button>
    </div>
  </form>
</template>
