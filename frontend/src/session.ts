import type { Report } from './contracts';

export function createSession() {
  return {
    report: null as Report | null,
    error: '',
    status: 'Bereit',
    scrollTop: 0,
    view: {
      fileName: '',
      fileText: '',
      inputMode: 'text' as 'url' | 'text' | 'file',
      expectedOpen: false,
      certificatesOpen: false,
      areaSearchOpen: false,
    },
    form: {
      destinationId: '',
      service: '',
      region: '',
      regionKind: 'ars',
      schema: '',
      mime: '',
      metadata: '',
      compare: false,
      revocation: true,
      trustPem: '',
      processModelId: '',
      messageId: '',
      fromDestinationId: '',
      toDestinationId: '',
      text: '',
      url: '',
      areaSearch: '',
      pvog: true,
      lbId: '',
    },
  };
}
export type DiagnosisDraft = ReturnType<typeof createSession>['form'];
export type DiagnosisView = ReturnType<typeof createSession>['view'];
