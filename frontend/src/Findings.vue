<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import FindingEvidence from './FindingEvidence.vue';
import DisclosurePanel from './DisclosurePanel.vue';
import { diagnosisSummary } from './diagnosisSummary';
import { labels, type Report, type Status } from './contracts';
import {
  overview,
  overviewDetail,
  findingDetail,
  findingNext,
  specificLimitation,
  statusOrder,
} from './diagnosis';
const props = defineProps<{ report: Report }>();
const filter = ref('all');
const compactOpen = ref(false);
watch(filter, (value) => {
  compactOpen.value = value !== 'all';
});
const conclusion = computed(() => diagnosisSummary(props.report));
watch(
  () => props.report,
  () => {
    filter.value = 'all';
    compactOpen.value = false;
  },
);
const summary = computed(() =>
  statusOrder
    .map((id) => ({
      id,
      label: labels[id],
      count: props.report.findings.filter((f) => f.status === id).length,
    }))
    .filter((s) => s.count),
);
const visible = computed(() =>
  [...props.report.findings]
    .filter((f) => filter.value === 'all' || f.status === filter.value)
    .sort(
      (a, b) => statusOrder.indexOf(a.status as Status) - statusOrder.indexOf(b.status as Status),
    ),
);
const attention = computed(() =>
  visible.value.filter((f) => !['confirmed', 'notApplicable'].includes(f.status)),
);
const compact = computed(() =>
  visible.value.filter((f) => ['confirmed', 'notApplicable'].includes(f.status)),
);
function select(id: string) {
  filter.value = filter.value === id ? 'all' : id;
}
</script>
<template>
  <div class="result-overview panel" role="status">
    <h3>{{ overview(report) }}</h3>
    <template v-if="conclusion">
      <p>{{ conclusion.message }}</p>
      <dl class="diagnosis-areas" aria-label="Zusammenfassung der Prüfbereiche">
        <div v-for="item in conclusion.items" :key="item.id" :class="item.status">
          <dt><span class="status-dot" aria-hidden="true"></span>{{ item.label }}</dt>
          <dd>
            <span class="badge" :class="item.status">{{ labels[item.status] }}</span
            >{{ item.text }}
          </dd>
        </div>
      </dl>
      <p class="hint">{{ overviewDetail(report) }}</p>
    </template>
    <p v-else>{{ overviewDetail(report) }}</p>
    <div class="result-summary" aria-label="Befunde filtern">
      <button
        v-for="s in summary"
        :key="s.id"
        type="button"
        class="badge"
        :class="s.id"
        :aria-pressed="filter === s.id"
        @click="select(s.id)"
      >
        {{ s.count }} {{ s.label }}
      </button>
    </div>
  </div>
  <section v-if="report.findings.length" aria-label="Befunde">
    <div v-if="attention.length || filter !== 'all'" class="section-heading">
      <h3>Befunde</h3>
      <label class="filter-label"
        >Anzeigen<select v-model="filter">
          <option value="all">Alle Befunde</option>
          <option v-for="(label, id) in labels" :key="id" :value="id">{{ label }}</option>
        </select></label
      >
    </div>
    <article v-for="(f, i) in attention" :key="i" class="finding panel" :class="f.status">
      <div class="finding-header">
        <span class="status-dot" aria-hidden="true"></span>
        <h3>{{ f.title }}</h3>
        <span class="badge" :class="f.status">{{ labels[f.status as Status] }}</span>
      </div>
      <p>{{ findingDetail(f) }}</p>
      <p v-if="findingNext(f)" class="hint">
        <strong>Nächster Schritt:</strong> {{ findingNext(f) }}
      </p>
      <p v-if="specificLimitation(f)" class="hint">
        <strong>Beweisgrenze:</strong> {{ specificLimitation(f) }}
      </p>
      <FindingEvidence :finding="f" />
    </article>
    <DisclosurePanel
      v-if="compact.length"
      v-model:open="compactOpen"
      class="compact-findings"
      :label="`${compact.length} ${
        filter === 'notApplicable'
          ? 'nicht anwendbare'
          : compact.every((f) => f.status === 'confirmed')
            ? 'bestätigte'
            : 'weitere'
      } ${compact.length === 1 ? 'Einzelprüfung' : 'Einzelprüfungen'}`"
    >
      <div v-for="(f, i) in compact" :key="i" class="compact-finding">
        <div class="finding-header">
          <h4>{{ f.title }}</h4>
          <span class="badge" :class="f.status">{{ labels[f.status as Status] }}</span>
        </div>
        <p>{{ findingDetail(f) }}</p>
        <p v-if="specificLimitation(f)" class="hint">{{ specificLimitation(f) }}</p>
        <FindingEvidence :finding="f" />
      </div>
    </DisclosurePanel>
    <p v-if="!visible.length" class="hint">Keine Befunde in diesem Filter.</p>
  </section>
</template>
