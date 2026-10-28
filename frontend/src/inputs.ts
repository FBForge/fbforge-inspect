const privateFields = new Set(['d', 'p', 'q', 'dp', 'dq', 'qi', 'oth', 'k']);
export function containsPrivateMaterial(text: string, strictKeyInput = false): boolean {
  if (text.includes('PRIVATE KEY')) return true;
  function walk(value: unknown): boolean {
    if (Array.isArray(value)) return value.some(walk);
    if (value && typeof value === 'object') {
      const object = value as Record<string, unknown>;
      if (
        (strictKeyInput || typeof object.kty === 'string') &&
        (object.kty === 'oct' || Object.keys(object).some((k) => privateFields.has(k)))
      )
        return true;
      return Object.values(object).some(walk);
    }
    return false;
  }
  try {
    return walk(JSON.parse(text));
  } catch {
    return /"(?:d|p|q|dp|dq|qi|oth|k)"\s*:/.test(text);
  }
}
