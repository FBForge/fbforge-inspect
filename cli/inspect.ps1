$ErrorActionPreference = 'Stop'
$engine = Join-Path $PSScriptRoot 'inspect-engine.jar'
$inspectJava = 'java'
if (-not (Test-Path -LiteralPath $engine -PathType Leaf)) {
    $engine = Join-Path (Split-Path -Parent $PSScriptRoot) 'engine\inspect-engine.jar'
    $bundled = Join-Path (Split-Path -Parent $PSScriptRoot) 'engine\runtime\bin\java.exe'
    if (Test-Path -LiteralPath $bundled -PathType Leaf) { $inspectJava = $bundled }
}
& $inspectJava '-Xmx256m' '-Dfile.encoding=UTF-8' '-Dcom.sun.security.enableAIAcaIssuers=false' '-jar' $engine @args
exit $LASTEXITCODE
