<script setup lang="ts">
import type { Report } from '../contracts';

defineProps<{ report: Report }>();
</script>
<template>
  <details class="panel">
    <summary>Abfrageprotokoll · {{ report.requests.length }} Abrufe</summary>
    <p class="hint">HTTP-Erfolg und fachlicher Befund werden getrennt bewertet.</p>
    <div class="table-scroll" tabindex="0" role="region" aria-label="Abfrageprotokoll">
      <table>
        <thead>
          <tr>
            <th>Zeit</th>
            <th>HTTP</th>
            <th>Öffentliche URL</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(request, i) in report.requests" :key="i">
            <td>{{ new Date(request.at).toLocaleTimeString('de-DE') }}</td>
            <td>{{ request.method }} {{ request.status || 'Fehler' }}</td>
            <td class="mono">{{ request.url }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </details>
</template>
