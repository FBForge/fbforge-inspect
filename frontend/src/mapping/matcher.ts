import type {
  Candidate,
  MappingDecision,
  MappingField,
  MappingSchema,
  Requirement,
  Requirements,
} from './types';

const aliases: Record<string, string> = {
  familienname: 'nachname',
  surname: 'nachname',
  lastname: 'nachname',
  firstname: 'vorname',
  vornamen: 'vorname',
  forename: 'vorname',
  geburtsdatum: 'geburtstag',
  dateofbirth: 'geburtstag',
  birthdate: 'geburtstag',
  zipcode: 'postleitzahl',
  postalcode: 'postleitzahl',
  plz: 'postleitzahl',
  street: 'strasse',
  city: 'ort',
  emailadresse: 'email',
  mail: 'email',
  telephone: 'telefon',
  phonenumber: 'telefon',
  country: 'staat',
};
export function normalize(text: string) {
  const plain = text
    .replace(/([a-zäöüß])([A-ZÄÖÜ])/g, '$1 $2')
    .toLocaleLowerCase('de')
    .replace(/ß/g, 'ss')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .replace(/[^a-z0-9]+/g, '');
  return aliases[plain] || plain;
}
export function tokens(text: string) {
  return new Set(
    text
      .replace(/([a-zäöüß])([A-ZÄÖÜ])/g, '$1 $2')
      .split(/[^a-zA-ZäöüÄÖÜß0-9]+/)
      .map(normalize)
      .filter((t) => t.length > 2),
  );
}
function overlap(a: string, b: string) {
  const left = tokens(a),
    right = tokens(b);
  if (!left.size || !right.size) return 0;
  return [...left].filter((t) => right.has(t)).length / Math.max(left.size, right.size);
}
const baseId = (text: string) => text.match(/F\d{5,}/i)?.[0]?.toUpperCase() || '';
function labelScore(a: string, b: string) {
  if (normalize(a) === normalize(b)) return 1;
  return overlap(a, b);
}
function relevant(source: MappingField, requirement: Requirement) {
  const id = baseId(requirement.id);
  if (id && source.identifiers.some((v) => baseId(v) === id)) return true;
  return (
    labelScore(source.label, requirement.label) === 1 ||
    source.names.some((n) => baseId(n) && baseId(n) === id)
  );
}
function contextScore(field: MappingField, requirement: Requirement) {
  const context = [...field.path.slice(0, -1), field.description].join(' ');
  return overlap(context, [requirement.group, ...requirement.path.slice(1, -1)].join(' '));
}
function requirementMatch(source: MappingField, requirements: Requirements | null) {
  const rows = requirements?.fields.filter((r) => relevant(source, r)) || [];
  if (rows.length < 2) return rows;
  const ranked = rows
    .map((row) => ({ row, score: contextScore(source, row) }))
    .sort((a, b) => b.score - a.score);
  const best = ranked[0]!.score;
  return ranked.filter((r) => best === 0 || r.score >= best - 0.08).map((r) => r.row);
}
function pathMatch(field: MappingField, path: string[]) {
  const requested = path.map((part) => part.replace(/^@/, '').replace(/^[a-zA-Z_][\w.-]*:/, ''));
  if (!requested.length || requested.some((part) => !part || part === '…' || part === '...'))
    return false;
  return (
    field.path.join('/') === requested.join('/') ||
    (requested.length === field.path.length - 1 &&
      field.path.slice(1).join('/') === requested.join('/'))
  );
}
export function typeFamily(type: string) {
  const name = type.replace(/^.*:/, '').toLocaleLowerCase('de');
  if (
    /^(string|text|token|normalizedstring|anyuri|datatype[abcde]|allgemeinername|id|idref)$/.test(
      name,
    )
  )
    return 'text';
  if (
    /^(int|integer|long|short|byte|nonnegativeinteger|positiveinteger|unsigned.*|ganzzahl)$/.test(
      name,
    )
  )
    return 'integer';
  if (/^(number|decimal|double|float|zahl|num)$/.test(name)) return 'number';
  if (/^(bool|boolean|wahrheitswert)$/.test(name)) return 'boolean';
  if (/^(date|datum|stichtag)$/.test(name)) return 'date';
  if (/^(datetime|zeitpunkt)$/.test(name)) return 'datetime';
  if (/^(time|uhrzeit)$/.test(name)) return 'time';
  return 'unknown';
}
export function compatibility(source: MappingField, target: MappingField) {
  const warnings: string[] = [],
    a = typeFamily(source.type),
    b = typeFamily(target.type);
  if (a === 'unknown' || b === 'unknown')
    warnings.push('Mindestens ein Datentyp ist nicht vollständig bekannt.');
  else if (a !== b && !(a === 'integer' && b === 'number'))
    warnings.push(`Typumwandlung erforderlich: ${source.type} → ${target.type}.`);
  if (source.repeated && !target.repeated)
    warnings.push('Mehrere Quellwerte treffen auf ein einzelnes Zielfeld.');
  if (!source.required && target.required)
    warnings.push('Optionales Quellfeld trifft auf ein erforderliches Zielfeld.');
  return [...warnings, ...source.warnings, ...target.warnings];
}
function scoreCandidate(
  source: MappingField,
  target: MappingField,
  rows: Requirement[],
  incomplete: boolean,
): Candidate | null {
  const evidence = rows.filter((row) => pathMatch(target, row.path));
  const reasons: string[] = [],
    warnings = compatibility(source, target);
  const roles = (field: MappingField) => tokens(field.path.slice(0, -1).join(' '));
  const roleNames = ['antragsteller', 'verstorbener', 'mutter', 'vater', 'partner', 'kind'];
  const sourceRoles = roleNames.filter((r) => roles(source).has(r)),
    targetRoles = roleNames.filter((r) => roles(target).has(r));
  if (sourceRoles.length && targetRoles.length && !sourceRoles.some((r) => targetRoles.includes(r)))
    warnings.push('Unterschiedliche Personenrollen im Gruppenpfad. Zuordnung ausdrücklich prüfen.');
  let score = 0;
  if (evidence.length) {
    score = 0.99;
    reasons.push('Zielpfad ist in den Excel-Anforderungen dokumentiert.');
    if (new Set(rows.map((r) => r.path.map(normalize).join('/'))).size > 1)
      warnings.push('Die Excel nennt mehrere Zielkontexte für diese Quellkennung.');
    if (evidence.some((r) => r.codelist.length || r.note))
      warnings.push('Codeliste oder fachliche Anmerkung aus Excel prüfen.');
    if (
      evidence.some((r) => {
        const documented = typeFamily(r.type.replace(/\s*\([^)]*\)\s*$/, '').trim()),
          actual = typeFamily(target.type);
        return documented !== 'unknown' && actual !== 'unknown' && documented !== actual;
      })
    )
      warnings.push('Der dokumentierte Excel-Datentyp weicht vom Zieltyp ab.');
  }
  const sameId = source.identifiers.some(
    (id) => !!baseId(id) && target.identifiers.some((other) => baseId(other) === baseId(id)),
  );
  if (sameId) {
    score = Math.max(score, 0.98);
    reasons.push('Übereinstimmende Feldkennung.');
  }
  const name = labelScore(source.label, target.label),
    technical = labelScore(source.names.at(-1)!, target.names.at(-1)!);
  const context = overlap(source.path.slice(0, -1).join(' '), target.path.slice(0, -1).join(' '));
  if (name === 1 || technical === 1) {
    score = Math.max(score, 0.94 + context * 0.04);
    reasons.push('Passende Feldbezeichnung oder technischer Feldname.');
  } else {
    const combined =
      Math.max(name, technical) * 0.65 +
      overlap(source.description, target.description) * 0.2 +
      context * 0.1;
    if (combined > score) {
      score = combined;
      reasons.push('Ähnliche Bezeichnung und Struktur.');
    }
  }
  if (rows.length && !evidence.length) {
    score = Math.min(score, 0.79);
    reasons.push(
      'Der dokumentierte Excel-Zielpfad passt zu einem anderen oder noch nicht aufgelösten Ziel.',
    );
  }
  if (score < 0.25) return null;
  if (context > 0) reasons.push('Gruppenkontext berücksichtigt.');
  if (incomplete)
    warnings.push('Mindestens ein Schema enthält ungeklärte Referenzen oder Strukturen.');
  return { target: target.key, score, reasons, warnings: [...new Set(warnings)], evidence };
}
export async function suggestMappings(
  source: MappingSchema,
  target: MappingSchema,
  requirements: Requirements | null,
  signal?: AbortSignal,
): Promise<MappingDecision[]> {
  if (source.fields.length * target.fields.length > 4000000)
    throw new Error('Zu viele Feldkombinationen. Bitte kleinere Teilstrukturen auswählen.');
  const decisions: MappingDecision[] = [];
  for (const [i, field] of source.fields.entries()) {
    if (signal?.aborted) throw new Error('Zuordnung abgebrochen.');
    if (i % 8 === 0) await new Promise((resolve) => setTimeout(resolve, 0));
    const rows = requirementMatch(field, requirements);
    const candidates = target.fields
      .map((t) =>
        scoreCandidate(field, t, rows, !!source.warnings.length || !!target.warnings.length),
      )
      .filter((c): c is Candidate => !!c)
      .sort((a, b) => b.score - a.score || a.target.localeCompare(b.target))
      .slice(0, 6);
    if (requirements?.namespace && requirements.namespace !== target.namespace)
      for (const candidate of candidates)
        if (candidate.evidence.length)
          candidate.warnings.push(
            'Der dokumentierte Excel-Namespace passt nicht zum ausgewählten Zielschema.',
          );
    const best = candidates[0],
      clear =
        best &&
        best.score >= 0.94 &&
        (!candidates[1] || best.score - candidates[1].score >= 0.1) &&
        !best.warnings.length;
    decisions.push({
      source: field.key,
      target: best?.target || '',
      state: clear ? 'automatic' : 'review',
      candidates,
      reasons: best?.reasons || ['Kein belastbarer Zielvorschlag gefunden.'],
      warnings: best?.warnings || [],
    });
  }
  return markConflicts(decisions);
}
export function markConflicts(decisions: MappingDecision[]) {
  const warning =
    'Mehrere Quellfelder zeigen auf dasselbe Ziel. Zusammenführung oder Auswahlregel festlegen.';
  const uses = new Map<string, MappingDecision[]>();
  for (const decision of decisions) {
    decision.warnings = decision.warnings.filter((w) => w !== warning);
    if (decision.target && decision.state !== 'skipped')
      uses.set(decision.target, [...(uses.get(decision.target) || []), decision]);
  }
  for (const group of uses.values())
    if (group.length > 1)
      for (const decision of group) {
        decision.warnings.push(warning);
        if (decision.state === 'automatic') decision.state = 'review';
      }
  return decisions;
}
export function chooseMapping(
  decision: MappingDecision,
  target: MappingField,
  source: MappingField,
) {
  decision.target = target.key;
  decision.state = 'confirmed';
  decision.reasons = ['Zielfeld ausdrücklich ausgewählt und bestätigt.'];
  decision.warnings = [
    ...new Set([
      ...compatibility(source, target),
      ...(decision.candidates.find((c) => c.target === target.key)?.warnings || []),
    ]),
  ];
}
