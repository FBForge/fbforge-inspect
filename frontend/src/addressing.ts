import type { Report } from './contracts';
import type { PvogParent, PvogRelation, AddressingSignature } from './reportData';
import { records, identifier, codeLabel, routingRows } from './resultValues.ts';

/** Each row represents one source relation. Equal destination IDs never merge authorities. */
export function addressingContexts(report: Report) {
  const signatures = records<AddressingSignature & { contextMatches?: boolean }>(
    report.data.addressingSignatures,
  );
  const routes = routingRows(report.data);
  return records<PvogParent>(report.data.pvogRelations).flatMap((parent, parentIndex) =>
    records<PvogRelation>(parent.zustaendigkeit).map((relation, index) => {
      const path = `$.pvogRelations[${parentIndex}].zustaendigkeit[${index}]`;
      const matches = signatures.filter((signature) => signature.context?.startsWith(path + '.'));
      const witnesses = report.findings.filter((finding) => finding.path.startsWith(path + '.'));
      return {
        path,
        parent: identifier(parent.uebergeordnetesObjektID),
        parentClass: codeLabel(parent.uebergeordnetesObjektKlasse || {}),
        role: records(Array.isArray(relation.rolle) ? relation.rolle : [relation.rolle])
          .map(codeLabel)
          .join(', '),
        services: records(relation.leistungID).map(identifier),
        areas: records(relation.gebietID).map(identifier),
        source: witnesses.find((finding) => finding.source)?.source || '',
        addresses: matches.map((signature) => ({
          id: signature.claims?.destinationId || '',
          verified: signature.verified === true,
          contextMatches: signature.contextMatches === true,
          inRouting: routes.some(
            (route) => route.destinationId === signature.claims?.destinationId,
          ),
          path: signature.context || '',
        })),
        openPoints: witnesses.filter((finding) =>
          ['error', 'unknown', 'warning'].includes(finding.status),
        ),
      };
    }),
  );
}
