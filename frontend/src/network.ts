export interface NetworkSettings {
  mode: 'direct' | 'manual';
  proxyUrl: string;
  username: string;
  password: string;
}

/** Session memory only. Direct requests never carry dormant proxy credentials over IPC. */
export function createNetworkSettings(): NetworkSettings {
  return { mode: 'direct', proxyUrl: '', username: '', password: '' };
}

export function networkRequest(settings: NetworkSettings) {
  return settings.mode === 'direct'
    ? { mode: 'direct' as const }
    : {
        mode: 'manual' as const,
        proxyUrl: settings.proxyUrl.trim(),
        username: settings.username,
        password: settings.password,
      };
}
