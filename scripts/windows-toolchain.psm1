# Windows JDK provenance and checksum-pinned Maven bootstrap, shared by local and CI builds.
$ErrorActionPreference = 'Stop'

function Get-InspectMaven {
    param([string]$ToolDirectory = (Join-Path $env:LOCALAPPDATA 'FBForge\BuildTools'))
    $installed = Get-Command mvn.cmd -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($installed) { return $installed.Source }
    $version = '3.9.11'
    $expectedHash = '03e2d65d4483a3396980629f260e25cac0d8b6f7f2791e4dc20bc83f9514db8d0f05b0479e699a5f34679250c49c8e52e961262ded468a20de0be254d8207076'
    $mavenDirectory = Join-Path $ToolDirectory "apache-maven-$version"
    $command = Join-Path $mavenDirectory 'bin\mvn.cmd'
    if (Test-Path -LiteralPath $command -PathType Leaf) { return $command }
    if (Test-Path -LiteralPath $mavenDirectory) {
        throw "Unvollstaendiger Maven-Ordner vorhanden: $mavenDirectory. Bitte pruefen."
    }
    New-Item -ItemType Directory -Path $ToolDirectory -Force | Out-Null
    $staging = Join-Path $ToolDirectory ('.maven-download-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $staging | Out-Null
    try {
        $archive = Join-Path $staging 'maven.zip'
        Write-Host "Lade Apache Maven $version nach $ToolDirectory ..."
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -UseBasicParsing -Uri "https://repo.maven.apache.org/maven2/org/apache/maven/apache-maven/$version/apache-maven-$version-bin.zip" -OutFile $archive
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA512).Hash -ne $expectedHash) {
            throw 'Maven-Download hat eine unerwartete SHA-512-Pruefsumme.'
        }
        Expand-Archive -LiteralPath $archive -DestinationPath $staging
        $extracted = Join-Path $staging "apache-maven-$version"
        if (!(Test-Path -LiteralPath (Join-Path $extracted 'bin\mvn.cmd') -PathType Leaf)) {
            throw 'Maven-Download enthaelt kein Windows-Startskript.'
        }
        Move-Item -LiteralPath $extracted -Destination $mavenDirectory
    }
    finally { Remove-Item -LiteralPath $staging -Recurse -Force }
    return $command
}


function Find-InspectJdk25 {
    $jdkCandidates = @($env:JAVA_HOME, $env:JDK_HOME)
    $javaOnPath = Get-Command java.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($javaOnPath) {
        $jdkCandidates += Split-Path (Split-Path $javaOnPath.Source -Parent) -Parent
        $javaLink = Get-Item -LiteralPath $javaOnPath.Source
        foreach ($linkTarget in @($javaLink.Target)) {
            if ($linkTarget -and [IO.Path]::IsPathRooted($linkTarget)) {
                $jdkCandidates += Split-Path (Split-Path $linkTarget -Parent) -Parent
            }
        }
    }
    $jdkRoots = @(
        "$env:ProgramFiles\Eclipse Adoptium", "$env:ProgramFiles\Java",
        "$env:ProgramFiles\Microsoft", "$env:ProgramFiles\Amazon Corretto",
        "$env:ProgramFiles\BellSoft", "$env:ProgramFiles\Zulu",
        "$env:USERPROFILE\.jdks", 'C:\Java', 'C:\Toolchains'
    )
    foreach ($root in $jdkRoots) {
        if (Test-Path -LiteralPath $root -PathType Container) {
            $jdkCandidates += @(Get-ChildItem -LiteralPath $root -Directory | Sort-Object Name -Descending | ForEach-Object FullName)
        }
    }
    $JavaHome = $null
    foreach ($candidate in @($jdkCandidates | Where-Object { $_ } | Select-Object -Unique)) {
        if (!(Test-Path -LiteralPath (Join-Path $candidate 'bin\java.exe')) -or
            !(Test-Path -LiteralPath (Join-Path $candidate 'bin\javac.exe')) -or
            !(Test-Path -LiteralPath (Join-Path $candidate 'bin\jlink.exe'))) { continue }
        $licensePath = Join-Path $candidate 'legal\java.base\LICENSE'
        $additionalPath = Join-Path $candidate 'legal\java.base\ADDITIONAL_LICENSE_INFO'
        if (!(Test-Path -LiteralPath $licensePath) -or !(Test-Path -LiteralPath $additionalPath)) { continue }
        if ((Get-Content -LiteralPath $licensePath -Raw) -notmatch 'GNU General Public License' -or
            (Get-Content -LiteralPath $additionalPath -Raw) -notmatch 'Classpath') { continue }
        $version = & (Join-Path $candidate 'bin\java.exe') --version | Out-String
        if ($LASTEXITCODE -eq 0 -and $version -match '(?m)^(?:openjdk|java) (?:version )?"?25(?:\.|\s|"|$)') {
            $JavaHome = $candidate
            break
        }
    }
    return $JavaHome
}


