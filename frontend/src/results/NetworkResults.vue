<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';
import { records } from '../resultValues';
interface NetworkCheck {
  name?: string;
  host?: string;
  port?: number;
  url?: string;
  method?: string;
  detail?: string;
  status?: string;
  httpStatus?: number;
  durationMs?: number;
  code?: string;
}
const props = defineProps<{ report: Report }>();
const checks = computed(() => records<NetworkCheck>(props.report.data.networkChecks));
const allowlist = computed(() =>
  checks.value
    .map((item) => `${item.method} ${item.url}\nZiel: ${item.host}:${item.port}`)
    .join('\n\n'),
);
</script>
<template>
  <section v-if="checks.length" class="panel">
    <h3>Netzwerkverbindungen</h3>
    <div
      class="table-scroll"
      tabindex="0"
      role="region"
      aria-label="Ergebnisse der Netzwerkprüfung"
    >
      <table>
        <thead>
          <tr>
            <th>Dienst / Ziel</th>
            <th>Ergebnis</th>
            <th>Dauer</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(item, index) in checks" :key="index">
            <td>
              <strong>{{ item.name }}</strong>
              <p class="mono hint">{{ item.method }} {{ item.host }}:{{ item.port }}</p>
            </td>
            <td>
              {{ item.detail }}
              <p v-if="item.code" class="mono hint">{{ item.code }}</p>
              <p v-if="item.httpStatus" class="hint">HTTP {{ item.httpStatus }}</p>
            </td>
            <td>{{ item.durationMs }} ms</td>
          </tr>
        </tbody>
      </table>
    </div>
    <details>
      <summary>Freigabeliste für die IT anzeigen</summary>
      <p class="hint">
        Konkrete Zieladressen, Ports und Methoden dieser Prüfung. Proxy-Zugangsdaten sind nicht
        enthalten. Die Liste ist auch Bestandteil des exportierbaren Diagnoseberichts.
      </p>
      <pre tabindex="0">{{ allowlist }}</pre>
    </details>
  </section>
</template>
