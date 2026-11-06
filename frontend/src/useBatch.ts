import { reactive, ref } from 'vue';
import { invoke, isTauri } from '@tauri-apps/api/core';
import type { EngineResponse, Report } from './contracts';
import { reportForExport } from './reports';
import {
  exitCode,
  parseJob,
  type CheckInput,
  type CheckResult,
  type PortableJob,
  type PortableResult,
} from './portable';

let sequence = 100000;
export function useBatch(network: () => Record<string, unknown>) {
  const busy = ref(false),
    paused = ref(false),
    stopping = ref(false),
    message = ref('');
  const checks = ref<CheckInput[]>([]),
    results = reactive<Record<string, CheckResult>>(Object.create(null));
  const environment = ref('TEST'),
    apiVersion = ref('v3');
  const activeId = ref('');
  let nativeId = 0;
  let wake: (() => void) | undefined;
  function setJob(job: PortableJob) {
    if (busy.value) return;
    const checked = parseJob(JSON.stringify(job));
    checks.value = checked.checks;
    environment.value = checked.environment;
    apiVersion.value = checked.apiVersion;
    for (const key of Object.keys(results)) delete results[key];
    message.value = 'Prüfauftrag übernommen. Noch keine Verbindung gestartet.';
  }
  function job(): PortableJob {
    const settings = network();
    return parseJob(
      JSON.stringify({
        format: 'fbforge-inspect-job',
        formatVersion: 1,
        environment: environment.value,
        apiVersion: apiVersion.value,
        checks: checks.value,
        network: {
          mode: settings.mode,
          ...(settings.mode === 'manual' ? { proxyUrl: settings.proxyUrl } : {}),
          ...(settings.httpsCaPem ? { httpsCaPem: settings.httpsCaPem } : {}),
        },
      }),
    );
  }
  async function start(ids = checks.value.map((c) => c.id)) {
    if (busy.value || !ids.length) return;
    if (!isTauri()) {
      message.value =
        'Die Prüfung läuft in der Desktop-App. Hier können Sie Aufträge vorbereiten und Serverergebnisse öffnen.';
      return;
    }
    const work = parseJob(JSON.stringify(job())).checks.filter((c) => ids.includes(c.id));
    busy.value = true;
    stopping.value = false;
    paused.value = false;
    try {
      for (const check of work) {
        if (stopping.value) break;
        if (paused.value) {
          message.value = `Sammelprüfung pausiert. ${Object.keys(results).length} Ergebnisse verfügbar.`;
          await new Promise<void>((resolve) => {
            wake = resolve;
          });
        }
        if (stopping.value) break;
        activeId.value = check.id;
        nativeId = ++sequence;
        message.value = `${check.id} wird geprüft. ${Object.keys(results).length} Ergebnisse verfügbar.`;
        try {
          const response = await invoke<EngineResponse>('diagnose', {
            id: nativeId,
            request: {
              protocolVersion: 1,
              operation: check.operation,
              environment: environment.value,
              apiVersion: apiVersion.value,
              input: check.input,
              network: network(),
            },
          });
          if (!stopping.value) {
            const entry: CheckResult =
              response.ok && response.report
                ? { id: check.id, report: reportForExport(response.report) }
                : { id: check.id, error: response.error || 'CHECK_FAILED' };
            const retained = { ...results, [check.id]: entry };
            results[check.id] =
              new TextEncoder().encode(JSON.stringify(retained)).length > 15 * 1024 * 1024
                ? { id: check.id, error: 'RESULT_SIZE_LIMIT' }
                : entry;
          }
        } catch {
          if (!stopping.value) results[check.id] = { id: check.id, error: 'CHECK_FAILED' };
        } finally {
          activeId.value = '';
        }
      }
      message.value = stopping.value
        ? 'Sammelprüfung abgebrochen. Abgeschlossene Ergebnisse bleiben erhalten.'
        : 'Sammelprüfung abgeschlossen.';
    } finally {
      activeId.value = '';
      busy.value = false;
      paused.value = false;
      wake = undefined;
    }
  }
  function resume() {
    paused.value = false;
    wake?.();
    wake = undefined;
  }
  async function stop() {
    stopping.value = true;
    resume();
    if (activeId.value) {
      try {
        await invoke('cancel', { id: nativeId });
      } catch {
        stopping.value = false;
        message.value = 'Abbruch nicht bestätigt. Die aktuelle Prüfung wird weiter abgewartet.';
      }
    }
  }
  function result(): PortableResult {
    const entries = checks.value.map(
      (check) => results[check.id] || { id: check.id, error: 'NOT_EXECUTED' },
    );
    return {
      format: 'fbforge-inspect-result',
      formatVersion: 1,
      appVersion: '0.2.0',
      createdAt: new Date().toISOString(),
      runtime: {
        os: 'Inspect Desktop',
        architecture: 'Desktop-App',
        javaVersion: 'Gebündelte Java-Laufzeit',
      },
      checks: entries.map((entry) =>
        JSON.parse(
          JSON.stringify(entry, (key, value) => (key === 'rawResponses' ? undefined : value)),
        ),
      ),
      exitCode: exitCode(entries),
    };
  }
  function reports(): Report[] {
    return Object.values(results).flatMap((r) => (r.report ? [r.report] : []));
  }
  return {
    busy,
    paused,
    stopping,
    message,
    checks,
    results,
    environment,
    apiVersion,
    activeId,
    setJob,
    job,
    start,
    resume,
    stop,
    result,
    reports,
  };
}
