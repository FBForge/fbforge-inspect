import type { Report } from './contracts';

export function createSession() {
  return {
    report: null as Report | null,
    error: '',
    status: 'Bereit',
    form: {
      destinationId: '',
      service: '',
      region: '',
      regionKind: 'ars',
      schema: '',
      mime: '',
      metadata: '',
      compare: false,
      revocation: false,
      trustPem: '',
      processModelId: '',
      messageId: '',
      fromDestinationId: '',
      toDestinationId: '',
      text: '',
      url: '',
      areaSearch: '',
      pvog: false,
      lbId: '',
    },
  };
}
export type DiagnosisDraft = ReturnType<typeof createSession>['form'];
