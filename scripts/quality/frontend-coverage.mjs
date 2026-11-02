import { readdir, readFile, rm } from 'node:fs/promises';
import path from 'node:path';
import CoverageReport from 'monocart-coverage-reports';
import ts from 'typescript';
import { checkCoverage } from './thresholds.mjs';

const sourceRoot = 'frontend/src';
const files = (await readdir(sourceRoot, { recursive: true })).filter((file) =>
  /\.(vue|ts)$/.test(file),
);
const runtimeFiles = [];
for (const file of files) {
  if (file.endsWith('.vue')) {
    runtimeFiles.push(sourceRoot + '/' + file);
    continue;
  }
  const { outputText } = ts.transpileModule(await readFile(sourceRoot + '/' + file, 'utf8'), {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
  });
  const parsed = ts.createSourceFile(file + '.js', outputText, ts.ScriptTarget.ES2022);
  if (
    parsed.statements.some(
      (statement) =>
        !(
          ts.isExportDeclaration(statement) &&
          !statement.moduleSpecifier &&
          statement.exportClause &&
          ts.isNamedExports(statement.exportClause) &&
          statement.exportClause.elements.length === 0
        ),
    )
  )
    runtimeFiles.push(sourceRoot + '/' + file);
}
const outputDir = '.local/quality/browser-coverage';
await rm(outputDir, { recursive: true, force: true });
const report = new CoverageReport({
  name: 'FBForge Inspect — browser coverage',
  outputDir,
  reports: ['v8', 'v8-json', 'json-summary', 'console-summary'],
  entryFilter: (entry) =>
    /\/src\/.*\.(vue|ts)(?:$|\?)/.test(entry.url) && !entry.url.includes('type=style'),
  sourcePath(filePath, info) {
    const entryUrl = info?.distFile?.url || info?.url;
    if (entryUrl) {
      const match = entryUrl.match(/\/src\/([^?]+\.(?:vue|ts))(?:\?|$)/);
      if (
        match &&
        files.includes(match[1]) &&
        path.basename(match[1]) === path.posix.basename(filePath)
      )
        return sourceRoot + '/' + match[1];
    }
    const basename = path.posix.basename(filePath.replaceAll('\\', '/'));
    const matches = files.filter((file) => path.basename(file) === basename);
    if (matches.length > 1) throw new Error('Ambiguous coverage source: ' + basename);
    return matches.length === 1 ? sourceRoot + '/' + matches[0] : filePath;
  },
  sourceFilter: (file) => runtimeFiles.includes(file),
});
const rawFiles = (await readdir('.local/quality/browser-raw')).filter((file) =>
  file.endsWith('.json'),
);
if (!rawFiles.length) throw new Error('No browser coverage collected.');
for (const file of rawFiles)
  await report.add(JSON.parse(await readFile('.local/quality/browser-raw/' + file, 'utf8')));
await report.generate();
checkCoverage(
  JSON.parse(await readFile(outputDir + '/coverage-summary.json', 'utf8')),
  runtimeFiles,
);
