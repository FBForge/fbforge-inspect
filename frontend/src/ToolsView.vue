<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { arsPattern } from './areaKeys';
import { recognizeInput } from './portable';
import { explainHttp } from './support';
import type { PasteContext } from './portable';
const props = defineProps<{ busy: boolean; environment: string }>();
const emit = defineEmits<{
  run: [operation: string, input: Record<string, unknown>];
  context: [context: PasteContext];
  changed: [];
}>();
const mode = ref('paste'),
  source = ref(''),
  destination = ref(''),
  service = ref(''),
  region = ref(''),
  certificate = ref(''),
  candidate = ref('');
const error = ref(''),
  httpCode = ref(407),
  revocation = ref(true);
const recognized = computed(() => recognizeInput(source.value));
watch([mode, source, destination, service, region, certificate, candidate, revocation], () => {
  error.value = '';
  emit('changed');
});
async function file(event: Event) {
  const element = event.target as HTMLInputElement,
    selected = element.files?.[0];
  element.value = '';
  if (!selected) return;
  try {
    if (selected.size > 1500000) throw new Error('Zertifikatsdatei überschreitet 1,5 MB.');
    const bytes = new Uint8Array(await selected.arrayBuffer());
    const text = new TextDecoder().decode(bytes);
    if (text.includes('PRIVATE KEY')) throw new Error('Private Schlüssel sind nicht zulässig.');
    if (text.includes('-----BEGIN')) certificate.value = text;
    else {
      let binary = '';
      for (const byte of bytes) binary += String.fromCharCode(byte);
      certificate.value = btoa(binary);
    }
    error.value = '';
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Zertifikat nicht lesbar.';
  }
}
function run() {
  error.value = '';
  if (
    ['reverse', 'signature'].includes(mode.value) &&
    Boolean(service.value) !== Boolean(region.value)
  ) {
    error.value = 'LeiKa-Schlüssel und ARS gemeinsam angeben oder beide Felder leer lassen.';
    return;
  }
  if (mode.value === 'certificate')
    emit('run', 'certificate', { text: certificate.value, revocation: revocation.value });
  if (mode.value === 'signature')
    emit('run', 'signature', {
      text: source.value,
      destinationId: destination.value,
      service: service.value,
      region: region.value,
    });
  if (mode.value === 'reverse')
    emit('run', 'reverse', {
      destinationId: destination.value,
      pairs:
        service.value && region.value
          ? [{ service: service.value, region: region.value, regionKind: 'ars' }]
          : [],
    });
  if (mode.value === 'rollover')
    emit('run', 'rollover', {
      destinationId: destination.value,
      candidateKid: candidate.value,
      revocation: revocation.value,
    });
}
defineExpose({
  prepareReverse: (id: string) => {
    mode.value = 'reverse';
    destination.value = id;
    service.value = '';
    region.value = '';
  },
});
</script>
<template>
  <section class="panel analysis-context">
    <h2>Gezielt analysieren</h2>
    <label
      >Aufgabe<select v-model="mode" :disabled="busy">
        <option value="paste">Kennung oder URL übernehmen</option>
        <option value="reverse">Rückwärtsrouting</option>
        <option value="certificate">Öffentliche Zertifikatsdatei</option>
        <option value="signature">Adressierungssignatur</option>
        <option value="rollover">Schlüsselwechsel vorbereiten</option>
        <option value="error">HTTP-Fehler erklären</option>
      </select></label
    >
    <template v-if="mode === 'paste'">
      <label
        >Destination-ID oder öffentliche FIT-Connect-URL<textarea
          v-model="source"
          rows="3"
          spellcheck="false"
          :disabled="busy"
        />
      </label>
      <template v-if="recognized"
        ><p>
          Erkannt: {{ recognized.operation === 'destination' ? 'Zustellpunkt' : 'Routing' }} ·
          {{ recognized.environment || environment }} ·
          {{ recognized.apiVersion || 'Gewählte API-Version' }}
        </p>
        <pre tabindex="0">{{ JSON.stringify(recognized.input, null, 2) }}</pre>
        <button class="button primary" :disabled="busy" @click="emit('context', recognized)">
          Angaben übernehmen
        </button></template
      >
      <p v-else class="hint">
        Unterstützt werden UUIDs, öffentliche Zustellpunkt-URLs und Routing-API-URLs. Einfügen
        startet keine Verbindung.
      </p>
    </template>
    <form v-else-if="mode !== 'error'" @submit.prevent="run">
      <fieldset :disabled="busy">
        <template v-if="mode === 'certificate'">
          <label
            >Öffentliches Zertifikat auswählen<input
              type="file"
              accept=".pem,.cer,.crt,.der,.p7b,.p7c"
              @change="file"
          /></label>
          <label
            >Öffentliche PEM-Kette oder Base64-DER<textarea
              v-model="certificate"
              rows="5"
              spellcheck="false"
              required
            />
          </label>
        </template>
        <template v-else>
          <label
            >Destination-ID{{ mode === 'signature' ? ' als Erwartung (optional)' : ''
            }}<input v-model.trim="destination" :required="mode !== 'signature'" spellcheck="false"
          /></label>
          <template v-if="mode === 'signature'"
            ><label
              >Öffentliche Adressierungssignatur<textarea
                v-model="source"
                rows="5"
                spellcheck="false"
                required
              />
            </label>
            <p class="hint">
              Nur öffentliche destinationSignature. Keine Zugangstoken oder privaten Schlüssel
              eingeben.
            </p></template
          >
          <template v-if="mode !== 'rollover'">
            <div class="form-grid">
              <label
                >LeiKa-Schlüssel (optional)<input
                  v-model.trim="service"
                  pattern="[0-9]{14}" /></label
              ><label
                >ARS (optional)<input
                  v-model.trim="region"
                  :pattern="arsPattern"
                  aria-describedby="tools-ars-hint"
              /></label>
            </div>
            <p id="tools-ars-hint" class="hint">
              ARS mit 2, 3, 5, 9 oder 12 Ziffern. Führende Nullen bleiben erhalten.
            </p>
            <p v-if="mode === 'reverse'" class="hint">
              Ohne Kombination werden zuerst die veröffentlichten Zuordnungen geladen. Für den
              Routingabruf eine konkrete Leistung und einen ARS angeben.
            </p>
          </template>
          <label v-else
            >Neue veröffentlichte Schlüsselkennung<input
              v-model.trim="candidate"
              required
              maxlength="255"
              spellcheck="false"
          /></label>
        </template>
        <label v-if="['certificate', 'rollover'].includes(mode)" class="check"
          ><input v-model="revocation" type="checkbox" />Öffentlichen Sperrstatus prüfen</label
        >
        <p v-if="['reverse', 'signature'].includes(mode) && environment === 'STAGE'" class="notice">
          Für diese Aufgabe TEST oder PROD auswählen. STAGE besitzt keine eigene Routing-API.
        </p>
        <button class="button primary" type="submit">
          {{ mode === 'reverse' ? 'Rückwärtsprüfung starten' : 'Prüfung starten' }}
        </button>
      </fieldset>
    </form>
    <template v-else
      ><label
        >HTTP-Status<input v-model.number="httpCode" type="number" min="100" max="599"
      /></label>
      <p role="status">{{ explainHttp(httpCode) }}</p>
      <p class="hint">
        Diese Erklärung ist offline verfügbar. Die Ursache muss anhand der betroffenen Anfrage
        eingegrenzt werden.
      </p></template
    >
    <p v-if="error" role="alert" class="error-text">{{ error }}</p>
  </section>
</template>
