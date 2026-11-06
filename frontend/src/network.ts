export interface NetworkSettings {
  mode: 'direct' | 'manual';
  proxyUrl: string;
  username: string;
  password: string;
  httpsCaPem: string;
}

/** Session memory only. Direct requests never carry dormant proxy credentials over IPC. */
export function createNetworkSettings(): NetworkSettings {
  return { mode: 'direct', proxyUrl: '', username: '', password: '', httpsCaPem: '' };
}

export function networkRequest(settings: NetworkSettings) {
  const trust = settings.httpsCaPem ? { httpsCaPem: settings.httpsCaPem } : {};
  return settings.mode === 'direct'
    ? { mode: 'direct' as const, ...trust }
    : {
        mode: 'manual' as const,
        proxyUrl: settings.proxyUrl.trim(),
        username: settings.username,
        password: settings.password,
        ...trust,
      };
}
