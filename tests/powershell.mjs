/** An indirect pwsh -> Node -> Windows PowerShell launch must not inherit pwsh's module path. */
export function powershellRuntime() {
  const executable =
    process.platform === 'win32' ? 'powershell.exe' : process.env.INSPECT_PWSH_EXECUTABLE || 'pwsh';
  const environment = { ...process.env };
  if (process.platform === 'win32') {
    for (const key of Object.keys(environment))
      if (key.toUpperCase() === 'PSMODULEPATH') delete environment[key];
  }
  return { executable, environment };
}
