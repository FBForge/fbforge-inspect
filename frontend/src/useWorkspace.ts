import { computed, reactive, ref, type Ref } from 'vue';
import type { Page } from './contracts';
import { createSession } from './session.ts';

/** In-memory work is isolated by environment, API, task and schema reference. */
export function useWorkspace(page: Ref<Page>, environment: Ref<string>, apiVersion: Ref<string>) {
  const sessions = reactive<Record<string, ReturnType<typeof createSession>>>({});

  const processMode = ref<'process' | 'limits'>('process');
  const schemaHistory = reactive<Record<string, { keys: string[]; index: number }>>({});
  let schemaSequence = 0;
  const schemaContext = computed(() => `${environment.value}/${apiVersion.value}/schema`);
  const schemaTrail = computed(() => {
    if (!schemaHistory[schemaContext.value])
      schemaHistory[schemaContext.value] = { keys: [schemaContext.value], index: 0 };
    return schemaHistory[schemaContext.value]!;
  });
  const sessionKey = computed(() =>
    page.value === 'schema'
      ? schemaTrail.value.keys[schemaTrail.value.index]!
      : `${environment.value}/${apiVersion.value}/${page.value}${page.value === 'process' && processMode.value === 'limits' ? '/limits' : ''}`,
  );
  const session = computed(() => {
    if (!sessions[sessionKey.value]) sessions[sessionKey.value] = createSession();
    return sessions[sessionKey.value]!;
  });
  const report = computed({
    get: () => session.value.report,
    set: (value) => {
      session.value.report = value;
    },
  });
  const error = computed({
    get: () => session.value.error,
    set: (value) => {
      session.value.error = value;
    },
  });
  const status = computed({
    get: () => session.value.status,
    set: (value) => {
      session.value.status = value;
    },
  });

  function prepareSchemaReference() {
    const trail = schemaTrail.value;
    const existing = sessions[trail.keys[trail.index]!];
    if (existing?.report || existing?.form.text || existing?.form.url || trail.keys.length > 1) {
      trail.keys.push(`${schemaContext.value}/reference-${++schemaSequence}`);
      trail.index = trail.keys.length - 1;
    }
  }
  function moveSchema(offset: number) {
    const next = schemaTrail.value.index + offset;
    if (next >= 0 && next < schemaTrail.value.keys.length) schemaTrail.value.index = next;
  }
  return {
    sessions,
    processMode,
    schemaTrail,
    sessionKey,
    session,
    report,
    error,
    status,
    prepareSchemaReference,
    moveSchema,
  };
}
