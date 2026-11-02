/** Fail closed on empty reports, omitted modules, and decreasing executable coverage. */
export function checkCoverage(report, expectedFiles, minimum = { lines: 90, branches: 80 }) {
  if (!report.total?.lines?.total || !report.total?.branches?.total)
    throw new Error('Empty coverage report.');
  for (const file of expectedFiles)
    if (!Object.hasOwn(report, file)) throw new Error('Missing coverage source: ' + file);
  for (const [metric, limit] of Object.entries(minimum)) {
    const percent = report.total[metric]?.pct;
    if (typeof percent !== 'number' || !Number.isFinite(percent) || percent < limit)
      throw new Error(`Coverage ${metric}: ${percent} below ${limit}%.`);
  }
}
