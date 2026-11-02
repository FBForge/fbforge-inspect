<script setup lang="ts">
import { buildCommit } from './build';
import ReferenceLink from './ReferenceLink.vue';
const emit = defineEmits<{ network: [] }>();
</script>
<template>
  <section class="panel about">
    <img src="/inspect.png" alt="" width="88" height="88" />
    <h2>FBForge Inspect <span class="muted">0.1.0</span></h2>
    <p class="mono">Build: {{ buildCommit === 'local' ? 'Lokaler Arbeitsstand' : buildCommit }}</p>
    <p>
      Eine eigenständige Open-Source-Desktop-App für die Diagnose öffentlicher FIT-Connect- und
      PVOG-Schnittstellen.
    </p>
    <p>Entwickelt von FBForge. Lizenz: EUPL-1.2.</p>
    <p>
      <ReferenceLink reference="repository">github.com/FBForge/fbforge-inspect</ReferenceLink>
    </p>
    <p class="hint">
      Kein offizielles Produkt von FITKO. Eine Diagnose ist eine Momentaufnahme und keine
      Zertifizierung.
    </p>
    <h3>Netzwerk</h3>
    <p>
      Inspect verbindet sich direkt mit öffentlichen Quellen. Authentifizierte Proxys, PAC und
      eigene Unternehmens-CAs werden in 0.1.0 nicht eingerichtet.
    </p>
    <button class="button secondary" @click="emit('network')">Netzwerkhinweise öffnen</button>
  </section>
</template>
