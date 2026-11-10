<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import type { MappingDecision, MappingSchema } from './mapping/types';
import { displayPath } from './mapping/export';
import DisclosurePanel from './DisclosurePanel.vue';
const props = defineProps<{
  source: MappingSchema;
  target: MappingSchema;
  decisions: MappingDecision[];
}>();
const emit = defineEmits<{
  choose: [source: string, target: string];
  skip: [source: string];
  reopen: [source: string];
}>();
const filter = ref('review'),
  query = ref('');
const list = ref<HTMLOListElement>(),
  filters = ref<HTMLDivElement>();
const selected = reactive<Record<string, string>>(Object.create(null));
const targetSearch = reactive<Record<string, string>>(Object.create(null));
const sources = computed(() => new Map(props.source.fields.map((f) => [f.key, f])));
const targets = computed(() => new Map(props.target.fields.map((f) => [f.key, f])));
const missingTargets = computed(() => {
  const assigned = new Set(
    props.decisions.filter((d) => d.state !== 'skipped' && d.target).map((d) => d.target),
  );
  return props.target.fields.filter((f) => !assigned.has(f.key));
});
const open = (d: MappingDecision) =>
  d.state === 'review' || (d.state === 'confirmed' && d.warnings.length > 0);
const counts = computed(() => ({
  all: props.decisions.length,
  review: props.decisions.filter(open).length,
  automatic: props.decisions.filter((d) => d.state === 'automatic').length,
  confirmed: props.decisions.filter((d) => d.state === 'confirmed').length,
  skipped: props.decisions.filter((d) => d.state === 'skipped').length,
}));
watch(
  () => props.decisions,
  () => {
    filter.value = counts.value.review ? 'review' : 'all';
    for (const key of Object.keys(selected)) delete selected[key];
    for (const key of Object.keys(targetSearch)) delete targetSearch[key];
  },
  { immediate: true },
);
const visible = computed(() =>
  props.decisions.filter(
    (d) =>
      (filter.value === 'all' ||
        (filter.value === 'review' ? open(d) : d.state === filter.value)) &&
      (!query.value ||
        [
          sources.value.get(d.source)?.label,
          displayPath(sources.value.get(d.source)),
          displayPath(targets.value.get(d.target)),
        ]
          .join(' ')
          .toLocaleLowerCase('de')
          .includes(query.value.toLocaleLowerCase('de'))),
  ),
);
const limit = ref(30);
watch([filter, query], () => {
  limit.value = 30;
});
const labels = {
  automatic: 'Automatisch zugeordnet',
  review: 'Bitte entscheiden',
  confirmed: 'Zuordnung bestätigt',
  skipped: 'Nicht zugeordnet',
};
function searchTargets(key: string) {
  const text = (targetSearch[key] || '').toLocaleLowerCase('de');
  return text
    ? props.target.fields
        .filter((f) => [f.label, displayPath(f)].join(' ').toLocaleLowerCase('de').includes(text))
        .slice(0, 40)
    : [];
}
async function act(kind: 'choose' | 'skip' | 'reopen', sourceKey: string, targetKey = '') {
  const trigger = document.activeElement;
  const index = visible.value.findIndex((d) => d.source === sourceKey);
  if (kind === 'choose') emit('choose', sourceKey, targetKey);
  else if (kind === 'skip') emit('skip', sourceKey);
  else emit('reopen', sourceKey);
  await nextTick();
  if (trigger?.isConnected) return;
  const next =
    visible.value.find((d) => d.source === sourceKey) ||
    visible.value[Math.min(index, visible.value.length - 1)];
  const row = Array.from(list.value?.querySelectorAll<HTMLElement>('.mapping-row') || []).find(
    (e) => e.dataset.source === next?.source,
  );
  const target =
    row?.querySelector<HTMLElement>('h3') ||
    filters.value?.querySelector<HTMLElement>('[aria-pressed="true"]');
  target?.focus({ preventScroll: false });
}
</script>
<template>
  <section class="panel mapping-results" aria-label="Automatische Zuordnungsergebnisse">
    <h2 tabindex="-1">Zuordnungen prüfen</h2>
    <p>
      Klare Treffer sind automatisch zugeordnet. Prüfen Sie die offenen Fälle und bestätigen Sie das
      passende Ziel. Fachliche Regeln bleiben als Hinweise sichtbar.
    </p>
    <div ref="filters" class="queue-filters" role="group" aria-label="Zuordnungen filtern">
      <button
        v-for="item in [
          { id: 'review', label: 'Offene Fälle' },
          { id: 'all', label: 'Alle Zuordnungen' },
          { id: 'automatic', label: 'Automatisch' },
          { id: 'confirmed', label: 'Bestätigt' },
          { id: 'skipped', label: 'Ausgelassen' },
        ]"
        :key="item.id"
        class="button secondary"
        :aria-pressed="filter === item.id"
        @click="filter = item.id"
      >
        {{ item.label }} · {{ counts[item.id as keyof typeof counts] }}
      </button>
    </div>
    <label
      >Zuordnungen durchsuchen<input v-model="query" type="search" placeholder="Feldname oder Pfad"
    /></label>
    <p role="status">
      {{ counts.automatic }} automatisch · {{ counts.confirmed }} bestätigt ·
      {{ counts.review }} mit Klärungsbedarf · {{ counts.skipped }} ausgelassen
    </p>
    <DisclosurePanel
      v-if="missingTargets.length"
      :label="`Zielfelder ohne Vorschlag · ${missingTargets.length}`"
    >
      <p class="hint">
        Diese Ziele sind noch keinem Quellfeld zugeordnet. Erforderliche Felder benötigen eine
        Quelle oder eine ausdrücklich festgelegte Regel. Ein Vorschlag allein bestätigt keine
        vollständige Transformation.
      </p>
      <ul>
        <li v-for="field in missingTargets" :key="field.key">
          {{ displayPath(field) }} · {{ field.required ? 'erforderlich' : 'optional' }}
        </li>
      </ul>
    </DisclosurePanel>
    <p v-if="!visible.length" class="hint">Keine Zuordnungen in diesem Filter.</p>
    <ol ref="list" class="mapping-list" role="list">
      <li
        v-for="decision in visible.slice(0, limit)"
        :key="decision.source"
        class="mapping-row"
        :data-source="decision.source"
      >
        <div class="check-heading">
          <h3 tabindex="-1">{{ sources.get(decision.source)?.label }}</h3>
          <span
            class="badge"
            :class="
              decision.state === 'automatic' || decision.state === 'confirmed'
                ? 'confirmed'
                : decision.state === 'skipped'
                  ? 'notApplicable'
                  : 'unknown'
            "
            >{{ labels[decision.state] }}</span
          >
        </div>
        <p class="mono hint">{{ displayPath(sources.get(decision.source)) }}</p>
        <p><strong>Ziel:</strong> {{ displayPath(targets.get(decision.target)) }}</p>
        <ul v-if="decision.warnings.length" class="mapping-warnings">
          <li v-for="warning in decision.warnings" :key="warning">{{ warning }}</li>
        </ul>
        <template v-if="decision.state !== 'skipped'">
          <label :for="`target-${decisions.indexOf(decision)}`">Zielfeld auswählen</label>
          <select
            :id="`target-${decisions.indexOf(decision)}`"
            :value="selected[decision.source] ?? decision.target"
            @change="selected[decision.source] = ($event.target as HTMLSelectElement).value"
          >
            <option value="">Kein Ziel auswählen</option>
            <option
              v-if="
                selected[decision.source] &&
                !decision.candidates.some((c) => c.target === selected[decision.source])
              "
              :value="selected[decision.source]"
            >
              {{ displayPath(targets.get(selected[decision.source]!)) }}
            </option>
            <option
              v-for="candidate in decision.candidates"
              :key="candidate.target"
              :value="candidate.target"
            >
              {{ displayPath(targets.get(candidate.target)) }}
            </option>
          </select>
          <div class="action-row">
            <button
              class="button secondary"
              :disabled="!(selected[decision.source] ?? decision.target)"
              @click="act('choose', decision.source, selected[decision.source] ?? decision.target)"
            >
              Zuordnung bestätigen
            </button>
            <button class="button secondary" @click="act('skip', decision.source)">
              Feld nicht zuordnen
            </button>
          </div>
          <DisclosurePanel label="Anderes Zielfeld suchen">
            <label
              >Zielfeld suchen<input v-model="targetSearch[decision.source]" type="search"
            /></label>
            <ul class="mapping-targets">
              <li v-for="targetField in searchTargets(decision.source)" :key="targetField.key">
                <button
                  class="button secondary"
                  @click="selected[decision.source] = targetField.key"
                >
                  {{ displayPath(targetField) }}
                </button>
              </li>
            </ul>
            <p class="hint">
              Bis zu 40 passende Ziele. Die Auswahl wird mit Zuordnung bestätigen übernommen.
            </p>
          </DisclosurePanel>
        </template>
        <button v-else class="button secondary" @click="act('reopen', decision.source)">
          Erneut zuordnen
        </button>
        <DisclosurePanel label="Begründung und Excel-Fundstelle">
          <ul>
            <li v-for="reason in decision.reasons" :key="reason">{{ reason }}</li>
          </ul>
          <template
            v-for="row in decision.candidates.find((c) => c.target === decision.target)?.evidence ||
            []"
            :key="`${row.sheet}-${row.row}`"
          >
            <p>
              <strong>{{ row.sheet }}, Zeile {{ row.row }}</strong> · {{ row.cells.join(', ') }}
            </p>
            <p>{{ row.id }} · {{ row.label }}</p>
            <p v-if="row.note">{{ row.note }}</p>
            <p v-if="row.codelist.length">
              {{ row.codelist.length }} dokumentierte Codelisteneinträge. Die Übersetzungsregel muss
              separat geprüft werden.
            </p>
          </template>
        </DisclosurePanel>
      </li>
    </ol>
    <button v-if="visible.length > limit" class="button secondary" @click="limit += 30">
      Weitere 30 Zuordnungen anzeigen
    </button>
  </section>
</template>
