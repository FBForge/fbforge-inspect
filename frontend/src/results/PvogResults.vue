<script setup lang="ts">
import { computed } from 'vue';
import type { Report } from '../contracts';
import type {
  OnlineServiceReference,
  OnlineServiceSource,
  PvogParent,
  AddressingSignature,
  PvogRelation,
} from '../reportData';
import {
  routingRows,
  records,
  list,
  identifier,
  codeLabel,
  onlineName,
  parameters,
} from '../resultValues';
const props = defineProps<{ report: Report }>();
const emit = defineEmits<{ destination: [id: string] }>();
const rows = computed(() => routingRows(props.report.data));
const onlineServices = computed(() =>
  records<OnlineServiceReference>(props.report.data.pvogOnlineServices).map((entry) => ({
    ...entry,
    data:
      records<OnlineServiceSource>(props.report.data.pvogOnlineServiceSources)[entry.sourceIndex]
        ?.data || {},
  })),
);
const pvog = computed(() =>
  records<PvogParent>(props.report.data.pvogRelations).flatMap((parent, parentIndex) =>
    records<PvogRelation>(parent.zustaendigkeit).map((relation, index) => {
      const prefix = `$.pvogRelations[${parentIndex}].zustaendigkeit[${index}].`;
      const signatures = records<AddressingSignature>(
        props.report.data.addressingSignatures,
      ).filter((s) => typeof s.context === 'string' && s.context.startsWith(prefix));
      return {
        path: prefix.slice(0, -1),
        parent: identifier(parent.uebergeordnetesObjektID),
        parentClass: codeLabel(parent.uebergeordnetesObjektKlasse || {}),
        parentFields: parent,
        id: identifier(relation.id),
        role:
          records(Array.isArray(relation.rolle) ? relation.rolle : [relation.rolle])
            .map(codeLabel)
            .join(', ') || 'Keine Rolle veröffentlicht',
        services: records(relation.leistungID).map(identifier),
        areas: records(relation.gebietID).map(identifier),
        addresses: signatures
          .filter((s) => s.verified === true && typeof s.claims?.destinationId === 'string')
          .map((s) => ({
            id: s.claims!.destinationId!,
            inRouting: rows.value.some((row) => row.destinationId === s.claims?.destinationId),
          })),
        unverified: signatures.some((s) => s.verified !== true),
      };
    }),
  ),
);
</script>
<template>
  <details v-if="pvog.length" class="panel">
    <summary>PVOG-Zuständigkeitskontexte im Detail</summary>
    <h3>PVOG-Adressierungskontexte</h3>
    <p class="hint">
      Adressierungssignaturen aus PVOG identifizieren den Zustellpunkt. Fachdatenschemata werden an
      dessen Leistungen in FIT-Connect geprüft. Jede Zeile bleibt ihrem übergeordneten Objekt
      zugeordnet. Ein fehlender Treffer in dieser Routingantwort belegt keine interne Fehlerursache.
    </p>
    <div
      class="table-scroll wide-details-table"
      tabindex="0"
      role="region"
      aria-label="PVOG-Zuständigkeitskontexte"
    >
      <table>
        <thead>
          <tr>
            <th>Übergeordnetes Objekt</th>
            <th>Rolle / Zuständigkeit</th>
            <th>Verifizierte Adressierung</th>
          </tr>
        </thead>
        <tbody>
          <template v-for="(context, index) in pvog" :key="index"
            ><tr>
              <td>
                <p>{{ context.parentClass || 'Keine Objektklasse veröffentlicht' }}</p>
                <p class="mono">{{ context.parent }}</p>
              </td>
              <td>
                {{ context.role }}
                <p class="mono hint">{{ context.id }}</p>
                <p v-for="(service, i) in context.services" :key="`service-${i}`" class="mono hint">
                  Leistung: {{ service }}
                </p>
                <p v-for="(area, i) in context.areas" :key="`area-${i}`" class="mono hint">
                  Gebiet: {{ area }}
                </p>
              </td>
              <td>
                <div v-for="(address, i) in context.addresses" :key="i">
                  <button class="text-button mono" @click="emit('destination', address.id)">
                    {{ address.id }}
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
            <tr class="detail-row">
              <td colspan="3">
                <details>
                  <summary>Alle Kontextfelder und Feldpfad</summary>
                  <p class="mono hint">{{ context.path }}</p>
                  <pre tabindex="0">{{ JSON.stringify(context.parentFields, null, 2) }}</pre>
                </details>
              </td>
            </tr>
          </template>
        </tbody>
      </table>
    </div>
  </details>
  <details v-if="onlineServices.length" class="panel">
    <summary>PVOG-Onlinedienste und Parameter im Detail</summary>
    <h3>PVOG-Onlinedienste und Parameter</h3>
    <p class="hint">
      Parameter bleiben beim übergeordneten Onlinedienst. Sie werden als veröffentlichte
      Konfiguration angezeigt. Werte und verlinkte Ziele werden nicht ausgeführt oder abgerufen. Sie
      sind keine Fachdatenschemata und keine verifizierte Adressierung.
    </p>
    <div v-for="(service, index) in onlineServices" :key="index">
      <h4>{{ onlineName(service.data) || service.id }}</h4>
      <p v-if="onlineName(service.data)" class="mono hint">{{ service.id }}</p>
      <p v-if="service.matchesParent !== true" class="hint">
        Die Antwort-ID passt nicht zum übergeordneten Objekt. Parameter werden diesem Kontext nicht
        zugeordnet. Die Antwort bleibt als Beleg erhalten.
      </p>
      <div
        v-else
        class="table-scroll parameter-table"
        tabindex="0"
        role="region"
        aria-label="Onlinedienstparameter"
      >
        <table v-if="parameters(service.data).length">
          <thead>
            <tr>
              <th>Art / Parameter</th>
              <th>Veröffentlichter Wert</th>
              <th>Sprachen</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(parameter, i) in parameters(service.data)" :key="i">
              <td>
                {{ parameter.kind }}
                <p class="mono">{{ parameter.name }}</p>
              </td>
              <td class="mono">{{ parameter.parameterWert ?? 'Kein Wert veröffentlicht' }}</td>
              <td>
                {{ list(parameter.languageCode) || 'Keine Sprachangabe' }}
                <details>
                  <summary>Feldpfad</summary>
                  <code
                    >$.pvogOnlineServiceSources[{{ service.sourceIndex }}].data.{{
                      parameter.path
                    }}</code
                  >
                </details>
              </td>
            </tr>
          </tbody>
        </table>
        <p v-else class="hint">Keine EfA- oder individuellen Parameter veröffentlicht.</p>
      </div>
      <details>
        <summary>Alle Onlinedienstfelder und Quelle</summary>
        <p class="mono hint">{{ service.source }}</p>
        <pre tabindex="0">{{ JSON.stringify(service.data, null, 2) }}</pre>
      </details>
    </div>
  </details>
  <details v-if="report.data.pvogService" class="panel">
    <summary>PVOG-Leistungsbeschreibung und LeiKa-Zuordnung</summary>
    <p class="mono hint">{{ report.data.pvogServiceSource }}</p>
    <p class="hint">
      Leistungsbeschreibungs-ID und LeiKa-Schlüssel sind unterschiedliche Kennungen.
      referenzLeistungsschluessel enthält die veröffentlichte Zuordnung. Alle weiteren
      Leistungsfelder bleiben hier als Quelle erhalten.
    </p>
    <pre tabindex="0">{{ JSON.stringify(report.data.pvogService, null, 2) }}</pre>
  </details>
</template>
