<script setup lang="ts">
import { reactive, watch, ref } from 'vue';
import type { Page } from './contracts';
import type { DiagnosisDraft, DiagnosisView } from './session';
import { containsPrivateMaterial } from './inputs';
import DestinationFields from './forms/DestinationFields.vue';
import RoutingFields from './forms/RoutingFields.vue';
import ProcessFields from './forms/ProcessFields.vue';
import LimitsFields from './forms/LimitsFields.vue';
import SourceFields from './forms/SourceFields.vue';
import CertificateFields from './forms/CertificateFields.vue';
const props = defineProps<{
  page: Page;
  busy: boolean;
  apiVersion: string;
  draft: DiagnosisDraft;
  view: DiagnosisView;
  operation: string;
}>();
const emit = defineEmits<{
  run: [operation: string, input: Record<string, unknown>];
  changed: [];
}>();
const form = reactive(props.draft);
const fileError = ref('');
const reading = ref(false);
watch(form, () => {
  fileError.value = '';
  emit('changed');
});
function submit(op: string = props.operation) {
  if (
    op === 'routing' &&
    props.draft.pvog &&
    (props.draft.regionKind !== 'ars' || !/^(?:DE)?[0-9]{12}$/.test(props.draft.region))
  ) {
    fileError.value =
      'Für den PVOG-Vergleich ARS als Gebietsart wählen und einen 12-stelligen ARS eingeben. Eine Gebiets-ID wird nicht in einen ARS umgewandelt.';
    return;
  }
  if (props.busy || reading.value) return;
  fileError.value = '';
  const input = { ...form };
  if (props.page === 'keys' || props.page === 'schema') {
    input.url = props.view.inputMode === 'url' ? form.url : '';
    input.text =
      props.view.inputMode === 'url'
        ? ''
        : props.view.inputMode === 'file'
          ? props.view.fileText
          : form.text;
  }
  if (op === 'process') {
    input.fromDestinationId = '';
    input.toDestinationId = '';
  }
  emit('run', op, input);
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
      props.view.fileText = text;
      props.view.fileName = selected.name;
      props.view.inputMode = 'file';
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
    props.view.inputMode = 'url';
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
      <h2>{{ operation === 'limits' ? 'Limits vorbereiten' : 'Prüfung vorbereiten' }}</h2>
      <span class="muted">{{
        page === 'process'
          ? 'Submission API v3'
          : page === 'routing'
            ? 'Routing API v2'
            : 'Öffentliche Daten'
      }}</span>
    </div>
    <fieldset :disabled="busy || reading">
      <DestinationFields v-if="page === 'destination'" :form="form" :view="view" />
      <RoutingFields
        v-if="page === 'routing'"
        :form="form"
        :view="view"
        @search="submit('areas')"
      />
      <ProcessFields
        v-if="page === 'process' && operation !== 'limits'"
        :form="form"
        :api-version="apiVersion"
      />
      <LimitsFields v-if="operation === 'limits'" :form="form" :api-version="apiVersion" />
      <SourceFields
        v-if="page === 'keys' || page === 'schema'"
        :form="form"
        :view="view"
        :page="page"
        @file="file($event)"
      />
      <CertificateFields :form="form" :view="view" :page="page" @file="file($event, true)" />
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
          ((operation === 'process' || operation === 'limits') && apiVersion !== 'v3') ||
          ((page === 'keys' || page === 'schema') &&
            (view.inputMode === 'url'
              ? !form.url
              : view.inputMode === 'file'
                ? !view.fileText
                : !form.text))
        "
        type="submit"
      >
        {{
          busy ? 'Prüfung läuft …' : operation === 'limits' ? 'Limits abrufen' : 'Prüfung starten'
        }}
      </button>
    </div>
  </form>
</template>
