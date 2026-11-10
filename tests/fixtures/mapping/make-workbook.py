"""Synthetic requirements only. Recreate standard OOXML independently of the app ZIP code."""
from pathlib import Path
from xml.sax.saxutils import escape
import zipfile

ROOT = Path(__file__).parent
NS = 'http://schemas.openxmlformats.org/spreadsheetml/2006/main'
ROWS = [
    ['Nr.', 'ID', 'Formularinhalt', 'Zielpfad Nachricht', 'Ziel Ebene 1', 'Ziel Ebene 2', 'Typ', 'Codelisten-Zuordnung', 'Anmerkung'],
    ['1', 'G90000001V1.0', 'Person'],
    ['2', 'F90000001V1.0 (1:1)', 'Familienname', 'Ziel', 'Person', 'Nachname', 'string'],
    ['3', 'F90000002V1.0 (1:1)', 'Straße', 'Ziel', 'Person', 'Strasse', 'string'],
    ['4', 'F90000003V1.0 (1:1)', 'Hausnummer', 'Ziel', 'Person', 'Strasse', 'string'],
    ['5', 'F90000004V1.0 (1:1)', 'Geburtsdatum', 'Ziel', 'Person', 'Geburtsdatum', 'date'],
    ['6', 'F90000005V1.0 (0:1)', 'E-Mail', 'Ziel', 'Kontakt', 'Email', 'string'],
]


def sheet(rows):
    data = []
    for number, values in enumerate(rows, 1):
        cells = ''.join(f'<c r="{chr(65 + index)}{number}" t="inlineStr"><is><t>{escape(value)}</t></is></c>'
                        for index, value in enumerate(values) if value)
        data.append(f'<row r="{number}">{cells}</row>')
    return f'<worksheet xmlns="{NS}"><sheetData>{"".join(data)}</sheetData></worksheet>'


for method, name in [(zipfile.ZIP_DEFLATED, 'requirements.xlsx'), (zipfile.ZIP_STORED, 'requirements-stored.xlsx')]:
    with zipfile.ZipFile(ROOT / name, 'w', compression=method) as archive:
        archive.writestr('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/></Types>')
        archive.writestr('xl/workbook.xml', f'<workbook xmlns="{NS}" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Fiktive Anforderungen" sheetId="1" r:id="rId1"/><sheet name="Konstante Werte" sheetId="2" r:id="rId2"/></sheets></workbook>')
        archive.writestr('xl/_rels/workbook.xml.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/></Relationships>')
        archive.writestr('xl/worksheets/sheet1.xml', sheet(ROWS))
        archive.writestr('xl/worksheets/sheet2.xml', sheet([['Nr.', 'Übermittelter Inhalt', 'Abbildung Ebene 1', 'Abbildung Ebene 2'], ['1', 'FIKTIV', 'Ziel', 'Kategorie']]))
