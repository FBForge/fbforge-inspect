<script setup lang="ts">
import { computed, ref } from 'vue';
import { labels, type Report, type Status } from './contracts';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{
  destination: [id: string];
  schema: [url: string];
  area: [id: string];
}>();
const filter = ref('all');
const findings = computed(() =>
  props.report.findings.filter((f) => filter.value === 'all' || f.status === filter.value),
);
const summary = computed(() =>
  Object.entries(labels)
    .map(([id, label]) => ({
      id,
      label,
      count: props.report.findings.filter((f) => f.status === id).length,
    }))
    .filter((x) => x.count),
);
type Item = Record<string, any>;
function records(value: unknown): Item[] {
  return Array.isArray(value)
    ? value.filter((x) => x && typeof x === 'object' && !Array.isArray(x))
    : [];
}
function list(value: unknown) {
  return Array.isArray(value) ? value.map(String).join(', ') : '';
}
const destination = computed(() => props.report.data.destination as Item | undefined);
const rows = computed(() =>
  records(
    props.report.data.routes || props.report.data.destinations || props.report.data.areas || [],
  ),
);
function identifier(value: unknown) {
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object') {
    const id = value as Item;
    return [id.schemeAgencyID, id.value].filter((x) => typeof x === 'string' && x).join(' · ');
  }
  return 'Keine ID veröffentlicht';
}
const certificates = computed(() => records(props.report.data.certificates));
const pvog = computed(() =>
  records(props.report.data.pvogRelations).flatMap((parent, parentIndex) =>
    records(parent.zustaendigkeit).map((relation, index) => {
      const prefix = `$.pvogRelations[${parentIndex}].zustaendigkeit[${index}].`;
      const signatures = records(props.report.data.addressingSignatures).filter(
        (s) => typeof s.context === 'string' && s.context.startsWith(prefix),
      );
      return {
        parent: identifier(parent.uebergeordnetesObjektID),
        id: identifier(relation.id),
        role: relation.rolle?.name || relation.rolle?.code || 'Keine Rolle veröffentlicht',
        addresses: signatures
          .filter((s) => s.verified === true && typeof s.claims?.destinationId === 'string')
          .map((s) => ({
            id: s.claims.destinationId,
            inRouting: rows.value.some((row) => row.destinationId === s.claims.destinationId),
          })),
        unverified: signatures.some((s) => s.verified !== true),
      };
    }),
  ),
);
const services = computed(
  () =>
    [
      ...records(destination.value?.publicServices).map((s: Item) => ({
        ...s,
        kind: 'Leistung',
        schemas: records(s.submissionSchemas),
      })),
      ...records(destination.value?.processMessages).map((s: Item) => ({
        ...s,
        kind: 'Prozessnachricht',
        identifier: s.processModelId + ' / ' + s.messageId,
        schemas: records(s.schemas),
      })),
    ] as Item[],
);
const structure = computed(() => (props.report.data.structure || []) as Item[]);
const references = computed(() =>
  records(props.report.data.references).map((ref) => {
    let url = '';
    try {
      const candidate = new URL(
        String(ref.target),
        typeof props.report.data.schemaSource === 'string'
          ? props.report.data.schemaSource
          : undefined,
      );
      if (candidate.protocol === 'https:') {
        candidate.hash = '';
        url = candidate.toString();
      }
    } catch {
      /* Local references stay local. */
    }
    return { target: ref.target, path: ref.path, url };
  }),
);
function canOpen(value: unknown) {
  return typeof value === 'string' && /^https:\/\//i.test(value);
}
</script>
<template>
  <section class="results" aria-label="Diagnoseergebnis">
    <div class="section-heading">
      <h2>Diagnoseergebnis</h2>
      <span class="muted"
        >{{ report.environment }} · {{ report.apiVersion }} ·
        {{ new Date(report.createdAt).toLocaleTimeString('de-DE') }}</span
      >
    </div>
    <div class="result-summary">
      <span v-for="s in summary" :key="s.id" class="badge" :class="s.id"
        >{{ s.count }} {{ s.label }}</span
      >
    </div>
    <div v-if="destination" class="panel destination-summary">
      <span class="eyebrow">Öffentlicher Zustellpunkt</span>
      <h3 class="mono">{{ destination.destinationId }}</h3>
      <div class="inline-facts">
        <span
          >Status <strong>{{ destination.status }}</strong></span
        ><span
          >Typ <strong>{{ destination.type || 'in v2 nicht veröffentlicht' }}</strong></span
        ><span
          >Metadaten
          <strong>{{ list(destination.metadataVersions) || 'keine Angabe' }}</strong></span
        >
      </div>
    </div>
    <div v-if="rows.length" class="panel">
      <h3>
        {{ report.operation === 'areas' ? 'Gefundene Gebiete' : 'Gefundene Kontexte' }}
        <span class="muted">({{ rows.length }})</span>
      </h3>
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th>Kontext</th>
              <th>ID</th>
              <th>Quelle / Status</th>
              <th>Aktion</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, index) in rows" :key="index">
              <td>
                {{
                  row.destinationName ||
                  row.destinationShortName ||
                  row.name ||
                  `Kontext ${index + 1}`
                }}
              </td>
              <td class="mono">{{ row.destinationId || row.id }}</td>
              <td>{{ row.origin || row.status || row.type || 'Öffentliche API' }}</td>
              <td>
                <button
                  v-if="row.destinationId"
                  class="text-button"
                  @click="emit('destination', row.destinationId)"
                >
                  Destination prüfen →</button
                ><button v-else-if="row.id" class="text-button" @click="emit('area', row.id)">
                  Gebiet übernehmen →
                </button>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div v-if="pvog.length" class="panel">
      <h3>PVOG-Zuständigkeitskontexte</h3>
      <p class="hint">
        Jede Zeile bleibt ihrem übergeordneten Objekt zugeordnet. Ein fehlender Treffer in dieser
        Routingantwort belegt keine interne Fehlerursache.
      </p>
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th>Übergeordnetes Objekt</th>
              <th>Rolle / Zuständigkeit</th>
              <th>Verifizierte Adressierung</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(context, index) in pvog" :key="index">
              <td class="mono">{{ context.parent }}</td>
              <td>
                {{ context.role }}
                <p class="mono hint">{{ context.id }}</p>
              </td>
              <td>
                <div v-for="(address, i) in context.addresses" :key="i">
                  <button class="text-button mono" @click="emit('destination', address.id)">
                    {{ address.id }} →
                  </button>
                  <p class="hint">
                    {{
                      address.inRouting
                        ? 'ID in dieser Routingantwort enthalten'
                        : 'ID in dieser Routingantwort nicht enthalten'
                    }}
                  </p>
                </div>
                <p v-if="!context.addresses.length" class="hint">
                  Keine verifizierte Destination-ID ausgewertet.
                </p>
                <p v-if="context.unverified" class="hint">
                  Zusätzlich ungeprüfte oder nicht verifizierte Signatur vorhanden.
                </p>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div v-if="certificates.length" class="panel">
      <h3>Zertifikatsketten</h3>
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th>Schlüssel / Inhaber</th>
              <th>Aussteller / Laufzeitende</th>
              <th>SHA-256</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(certificate, index) in certificates" :key="index">
              <td>
                {{ certificate.subject }}
                <p class="mono hint">{{ certificate.kid }} · {{ certificate.keyPath }}</p>
              </td>
              <td>
                {{ certificate.issuer }}
                <p class="hint">{{ certificate.notAfter }}</p>
              </td>
              <td class="mono">{{ certificate.sha256 }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div v-if="services.length" class="panel">
      <h3>Leistungen und Prozessnachrichten</h3>
      <article v-for="(s, i) in services" :key="i" class="service">
        <span class="eyebrow">{{ s.kind }}</span>
        <h4 class="mono">{{ s.identifier }}</h4>
        <p class="muted">Regionen: {{ list(s.regions) || 'keine Angabe' }}</p>
        <div v-for="(schema, j) in s.schemas" :key="j" class="schema-line">
          <span class="mono">{{ schema.schemaUri }}</span
          ><span class="badge neutral">{{ schema.mimeType }}</span
          ><button
            v-if="canOpen(schema.schemaUri)"
            class="text-button"
            @click="emit('schema', schema.schemaUri)"
          >
            Schema ansehen →
          </button>
        </div>
      </article>
    </div>
    <div v-if="structure.length" class="panel">
      <h3>
        Struktur <span class="muted">{{ report.data.dialect || 'Dialekt nicht angegeben' }}</span>
      </h3>
      <div class="table-scroll">
        <table>
          <thead>
            <tr>
              <th>Pfad</th>
              <th>Typ / Bedeutung</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, i) in structure" :key="i">
              <td class="mono">{{ row.path }}</td>
              <td>
                {{ row.type || row.title || row.name || row.base || 'Strukturknoten' }}
                <span class="muted">{{ row.description }}</span>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div v-if="references.length" class="panel">
      <h3>Schemareferenzen</h3>
      <p class="hint">
        Ein weiterer Abruf erfolgt erst nach Auswahl und erneutem Start der Prüfung.
      </p>
      <div v-for="(reference, index) in references" :key="index" class="schema-line">
        <span class="mono">{{ reference.target }}</span
        ><button v-if="reference.url" class="text-button" @click="emit('schema', reference.url)">
          Referenz vorbereiten →</button
        ><span v-else class="hint">Lokale oder nicht direkt abrufbare Referenz</span>
      </div>
    </div>
    <div class="section-heading">
      <h3>Befunde</h3>
      <label class="filter-label"
        >Anzeigen<select v-model="filter">
          <option value="all">Alle Befunde</option>
          <option v-for="(label, id) in labels" :key="id" :value="id">{{ label }}</option>
        </select></label
      >
    </div>
    <article v-for="(f, i) in findings" :key="i" class="finding panel" :class="f.status">
      <div class="finding-header">
        <span class="status-dot" aria-hidden="true"></span>
        <h3>{{ f.title }}</h3>
        <span class="badge" :class="f.status">{{ labels[f.status as Status] }}</span>
      </div>
      <p>{{ f.detail }}</p>
      <p v-if="f.nextStep" class="hint"><strong>Nächster Schritt:</strong> {{ f.nextStep }}</p>
      <p v-if="f.limitation" class="hint"><strong>Beweisgrenze:</strong> {{ f.limitation }}</p>
      <details v-if="f.source || f.path">
        <summary>Beleg und Feldpfad</summary>
        <p class="mono hint">{{ f.path }}</p>
        <p class="mono hint">{{ f.source || 'Lokale Eingabe' }}</p>
      </details>
    </article>
    <div v-if="!findings.length" class="empty-state small">Keine Befunde in diesem Filter.</div>
    <details class="panel">
      <summary>Normalisierte Daten und Vergleich</summary>
      <pre>{{ JSON.stringify(report.data, null, 2) }}</pre>
    </details>
    <details class="panel">
      <summary>Abfrageprotokoll · {{ report.requests.length }} Abrufe</summary>
      <p class="hint">HTTP-Erfolg und fachlicher Befund werden getrennt bewertet.</p>
      <div class="table-scroll">
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
  </section>
</template>
