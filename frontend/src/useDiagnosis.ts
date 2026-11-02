import { nextTick, ref, type Ref, type ComputedRef } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { EngineResponse } from './contracts';
import type { createSession } from './session';
import { containsPrivateMaterial } from './inputs';

/** Native request lifecycle; retained work and presentation are supplied by the caller. */
export function useDiagnosis(
  session: ComputedRef<ReturnType<typeof createSession>>,
  environment: Ref<string>,
  apiVersion: Ref<string>,
  onClear: () => void,
  network?: () => Record<string, unknown>,
) {
  const busy = ref(false);
  let requestId = 0;
  let active: { id: number; target: ReturnType<typeof createSession>; cancelled: boolean } | null =
    null;
  function clear() {
    session.value.report = null;
    session.value.error = '';
    onClear();
    session.value.status = 'Bereit';
  }
  async function run(operation: string, input: Record<string, unknown>) {
    if (busy.value) return;
    clear();
    const target = session.value;
    if (!isTauri()) {
      session.value.error =
        'Die Live-Diagnose läuft in der Desktop-App. Diese Browseransicht zeigt die Oberfläche.';
      return;
    }
    // Reject private material before it crosses IPC. Do not retain an error copy of the input.
    if (
      containsPrivateMaterial(String(input.text), operation === 'keys') ||
      containsPrivateMaterial(String(input.trustPem), true)
    ) {
      session.value.error = 'Private oder symmetrische Schlüssel sind nicht zulässig.';
      return;
    }
    busy.value = true;
    session.value.status = 'Öffentliche Daten werden geprüft …';
    const id = ++requestId;
    const request = { id, target, cancelled: false };
    active = request;
    try {
      const response = await invoke<EngineResponse>('diagnose', {
        id,
        request: {
          protocolVersion: 1,
          operation,
          environment: environment.value,
          apiVersion: apiVersion.value,
          input,
          ...(network ? { network: network() } : {}),
        },
      });
      if (active !== request || request.cancelled) return;
      if (!response.ok || !response.report)
        throw new Error(response.error || 'Keine Diagnoseantwort.');
      target.error = '';
      target.report = response.report;
      target.status = 'Prüfung abgeschlossen';
      await nextTick();
      document
        .querySelector<HTMLElement>('.results')
        ?.scrollIntoView({ block: 'start', behavior: 'instant' });
    } catch (e) {
      if (active === request && !request.cancelled) {
        target.error = e instanceof Error ? e.message : String(e);
        target.status = 'Prüfung beendet';
      }
    } finally {
      if (active === request) {
        if (request.cancelled) target.status = 'Prüfung abgebrochen';
        active = null;
        busy.value = false;
      }
    }
  }
  async function cancel() {
    const request = active;
    if (!request || request.cancelled) return;
    request.cancelled = true;
    request.target.status = 'Prüfung wird abgebrochen …';
    try {
      await invoke('cancel', { id: request.id });
    } catch {
      if (active === request) {
        request.cancelled = false;
        request.target.error = 'Prüfung konnte nicht abgebrochen werden.';
        request.target.status = 'Abbruch nicht bestätigt';
      }
    }
  }

  return { busy, clear, run, cancel };
}
