<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Report } from './contracts';
import { reportForExport, reportHtml } from './reports';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ close: []; saved: [] }>();
const exportDialog = ref<HTMLDialogElement>();
const raw = ref(false),
  format = ref('html'),
  exportError = ref(''),
  saving = ref(false);
const exportReport = computed(() => reportForExport(props.report, raw.value));
const previewHtml = computed(() => (format.value === 'html' ? reportHtml(exportReport.value) : ''));
const previewText = computed(() =>
  format.value === 'json' ? JSON.stringify(exportReport.value, null, 2) : '',
);
onMounted(async () => {
  await nextTick();
  exportDialog.value?.showModal();
});
function close() {
  if (!saving.value) emit('close');
}
function prepareHtmlPreview(event: Event) {
  // The report keeps scripts, forms and top-level navigation disabled. Same-origin
  // access lets the parent handle Escape consistently when focus is in the preview.
  const frame = event.target as HTMLIFrameElement;
  frame.contentDocument?.addEventListener('keydown', (key) => {
    if (key.key !== 'Escape') return;
    key.preventDefault();
    key.stopPropagation();
    if (!saving.value) emit('close');
  });
}
function trapExportFocus(event: KeyboardEvent) {
  if (event.key !== 'Tab' || event.altKey || event.ctrlKey || event.metaKey) return;
  const dialog = exportDialog.value;
  if (!dialog) return;
  const targets = [
    ...dialog.querySelectorAll<HTMLElement>(
      'button, input, select, textarea, iframe, a[href], [tabindex]',
    ),
  ].filter(
    (element) =>
      element.tabIndex >= 0 && !element.matches(':disabled') && element.getClientRects().length > 0,
  );
  const first = targets[0];
  const last = targets.at(-1);
  if (!first || !last) return;
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}
async function save() {
  if (!exportReport.value || saving.value) return;
  saving.value = true;
  exportError.value = '';
  try {
    const content = format.value === 'html' ? reportHtml(exportReport.value) : previewText.value;
    const saved = await invoke<boolean>('save_report', { content, format: format.value });
    if (saved) {
      emit('close');
      emit('saved');
    }
  } catch {
    exportError.value = 'Bericht konnte nicht gespeichert werden.';
  } finally {
    saving.value = false;
  }
}
</script>
<template>
  <dialog
    ref="exportDialog"
    class="export-dialog"
    aria-labelledby="export-title"
    :aria-busy="saving ? 'true' : undefined"
    @keydown="trapExportFocus"
    @cancel.prevent="close"
  >
    <div class="section-heading">
      <h2 id="export-title">Bericht vor dem Speichern prüfen</h2>
      <button class="icon-button" aria-label="Schließen" :disabled="saving" @click="close">
        ×
      </button>
    </div>
    <p>
      Dieser Umfang wird als lokale Datei gespeichert. Prüfen Sie die Angaben vor dem Weitergeben.
    </p>
    <div class="form-grid">
      <label
        >Format<select v-model="format" :disabled="saving">
          <option value="html">Eigenständiges HTML</option>
          <option value="json">JSON</option>
        </select></label
      ><label class="check"
        ><input v-model="raw" type="checkbox" :disabled="saving" />Rohantworten einschließen</label
      >
    </div>
    <p v-if="exportError" class="notice error-notice" role="alert">{{ exportError }}</p>
    <iframe
      v-if="format === 'html'"
      class="export-html-preview"
      title="HTML-Berichtsvorschau"
      sandbox="allow-same-origin"
      @load="prepareHtmlPreview"
      referrerpolicy="no-referrer"
      :srcdoc="previewHtml"
    ></iframe>
    <pre v-else tabindex="0" class="export-preview">{{ previewText }}</pre>
    <div class="form-footer">
      <button class="button secondary" :disabled="saving" @click="close">Abbrechen</button
      ><button class="button primary" :disabled="saving" @click="save">Speicherort wählen</button>
    </div>
  </dialog>
</template>
