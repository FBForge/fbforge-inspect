<script setup lang="ts">
import FileButton from './FileButton.vue';
import DisclosurePanel from './DisclosurePanel.vue';
import MappingResults from './MappingResults.vue';
import { useMappingWorkspace } from './mapping/workspace';
const props = defineProps<{ currentSchema?: string }>();
const {
  inputs,
  workbook,
  excelName,
  sheetName,
  requirements,
  columns,
  pathColumns,
  error,
  message,
  processing,
  excelLoading,
  decisions,
  setResults,
  loading,
  ready,
  exportLimit,
  loadSchema,
  removeSchema,
  changeRoot,
  useCurrent,
  loadExcel,
  removeExcel,
  selectSheet,
  applyColumns,
  run,
  choose,
  skip,
  reopen,
  save,
  selectedSheet,
  cancel,
} = useMappingWorkspace(() => props.currentSchema);
function focusOpenButton(event: Event) {
  (event.currentTarget as HTMLElement)
    .closest('.mapping-schema, .mapping-excel')
    ?.querySelector<HTMLButtonElement>('button')
    ?.focus();
}
</script>
<template>
  <section class="panel mapping-inputs">
    <h2>Strukturen automatisch zuordnen</h2>
    <p>
      Öffnen Sie Quelle und Ziel. Eine Excel-Anforderungsdatei ergänzt die Vorschläge bei Bedarf.
      Die Analyse nutzt Kennungen, Bezeichnungen, Gruppenkontext und Datentypen lokal auf diesem
      Gerät.
    </p>
    <div class="mapping-schema-grid">
      <section
        v-for="item in [
          { id: 'source' as const, label: 'Quelle' },
          { id: 'target' as const, label: 'Ziel' },
        ]"
        :key="item.id"
        class="mapping-schema"
        :aria-label="`${item.label} der Zuordnung`"
      >
        <h3>{{ item.label }}</h3>
        <p class="hint">XSD, JSON Schema oder XDF. Begleitdateien gemeinsam auswählen.</p>
        <div class="action-row">
          <FileButton
            :label="`${item.label} öffnen`"
            accept=".xsd,.xml,.json,.xdf"
            multiple
            :disabled="processing"
            @change="loadSchema(item.id, $event)"
          /><button
            v-if="inputs[item.id].schema || inputs[item.id].loading"
            class="button secondary"
            :disabled="processing"
            @click="
              removeSchema(item.id);
              focusOpenButton($event);
            "
          >
            {{ item.label }} entfernen
          </button>
        </div>
        <button
          v-if="item.id === 'source' && currentSchema"
          class="button secondary"
          :disabled="loading"
          @click="useCurrent"
        >
          Aktuelles Schema als Quelle übernehmen
        </button>
        <p v-if="inputs[item.id].loading" role="status">{{ item.label }} wird eingelesen …</p>
        <template v-if="inputs[item.id].documents.length">
          <label v-if="inputs[item.id].documents.length > 1"
            >Hauptschema {{ item.label
            }}<select
              v-model="inputs[item.id].main"
              :disabled="processing"
              @change="changeRoot(item.id, true)"
            >
              <option
                v-for="document in inputs[item.id].documents"
                :key="document.name"
                :value="document.name"
              >
                {{ document.name }}
              </option>
            </select></label
          >
          <p v-else class="mono">{{ inputs[item.id].main }}</p>
          <label v-if="(inputs[item.id].schema?.roots.length || 0) > 1"
            >Wurzelelement {{ item.label
            }}<select
              v-model="inputs[item.id].root"
              :disabled="processing"
              @change="changeRoot(item.id)"
            >
              <option v-for="root in inputs[item.id].schema!.roots" :key="root">{{ root }}</option>
            </select></label
          >
        </template>
        <p v-if="inputs[item.id].schema">
          <strong>{{ inputs[item.id].schema!.fields.length }} Feldvorkommen</strong> ·
          {{ inputs[item.id].schema!.format.toUpperCase() }}
        </p>
        <ul v-if="inputs[item.id].schema?.warnings.length" class="mapping-warnings">
          <li v-for="warning in inputs[item.id].schema!.warnings" :key="warning">{{ warning }}</li>
        </ul>
      </section>
    </div>
    <div class="mapping-excel">
      <h3>Excel-Anforderungen ergänzen <span class="hint">· optional</span></h3>
      <div class="action-row">
        <FileButton
          label="Excel-Anforderungen öffnen"
          accept=".xlsx"
          :disabled="processing"
          @change="loadExcel"
        /><button
          v-if="excelName || excelLoading"
          class="button secondary"
          :disabled="processing"
          @click="
            removeExcel();
            focusOpenButton($event);
          "
        >
          Excel entfernen
        </button>
      </div>
      <p v-if="excelLoading" role="status">Arbeitsmappe wird gelesen …</p>
      <template v-if="workbook.length">
        <p class="mono">{{ excelName }}</p>
        <label
          >Tabellenblatt mit Feldanforderungen<select
            v-model="sheetName"
            :disabled="processing"
            @change="selectSheet"
          >
            <option v-for="sheet in workbook" :key="sheet.name">{{ sheet.name }}</option>
          </select></label
        >
        <p v-if="requirements">
          {{ requirements.fields.length }}
          {{ requirements.fields.length === 1 ? 'Feldanforderung' : 'Feldanforderungen' }} ·
          {{ requirements.constants.length }}
          {{
            requirements.constants.length === 1
              ? 'dokumentierte Konstante'
              : 'dokumentierte Konstanten'
          }}
        </p>
        <p v-else class="error-text">
          Excel-Spalten zuordnen oder die optionale Excel-Datei entfernen.
        </p>
        <DisclosurePanel label="Erkannte Spalten und Vorschau">
          <fieldset :disabled="processing">
            <div class="form-grid">
              <label>Spalte Feldkennung<input v-model="columns.id" maxlength="3" /></label
              ><label>Spalte Feldbezeichnung<input v-model="columns.label" maxlength="3" /></label
              ><label
                >Zielpfad-Spalten<input v-model="pathColumns" placeholder="D, E, F, G" /></label
              ><label>Spalte Datentyp<input v-model="columns.type" maxlength="3" /></label
              ><label>Spalte Anmerkung<input v-model="columns.note" maxlength="3" /></label
              ><label
                >Spalte Codelistenzuordnung<input v-model="columns.codelist" maxlength="3"
              /></label>
            </div>
            <button class="button secondary" @click="applyColumns">
              Spaltenzuordnung übernehmen
            </button>
          </fieldset>
          <div class="table-scroll" tabindex="0" role="region" aria-label="Excel-Vorschau">
            <table>
              <thead>
                <tr>
                  <th>Zeile</th>
                  <th>Inhalt</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="row in selectedSheet?.rows.slice(0, 6)" :key="row.number">
                  <td>{{ row.number }}</td>
                  <td>
                    {{
                      Object.entries(row.cells)
                        .map(([key, value]) => `${key}: ${value}`)
                        .join(' · ')
                    }}
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </DisclosurePanel>
        <ul v-if="requirements?.warnings.length" class="mapping-warnings">
          <li v-for="warning in requirements.warnings" :key="warning">{{ warning }}</li>
        </ul>
      </template>
    </div>
    <div class="action-row">
      <button class="button primary" :disabled="!ready" @click="run">
        Automatische Zuordnung starten</button
      ><button v-if="processing" class="button secondary" @click="cancel">
        Zuordnung abbrechen
      </button>
    </div>
    <p class="hint">
      Zuordnungshilfe für Strukturen und dokumentierte Anforderungen. Keine Prüfung von Fachdaten.
    </p>
  </section>
  <p v-if="error" role="alert" class="notice error-notice mapping-status">{{ error }}</p>
  <p role="status" class="mapping-status">{{ message }}</p>
  <div v-if="decisions.length && inputs.source.schema && inputs.target.schema" :ref="setResults">
    <MappingResults
      :source="inputs.source.schema"
      :target="inputs.target.schema"
      :decisions="decisions"
      @choose="choose"
      @skip="skip"
      @reopen="reopen"
    />
    <section class="panel mapping-export">
      <h2>Zuordnung weitergeben</h2>
      <div class="action-row">
        <button class="button secondary" @click="save('report')">Zuordnungsbericht speichern</button
        ><button class="button secondary" :disabled="!!exportLimit" @click="save('mapforce')">
          MapForce-Entwurf exportieren
        </button>
      </div>
      <p v-if="exportLimit" class="hint">{{ exportLimit }}</p>
      <p class="hint">
        Der MapForce-Entwurf für Version 2025 verbindet geklärte direkte XSD-Zuordnungen. Offene
        Regeln und nicht unterstützte Strukturen stehen im Bericht. Öffnen und Ausführung müssen in
        Ihrer MapForce-Installation geprüft werden.
      </p>
    </section>
  </div>
</template>
