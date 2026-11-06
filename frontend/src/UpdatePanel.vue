<script setup lang="ts">
import { onMounted, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
defineProps<{ busy: boolean }>();
interface UpdateStatus {
  configured: boolean;
  supported: boolean;
  phase: string;
  version: string | null;
  notes: string;
  downloaded: number;
  total: number | null;
}
const state = ref<UpdateStatus | null>(null),
  working = ref(false),
  message = ref('');
const errors: Record<string, string> = {
  update_not_configured: 'Für diesen Build ist kein Update-Vertrauensschlüssel eingerichtet.',
  update_platform_unsupported: 'Integrierte Installation wird für Windows x64 angeboten.',
  update_network_failed:
    'Updatequelle nicht erreichbar. Der Updater verwendet den Netzwerkweg des Betriebssystems und keine FIT-Connect-Zugangsdaten.',
  update_feed_unavailable: 'Für diese Version ist noch kein Updatekatalog veröffentlicht.',
  update_manifest_invalid: 'Updateinformationen konnten nicht sicher ausgewertet werden.',
  update_signature_invalid: 'Die Updatesignatur ist ungültig. Das Paket wird nicht installiert.',
  update_activity_pending:
    'Beenden Sie laufende Prüfungen und schließen Sie Speicherdialoge vor der Installation.',
  update_cancelled: 'Aktualisierung abgebrochen.',
  update_rate_limited: 'Die Updatequelle begrenzt die Anfragen. Versuchen Sie es später erneut.',
};
onMounted(async () => {
  if (isTauri())
    try {
      state.value = await invoke<UpdateStatus>('application_update_status');
    } catch {
      /* A status failure never starts network access. */
    }
});
async function command(name: string) {
  working.value = true;
  message.value = '';
  let timer: ReturnType<typeof setInterval> | undefined;
  if (name === 'download_application_update')
    timer = setInterval(async () => {
      try {
        const progress = await invoke<UpdateStatus>('application_update_status');
        if (working.value) state.value = progress;
      } catch {
        /* Keep last progress. */
      }
    }, 500);
  try {
    state.value = await invoke<UpdateStatus>(
      name,
      state.value?.version ? { version: state.value.version } : {},
    );
  } catch (e) {
    message.value =
      errors[String(e)] || 'Aktualisierung fehlgeschlagen. Die Anwendung bleibt geöffnet.';
    try {
      state.value = await invoke<UpdateStatus>('application_update_status');
    } catch {
      /* Preserve the last known status. */
    }
  } finally {
    if (timer) clearInterval(timer);
    working.value = false;
  }
}
async function cancel() {
  try {
    await invoke('cancel_application_update');
  } catch {
    message.value = 'Downloadabbruch nicht bestätigt. Der aktuelle Vorgang wird weiter abgewartet.';
  }
}
</script>
<template>
  <section class="panel analysis-context">
    <h2>Inspect aktualisieren</h2>
    <p>
      Neue stabile Versionen werden auf ausdrücklichen Wunsch geprüft. Vor der Installation wird die
      Paketsignatur mit dem für Inspect hinterlegten Schlüssel verifiziert.
    </p>
    <p class="hint">
      Eine Installation beendet Inspect. Exportieren Sie benötigte Ergebnisse und Prüfaufträge
      vorher. Die abschließende Bestätigung erfolgt in einem eigenen Windows-Dialog.
    </p>
    <div class="action-row">
      <button
        class="button secondary"
        :disabled="busy || working || !state?.supported || !state?.configured"
        @click="command('check_application_update')"
      >
        Nach Updates suchen
      </button>
      <button
        v-if="state?.phase === 'available'"
        class="button primary"
        :disabled="busy || working"
        @click="command('download_application_update')"
      >
        Version {{ state.version }} herunterladen
      </button>
      <button
        v-if="state?.phase === 'downloaded'"
        class="button primary"
        :disabled="busy || working"
        @click="command('install_application_update')"
      >
        Geprüftes Update installieren
      </button>
      <button v-if="state?.phase === 'downloading'" class="button secondary" @click="cancel">
        Download abbrechen
      </button>
    </div>
    <p v-if="state?.phase === 'current'" role="status">
      Sie verwenden die aktuelle stabile Version.
    </p>
    <p v-if="state?.phase === 'downloading'" role="status">
      {{ Math.round(state.downloaded / 1024 / 1024) }} MiB heruntergeladen<template
        v-if="state.total"
      >
        von {{ Math.round(state.total / 1024 / 1024) }} MiB</template
      >.
    </p>
    <pre v-if="state?.notes" tabindex="0">{{ state.notes }}</pre>
    <p v-if="!state?.configured" class="hint">
      Die integrierte Aktualisierung ist in diesem Entwicklungsbuild noch nicht eingerichtet.
    </p>
    <p v-if="message" role="alert">{{ message }}</p>
  </section>
</template>
