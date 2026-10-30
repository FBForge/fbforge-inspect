#!/usr/bin/env python3
"""Verify locally supplied official packages and inspect every XSD with the real engine.

No downloads, ZIP extraction, schema validation or external reference resolution.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
NS = {'xs': 'http://www.w3.org/2001/XMLSchema'}
MAPPING = ROOT / 'docs/pvog-field-mapping.json'
CODELISTS = ROOT / 'docs/pvog-codelists.json'


def codelist_inventory(filename):
    selected = {}
    with zipfile.ZipFile(filename) as archive:
        for name in ('transferzustaendigkeittyp', 'zustaendigkeitsrolle',
                     'kommunikationssystemtyp', 'efaparameter'):
            root = ET.fromstring(archive.read(
                f'genericode/urn-xoev-de-fim-codeliste-xzufi.{name}.xml'))
            rows = []
            for row in root.findall('.//Row'):
                rows.append({v.get('ColumnRef'): v.findtext('SimpleValue')
                             for v in row.findall('Value')
                             if v.get('ColumnRef') in ('code', 'Typ', 'type', 'Rolle', 'role',
                                                      'Parameter', 'parameter', 'cardinality', 'multilanguage')})
            selected[name] = {'uri': root.findtext('Identification/CanonicalUri'),
                              'version': root.findtext('Identification/Version'), 'entries': rows}
    return selected


def inventory(documents):
    definitions = {}
    for filename, document in documents.items():
        root = ET.fromstring(document)
        for node in root.findall('xs:complexType', NS) + root.findall('xs:simpleType', NS):
            name = node.get('name')
            bases = [e.get('base') for e in node.iter() if e.get('base')]
            fields = []
            for field in node.iter():
                kind = field.tag.rsplit('}', 1)[-1]
                if kind not in ('element', 'attribute'):
                    continue
                declaration = dict(field.attrib)
                declaration['kind'] = kind
                if kind == 'element':
                    declaration.setdefault('minOccurs', '1')
                    declaration.setdefault('maxOccurs', '1')
                else:
                    declaration.setdefault('use', 'optional')
                fields.append(declaration)
            definitions[name] = {'file': filename, 'bases': bases, 'fields': fields}
    selected = {}
    pending = ['ZustaendigkeitTransferObjekt', 'ZustaendigkeitOrganisationseinheit',
               'ZustaendigkeitOnlinedienst', 'ZustaendigkeitPerson',
               'OnlinedienstErweitert', 'Leistung']
    while pending:
        name = pending.pop()
        if name in selected or name not in definitions:
            continue
        definition = definitions[name]
        selected[name] = definition
        refs = definition['bases'] + [f.get('type', '') for f in definition['fields']]
        pending.extend(ref.split(':', 1)[1] for ref in refs if ref.startswith('xzufi:'))
    return dict(sorted(selected.items()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', action='append', required=True, metavar='VERSION=ZIP')
    parser.add_argument('--engine', required=True, type=Path)
    parser.add_argument('--java', default='java')
    parser.add_argument('--codelists', type=Path, help='Official XZuFi 2.3.1 Genericode ZIP')
    parser.add_argument('--write-mapping', action='store_true', help='Explicitly regenerate the reviewed contract inventory')
    args = parser.parse_args()
    pins = json.loads((ROOT / 'docs/source-artifacts.json').read_text())
    inventories = {}
    results = []
    for spec in args.package:
        version, filename = spec.split('=', 1)
        data = Path(filename).read_bytes()
        pin = pins['xzufi'][version]
        digest = hashlib.sha256(data).hexdigest()
        if digest != pin['sha256']:
            raise ValueError(f'{version}: package SHA-256 does not match the reviewed source')
        with zipfile.ZipFile(filename) as archive:
            documents = {}
            for entry in archive.infolist():
                if entry.filename.endswith('.xsd'):
                    if entry.file_size > 2 * 1024 * 1024:
                        raise ValueError('XSD exceeds the engine input budget')
                    documents[entry.filename] = archive.read(entry)
        if not documents:
            raise ValueError('No XSD documents in package')
        inventories[version] = inventory(documents)
        for name, document in sorted(documents.items()):
            request = {'protocolVersion': 1, 'operation': 'schema', 'environment': 'TEST',
                       'apiVersion': 'v3', 'input': {'text': document.decode('utf-8-sig')}}
            run = subprocess.run([args.java, '-jar', str(args.engine.resolve())],
                                 input=json.dumps(request), capture_output=True, text=True,
                                 encoding='utf-8', timeout=20, check=True)
            response = json.loads(run.stdout)
            if not response.get('ok'):
                raise ValueError(f'{version}/{name}: engine rejected official XSD: {response}')
            report = response['report']
            model = report['data']
            assert model['dialect'] == 'XSD', (name, model['dialect'])
            assert model['schemaVersion'] == version, (name, model['schemaVersion'])
            assert model['targetNamespace'] == pin['namespace'], name
            assert report['requests'] == [], 'Schema inspection must not fetch imports'
            assert any(f['title'] == 'Schema eingelesen' for f in report['findings']), name
            results.append({'version': version, 'file': name,
                            'nodes': len(model['structure']), 'references': len(model['references'])})
    if args.write_mapping:
        MAPPING.write_text(json.dumps(inventories, ensure_ascii=False, indent=2) + '\n')
    expected = json.loads(MAPPING.read_text())
    for version, actual in inventories.items():
        if actual != expected[version]:
            raise ValueError(f'{version}: PVOG field inventory differs from the reviewed package')
    if args.codelists:
        if hashlib.sha256(args.codelists.read_bytes()).hexdigest() != pins['xzufi231Codelists']['sha256']:
            raise ValueError('Codelist ZIP SHA-256 differs from the reviewed source')
        codes = codelist_inventory(args.codelists)
        if args.write_mapping:
            CODELISTS.write_text(json.dumps(codes, ensure_ascii=False, indent=2) + '\n')
        if codes != json.loads(CODELISTS.read_text()):
            raise ValueError('Codelist inventory differs from the reviewed package')
    print(json.dumps({'packages': list(inventories), 'schemas': results,
                      'types': {v: len(i) for v, i in inventories.items()},
                      'codelistsChecked': bool(args.codelists)}, indent=2))


if __name__ == '__main__':
    main()