function Get-InspectJdkSource {
    param([string]$Jdk)
    $releaseText = Get-Content -LiteralPath (Join-Path $Jdk 'release') -Raw
    $properties = @{}
    foreach ($entry in [regex]::Matches($releaseText, '(?m)^([A-Z_]+)="([^"\r\n]*)"\s*$')) {
        $properties[$entry.Groups[1].Value] = $entry.Groups[2].Value
    }
    # Resolve the installed Corretto revision, rather than guessing a Temurin tag.
    if ($properties['IMPLEMENTOR'] -eq 'Amazon.com Inc.' -or
        $properties['IMPLEMENTOR_VERSION'] -match '^Corretto-') {
        $commitFile = Join-Path $Jdk 'commitId.txt'
        $versionFile = Join-Path $Jdk 'version.txt'
        $sourceRef = $null
        $isCommit = $false
        if (Test-Path -LiteralPath $commitFile) {
            $id = (Get-Content -LiteralPath $commitFile -Raw).Trim()
            if ($id -match '^[a-fA-F0-9]{40}$') {
                $sourceRef = $id.ToLowerInvariant()
                $isCommit = $true
            }
        }
        if (!$sourceRef) {
            $vendorVersion = [regex]::Match(
                [string]$properties['IMPLEMENTOR_VERSION'], '^Corretto-(25(?:\.\d+){3,5})$'
            )
            if ($vendorVersion.Success) { $sourceRef = $vendorVersion.Groups[1].Value }
            elseif (Test-Path -LiteralPath $versionFile) {
                $sourceRef = (Get-Content -LiteralPath $versionFile -Raw).Trim()
            }
            if ($sourceRef -notmatch '^25(?:\.\d+){3,5}$') {
                throw 'Corretto-Distributionskennung fehlt im installierten JDK.'
            }
        }
        Write-Host "Ermittle Corretto-Quellen fuer $sourceRef ..."
        [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
        $headers = @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'FBForge-Inspect-local-build' }
        $commit = Invoke-RestMethod -Headers $headers -TimeoutSec 60 -Uri (
            'https://api.github.com/repos/corretto/corretto-25/commits/' + [Uri]::EscapeDataString($sourceRef)
        )
        if ($commit.sha -notmatch '^[a-f0-9]{40}$' -or
            ($isCommit -and $commit.sha -cne $sourceRef)) {
            throw 'Corretto-Quellrevision konnte nicht eindeutig bestaetigt werden.'
        }
        return "https://github.com/corretto/corretto-25/archive/$($commit.sha).tar.gz"
    }
    $revision = [regex]::Match([string]$properties['SOURCE'], '(?:^|\s)\.:git:([a-f0-9]{12,40})(?:\s|$)')
    $repository = ([string]$properties['SOURCE_REPO']).TrimEnd('/') -replace '\.git$', ''
    if ($revision.Success -and $repository -match '^https://github\.com/[\w.-]+/[\w.-]+$') {
        return "$repository/archive/$($revision.Groups[1].Value).tar.gz"
    }
    if ($properties['IMPLEMENTOR'] -ne 'Eclipse Adoptium' -and
        $properties['IMPLEMENTOR_VERSION'] -notmatch '^Temurin-') {
        throw "JDK-Quellen nicht eindeutig: $($properties['IMPLEMENTOR']) / $($properties['JAVA_RUNTIME_VERSION'])."
    }
    $versionText = & (Join-Path $Jdk 'bin\java.exe') --version | Out-String
    if ($LASTEXITCODE -ne 0) { throw 'JDK-Version konnte nicht gelesen werden.' }
    $version = [regex]::Match($versionText, '\(build (25(?:\.\d+){0,3}\+\d+)(?:[-),\s]|$)')
    if (!$version.Success) { throw 'Vollstaendige Temurin-25-Buildversion fehlt.' }
    $tag = 'jdk-' + $version.Groups[1].Value
    Write-Host "Ermittle Temurin-Quellen fuer $tag ..."
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $headers = @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'FBForge-Inspect-local-build' }
    $release = Invoke-RestMethod -Headers $headers -TimeoutSec 60 -Uri (
        'https://api.github.com/repos/adoptium/temurin25-binaries/releases/tags/' + [Uri]::EscapeDataString($tag)
    )
    if ($release.tag_name -cne $tag -or $release.draft) { throw 'Temurin-Release passt nicht zum JDK.' }
    $sources = @($release.assets | Where-Object { $_.name -match '^OpenJDK25U-jdk-sources_.+\.tar\.gz$' })
    if ($sources.Count -ne 1) { throw 'Kein eindeutiges Quellenarchiv fuer dieses JDK gefunden.' }
    $url = [Uri]$sources[0].browser_download_url
    if ($url.Scheme -ne 'https' -or $url.Host -ne 'github.com' -or
        !$url.AbsolutePath.StartsWith('/adoptium/temurin25-binaries/releases/download/') -or
        $url.Query -or $url.Fragment -or $url.UserInfo) {
        throw 'Unerwartete Temurin-Quellenadresse.'
    }
    return $url.AbsoluteUri
}


Export-ModuleMember -Function Get-InspectMaven, Find-InspectJdk25, Get-InspectJdkSource
