export const XSD_NS = 'http://www.w3.org/2001/XMLSchema';
export function xmlDocument(text: string): Document {
  if (text.length > 4 * 1024 * 1024) throw new Error('XML-Datei überschreitet 4 MiB.');
  if (/<!\s*(DOCTYPE|ENTITY)\b/i.test(text))
    throw new Error('XML mit DTD oder Entitätsdeklarationen wird nicht eingelesen.');
  const doc = new DOMParser().parseFromString(text, 'application/xml');
  if (doc.getElementsByTagName('parsererror').length)
    throw new Error('XML-Syntax ungültig. Bitte die Originaldatei prüfen.');
  if (doc.getElementsByTagName('*').length > 100000)
    throw new Error('Die XML-Struktur enthält zu viele Elemente.');
  return doc;
}
export const children = (element: Element, name?: string) =>
  Array.from(element.children).filter((child) => !name || child.localName === name);
export const first = (element: Element, name: string): Element | undefined =>
  children(element, name)[0];
export const value = (element: Element, name: string) =>
  first(element, name)?.textContent?.trim() || '';
export function qname(element: Element, name: string, fallback = '') {
  const colon = name.indexOf(':');
  return {
    local: colon < 0 ? name : name.slice(colon + 1),
    namespace:
      colon < 0
        ? element.lookupNamespaceURI(null) || fallback
        : element.lookupNamespaceURI(name.slice(0, colon)) || '',
  };
}
export function escapeXml(value: string) {
  return value.replace(
    /[&<>"']/g,
    (char) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&apos;' })[char]!,
  );
}
