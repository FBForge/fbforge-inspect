import { computed, nextTick, reactive, ref, watch } from 'vue';
import { readSchema } from './schema';
import {
  readWorkbook,
  detectColumns,
  requirementsFromSheet,
  selectRequirementSheet,
} from './requirements';
import { chooseMapping, markConflicts, suggestMappings } from './matcher';
import { mapforceFiles, mapforceLimit, mappingReport, saveMappingArchive } from './export';
import { saveFile } from '../saveFile';
import type {
  MappingDecision,
  MappingSchema,
  RequirementColumns,
  Requirements,
  SchemaDocument,
  WorkbookSheet,
} from './types';
export function useMappingWorkspace(currentSchema: () => string | undefined) {
  const side = () => ({
    schema: null as MappingSchema | null,
    documents: [] as SchemaDocument[],
    main: '',
    root: '',
    sequence: 0,
    loading: false,
  });
  const inputs = reactive({ source: side(), target: side() });
  const workbook = ref<WorkbookSheet[]>([]),
    excelName = ref(''),
    sheetName = ref('');
  const requirements = ref<Requirements | null>(null);
  const columns = ref<RequirementColumns>({
    id: '',
    label: '',
    path: [],
    type: '',
    note: '',
    codelist: '',
  });
  const pathColumns = ref('');
  const error = ref(''),
    message = ref(''),
    processing = ref(false),
    excelLoading = ref(false);
  const decisions = ref<MappingDecision[]>([]);
  const results = ref<HTMLElement>();
  let excelSequence = 0,
    controller: AbortController | null = null;
  const loading = computed(
    () => processing.value || excelLoading.value || inputs.source.loading || inputs.target.loading,
  );
  const ready = computed(
    () =>
      !!inputs.source.schema &&
      !!inputs.target.schema &&
      (!workbook.value.length || !!requirements.value) &&
      !loading.value,
  );
  const exportLimit = computed(() =>
    inputs.source.schema && inputs.target.schema
      ? mapforceLimit(inputs.source.schema, inputs.target.schema)
      : '',
  );
  watch(
    [columns, pathColumns],
    () => {
      if (requirements.value) {
        requirements.value = null;
        invalidate();
      }
    },
    { deep: true, flush: 'sync' },
  );
  function invalidate() {
    decisions.value = [];
    message.value = 'Eingaben geändert. Automatische Zuordnung erneut starten.';
  }
  async function loadSchema(kind: 'source' | 'target', event: Event) {
    const input = event.target as HTMLInputElement,
      files = Array.from(input.files || []);
    input.value = '';
    if (!files.length) return;
    const state = inputs[kind],
      sequence = ++state.sequence;
    state.loading = true;
    state.schema = null;
    state.documents = [];
    invalidate();
    error.value = '';
    try {
      if (
        files.length > 32 ||
        files.some((f) => f.size > 4 * 1024 * 1024) ||
        files.reduce((sum, f) => sum + f.size, 0) > 16 * 1024 * 1024
      )
        throw new Error(
          'Bis zu 32 Dateien mit höchstens 4 MiB pro Datei und zusammen 16 MiB auswählen.',
        );
      const documents = await Promise.all(
        files.map(async (file) => ({ name: file.name, text: await file.text() })),
      );
      if (sequence !== state.sequence) return;
      const schema = readSchema(documents);
      state.documents = documents;
      state.main = schema.name;
      state.root = schema.root;
      state.schema = schema;
      message.value = `${kind === 'source' ? 'Quelle' : 'Ziel'} eingelesen: ${schema.fields.length} Feldvorkommen.`;
    } catch (e) {
      if (sequence === state.sequence)
        error.value = e instanceof Error ? e.message : 'Schema konnte nicht gelesen werden.';
    } finally {
      if (sequence === state.sequence) state.loading = false;
    }
  }
  function removeSchema(kind: 'source' | 'target') {
    const state = inputs[kind];
    state.sequence++;
    state.schema = null;
    state.documents = [];
    state.main = '';
    state.root = '';
    state.loading = false;
    invalidate();
    error.value = '';
  }
  function changeRoot(kind: 'source' | 'target', mainChanged = false) {
    const state = inputs[kind];
    invalidate();
    error.value = '';
    try {
      state.schema = readSchema(state.documents, mainChanged ? '' : state.root, state.main);
      state.root = state.schema.root;
    } catch (e) {
      state.schema = null;
      error.value = e instanceof Error ? e.message : 'Schemaauswahl ungültig.';
    }
  }
  function useCurrent() {
    const text = currentSchema();
    if (!text) return;
    removeSchema('source');
    try {
      const documents = [
        {
          name: text.trimStart().startsWith('<') ? 'aktuelles-schema.xml' : 'aktuelles-schema.json',
          text,
        },
      ];
      const schema = readSchema(documents);
      Object.assign(inputs.source, { documents, schema, main: schema.name, root: schema.root });
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Aktuelles Schema nicht nutzbar.';
    }
  }
  async function loadExcel(event: Event) {
    const input = event.target as HTMLInputElement,
      file = input.files?.[0];
    input.value = '';
    if (!file) return;
    const sequence = ++excelSequence;
    excelLoading.value = true;
    workbook.value = [];
    requirements.value = null;
    excelName.value = '';
    invalidate();
    error.value = '';
    try {
      if (!/\.xlsx$/i.test(file.name) || file.size > 8 * 1024 * 1024)
        throw new Error('Eine XLSX-Datei mit höchstens 8 MiB auswählen.');
      const sheets = await readWorkbook(new Uint8Array(await file.arrayBuffer()));
      if (sequence !== excelSequence) return;
      workbook.value = sheets;
      excelName.value = file.name;
      sheetName.value = selectRequirementSheet(sheets);
      selectSheet();
    } catch (e) {
      if (sequence === excelSequence)
        error.value = e instanceof Error ? e.message : 'Excel-Datei konnte nicht gelesen werden.';
    } finally {
      if (sequence === excelSequence) excelLoading.value = false;
    }
  }
  function removeExcel() {
    excelSequence++;
    workbook.value = [];
    requirements.value = null;
    excelName.value = '';
    sheetName.value = '';
    excelLoading.value = false;
    invalidate();
    error.value = '';
  }
  function selectSheet() {
    const sheet = workbook.value.find((s) => s.name === sheetName.value);
    if (!sheet) return;
    columns.value = detectColumns(sheet);
    pathColumns.value = columns.value.path.join(', ');
    applyColumns();
  }
  function applyColumns() {
    invalidate();
    requirements.value = null;
    error.value = '';
    try {
      for (const key of ['id', 'label', 'type', 'note', 'codelist'] as const)
        columns.value[key] = columns.value[key].trim().toUpperCase();
      columns.value.path = [
        ...new Set(
          pathColumns.value
            .toUpperCase()
            .split(/[,\s]+/)
            .filter(Boolean),
        ),
      ];
      if (
        ![
          columns.value.id,
          columns.value.label,
          ...columns.value.path,
          ...[columns.value.type, columns.value.note, columns.value.codelist].filter(Boolean),
        ].every((c) => /^[A-Z]{1,3}$/.test(c))
      )
        throw new Error('Spalten als Excel-Buchstaben angeben, zum Beispiel B oder D, E, F.');
      requirements.value = requirementsFromSheet(
        excelName.value,
        workbook.value,
        sheetName.value,
        columns.value,
      );
      message.value = `${requirements.value.fields.length} Feldanforderungen aus Excel erkannt.`;
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Spaltenzuordnung ungültig.';
    }
  }
  async function run() {
    if (!ready.value) return;
    processing.value = true;
    error.value = '';
    decisions.value = [];
    controller = new AbortController();
    message.value = 'Felder werden lokal zugeordnet …';
    try {
      const result = await suggestMappings(
        inputs.source.schema!,
        inputs.target.schema!,
        requirements.value,
        controller.signal,
      );
      if (controller.signal.aborted) throw new Error('Zuordnung abgebrochen.');
      decisions.value = result;
      message.value = `${result.length} Feldvorkommen analysiert.`;
      await nextTick();
      results.value?.scrollIntoView({ block: 'start' });
      results.value?.querySelector<HTMLElement>('h2')?.focus({ preventScroll: true });
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Zuordnung fehlgeschlagen.';
      message.value = 'Keine neuen Zuordnungsergebnisse.';
    } finally {
      processing.value = false;
      controller = null;
    }
  }
  function choose(sourceKey: string, targetKey: string) {
    const d = decisions.value.find((d) => d.source === sourceKey),
      s = inputs.source.schema?.fields.find((f) => f.key === sourceKey),
      t = inputs.target.schema?.fields.find((f) => f.key === targetKey);
    if (!d || !s || !t) return;
    chooseMapping(d, t, s);
    markConflicts(decisions.value);
    message.value = `Zuordnung für ${s.label} bestätigt.`;
  }
  function skip(key: string) {
    const d = decisions.value.find((d) => d.source === key);
    if (!d) return;
    d.state = 'skipped';
    d.target = '';
    d.warnings = [];
    markConflicts(decisions.value);
    message.value = 'Feld wird nicht zugeordnet.';
  }
  function reopen(key: string) {
    const d = decisions.value.find((d) => d.source === key);
    if (!d) return;
    d.state = 'review';
    d.target = d.candidates[0]?.target || '';
    d.reasons = d.candidates[0]?.reasons || [];
    d.warnings = d.candidates[0]?.warnings || [];
    markConflicts(decisions.value);
    message.value = 'Zuordnung erneut zur Entscheidung geöffnet.';
  }
  async function save(kind: 'report' | 'mapforce') {
    if (!inputs.source.schema || !inputs.target.schema || !decisions.value.length) return;
    error.value = '';
    try {
      const ok =
        kind === 'report'
          ? await saveFile(
              JSON.stringify(
                mappingReport(
                  inputs.source.schema,
                  inputs.target.schema,
                  requirements.value,
                  decisions.value,
                ),
                null,
                2,
              ),
            )
          : await saveMappingArchive(
              mapforceFiles(
                inputs.source.schema,
                inputs.target.schema,
                requirements.value,
                decisions.value,
              ),
            );
      message.value = ok
        ? kind === 'report'
          ? 'Zuordnungsbericht gespeichert.'
          : 'MapForce-Entwurf als ZIP gespeichert.'
        : 'Speichern abgebrochen.';
    } catch (e) {
      error.value = e instanceof Error ? e.message : 'Export konnte nicht gespeichert werden.';
    }
  }
  const selectedSheet = computed(() => workbook.value.find((s) => s.name === sheetName.value));

  function cancel() {
    controller?.abort();
  }
  function setResults(element: unknown) {
    results.value = element instanceof HTMLElement ? element : undefined;
  }
  return {
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
  };
}
