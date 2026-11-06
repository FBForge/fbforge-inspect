<script setup lang="ts">
import { computed, ref } from 'vue';
import type { Report } from '../contracts';
import type { PasteContext } from '../portable';
import { records } from '../resultValues';
import { addressingContexts } from '../addressing';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{
  context: [context: PasteContext];
  reverse: [id: string];
  keys: [text: string];
}>();
interface ReverseRow {
  service: string;
  region: string;
  complete: boolean;
  expectedFound: boolean;
  publishedContext: boolean;
  routes: { destinationId?: string; destinationName?: string }[];
}
const reverse = computed(() => records<ReverseRow>(props.report.data.reverseRoutes));
const contexts = computed(() => addressingContexts(props.report));
const expanded = ref(false),
  search = ref(''),
  limit = ref(20);
const filtered = computed(() =>
  contexts.value.filter((context) =>
    [
      context.parent,
      context.role,
      ...context.services,
      ...context.areas,
      ...context.addresses.map((address) => address.id),
    ]
      .join(' ')
      .toLocaleLowerCase('de')
      .includes(search.value.toLocaleLowerCase('de')),
  ),
);
const publicKeys = computed(
  () => props.report.data.keys?.flatMap((key) => (key.publicJwk ? [key.publicJwk] : [])) || [],
);
function route() {
  const d = props.report.data.destination;
  if (d?.destinationId) emit('reverse', d.destinationId);
}
</script>
<template>
  <section v-if="reverse.length" class="panel">
    <h3>Rückwärtsrouting</h3>
    <article v-for="(row, index) in reverse" :key="index">
      <h4>{{ row.service }} · {{ row.region }}</h4>
      <p>
        {{
          !row.complete
            ? 'Abfrage unvollständig'
            : row.expectedFound
              ? 'Erwarteter Zustellpunkt gefunden'
              : 'Erwarteter Zustellpunkt nicht zurückgegeben'
        }}
      </p>
      <p>
        {{
          row.publishedContext
            ? 'Leistung und Gebiet sind in einem gemeinsamen veröffentlichten Kontext vorhanden.'
            : 'Gemeinsamer veröffentlichter Leistungs-/Gebietsbezug nicht bestätigt.'
        }}
      </p>
      <button
        class="button secondary"
        @click="
          emit('context', {
            operation: 'routing',
            environment: report.environment,
            apiVersion: report.apiVersion,
            input: { service: row.service, region: row.region, regionKind: 'ars' },
          })
        "
      >
        Routing mit diesem Kontext vorbereiten
      </button>
    </article>
  </section>
  <details
    v-if="report.data.addressingSignatures?.length || report.data.pvogRelations?.length"
    class="panel"
    @toggle="expanded = ($event.target as HTMLDetailsElement).open"
  >
    <summary>Zusammenhänge der Adressierung nachvollziehen</summary>
    <p>
      Routing-Treffer, veröffentlichte Zuständigkeit und Signatur sind getrennte Belege. Eine
      gültige Signatur allein garantiert keinen Routing-Treffer.
    </p>
    <template v-if="expanded">
      <label v-if="contexts.length"
        >Adressierungskontexte durchsuchen<input v-model="search" type="search" @input="limit = 20"
      /></label>
      <p v-if="contexts.length">
        {{ Math.min(limit, filtered.length) }} von {{ filtered.length }} passenden geladenen
        Kontexten
      </p>
      <article
        v-for="context in filtered.slice(0, limit)"
        :key="context.path"
        class="imported-finding"
      >
        <h4>
          {{ context.parentClass || 'Übergeordnetes PVOG-Objekt' }} ·
          {{ context.parent || 'Kennung nicht veröffentlicht' }}
        </h4>
        <ol>
          <li>
            <strong>Veröffentlichte Zuständigkeit</strong>
            <p>{{ context.role || 'Keine Rolle veröffentlicht' }}</p>
            <p>
              Leistungsbeschreibung: {{ context.services.join(', ') || 'Nicht veröffentlicht' }}
            </p>
            <p>Gebiet: {{ context.areas.join(', ') || 'Nicht veröffentlicht' }}</p>
          </li>
          <li>
            <strong>Adressierungsinformation</strong>
            <p v-if="!context.addresses.length">
              Keine auswertbare Adressierungssignatur in dieser Zuständigkeit.
            </p>
            <div v-for="address in context.addresses" :key="address.path">
              <p>
                {{ address.verified ? 'Signatur verifiziert' : 'Signatur nicht verifiziert' }} ·
                {{
                  address.contextMatches
                    ? 'Leistungs-/Gebietskontext bestätigt'
                    : 'Leistungs-/Gebietskontext nicht bestätigt'
                }}
              </p>
              <p class="mono">Zustellpunkt: {{ address.id || 'Nicht angegeben' }}</p>
              <p>
                {{
                  address.inRouting
                    ? 'Kennung auch in der geladenen Routingantwort enthalten.'
                    : 'Kennung in der geladenen Routingantwort nicht enthalten. Vollständigkeit der Abfrage beachten.'
                }}
              </p>
              <button
                v-if="address.verified && address.contextMatches && address.id"
                class="button secondary"
                @click="
                  emit('context', {
                    operation: 'destination',
                    environment: report.environment,
                    apiVersion: report.apiVersion,
                    input: { destinationId: address.id },
                  })
                "
              >
                Diesen Zustellpunkt vorbereiten
              </button>
            </div>
          </li>
        </ol>
        <p v-for="(point, i) in context.openPoints" :key="i">
          Offener Punkt: {{ point.title }} · {{ point.detail }}
        </p>
        <details>
          <summary>Quelle und Kontextpfad</summary>
          <p class="mono">{{ context.path }}</p>
          <p>{{ context.source || 'Quelle in den Einzelbefunden nachvollziehen.' }}</p>
        </details>
      </article>
      <button v-if="limit < filtered.length" class="button secondary" @click="limit += 20">
        Weitere Kontexte anzeigen
      </button>
    </template>
    <template v-if="!contexts.length"
      ><p v-for="(signature, index) in report.data.addressingSignatures" :key="index">
        {{ signature.context }} ·
        {{ signature.verified ? 'Signatur verifiziert' : 'Signatur nicht verifiziert' }} ·
        {{ signature.claims?.destinationId || 'Keine Destination-ID angegeben' }}
      </p></template
    >
    <p>
      Mehrere zuständige Organisationen können berechtigt nebeneinander bestehen. Die interne
      Ursache fehlender Routing-Treffer lässt sich aus öffentlichen Daten nicht immer feststellen.
    </p>
  </details>
  <section v-if="report.data.destination || publicKeys.length" class="panel">
    <h3>Gezielt weiterprüfen</h3>
    <div class="action-row">
      <button
        v-if="report.data.destination && report.environment !== 'STAGE'"
        class="button secondary"
        @click="route"
      >
        Rückwärtsrouting vorbereiten
      </button>
      <button
        v-if="publicKeys.length"
        class="button secondary"
        @click="emit('keys', JSON.stringify({ keys: publicKeys }, null, 2))"
      >
        Öffentliche Schlüssel separat vorbereiten
      </button>
    </div>
    <p class="hint">Die Angaben werden übernommen. Eine neue Prüfung starten Sie ausdrücklich.</p>
  </section>
</template>
