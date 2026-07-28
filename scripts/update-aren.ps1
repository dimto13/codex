param(
    [string]$Tag = "",
    [string]$Repository = $(if ($env:AREN_GITHUB_REPOSITORY) { $env:AREN_GITHUB_REPOSITORY } else { "dimto13/codex" }),
    [string]$InstallDir = $(if ($env:AREN_INSTALL_DIR) { $env:AREN_INSTALL_DIR } else { Join-Path $HOME ".local\bin" }),
    [int]$ParentProcessId = 0,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

if ($Help) {
    Write-Host "Usage: aren-update [-Tag TAG] [-Repository OWNER/REPO] [-InstallDir DIR]"
    Write-Host "Installs the latest stable Aren release when -Tag is omitted."
    exit 0
}

if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [System.Runtime.InteropServices.OSPlatform]::Windows
)) {
    throw "The Aren PowerShell updater supports Windows only."
}
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne
    [System.Runtime.InteropServices.Architecture]::X64) {
    throw "Aren releases currently support Windows x86_64 only."
}

if (-not $Tag) {
    $release = Invoke-RestMethod `
        -Headers @{ Accept = "application/vnd.github+json" } `
        -Uri "https://api.github.com/repos/$Repository/releases/latest"
    $Tag = $release.tag_name
}
if ($Tag -notmatch '^aren-v[0-9]+\.[0-9]+\.[0-9]+([.-][A-Za-z0-9.-]+)?$') {
    throw "Invalid or unavailable Aren release tag: $Tag"
}

$archiveName = "aren-windows-x86_64.zip"
$checksumName = "$archiveName.sha256"
$releaseUrl = "https://github.com/$Repository/releases/download/$Tag"
$temporaryDir = Join-Path ([System.IO.Path]::GetTempPath()) "aren-update-$([guid]::NewGuid())"

try {
    New-Item -ItemType Directory -Path $temporaryDir | Out-Null
    $archivePath = Join-Path $temporaryDir $archiveName
    $checksumPath = Join-Path $temporaryDir $checksumName
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseUrl/$archiveName" -OutFile $archivePath
    Invoke-WebRequest -UseBasicParsing -Uri "$releaseUrl/$checksumName" -OutFile $checksumPath

    $expectedChecksum = ((Get-Content -Raw $checksumPath).Trim() -split '\s+')[0]
    $actualChecksum = (Get-FileHash -Algorithm SHA256 $archivePath).Hash
    if ($actualChecksum -ne $expectedChecksum) {
        throw "SHA-256 verification failed for $archiveName."
    }

    $extractDir = Join-Path $temporaryDir "extracted"
    Expand-Archive -Path $archivePath -DestinationPath $extractDir
    foreach ($requiredFile in @("aren.exe", "aren-update.ps1", "aren-update.cmd", "BUILD-INFO.txt")) {
        if (-not (Test-Path (Join-Path $extractDir $requiredFile))) {
            throw "Release archive does not contain $requiredFile."
        }
    }
    $expectedVersion = $Tag.Substring(6)
    $versionOutput = & (Join-Path $extractDir "aren.exe") --version
    if ($LASTEXITCODE -ne 0 -or $versionOutput -ne "aren $expectedVersion") {
        throw "Downloaded Aren binary does not match $Tag."
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    foreach ($fileName in @("aren.exe", "aren-update.ps1")) {
        $temporaryTarget = Join-Path $InstallDir ".$fileName.new"
        Copy-Item -Force (Join-Path $extractDir $fileName) $temporaryTarget
    }
    $cmdTarget = Join-Path $InstallDir "aren-update.cmd"
    if (-not (Test-Path $cmdTarget)) {
        Copy-Item (Join-Path $extractDir "aren-update.cmd") $cmdTarget
    }

    if ($ParentProcessId -gt 0) {
        $finalizerPath = Join-Path $InstallDir ".aren-update-finalize-$ParentProcessId.ps1"
        @'
param(
    [int]$ParentProcessId,
    [string]$InstallDir
)

$ErrorActionPreference = "Stop"
Wait-Process -Id $ParentProcessId -ErrorAction SilentlyContinue
foreach ($fileName in @("aren.exe", "aren-update.ps1")) {
    Move-Item -Force `
        (Join-Path $InstallDir ".$fileName.new") `
        (Join-Path $InstallDir $fileName)
}
Remove-Item -Force $PSCommandPath
'@ | Set-Content -Encoding utf8 $finalizerPath

        Start-Process `
            -FilePath "powershell.exe" `
            -WindowStyle Hidden `
            -ArgumentList @(
                "-NoProfile",
                "-ExecutionPolicy", "Bypass",
                "-File", "`"$finalizerPath`"",
                "-ParentProcessId", $ParentProcessId,
                "-InstallDir", "`"$InstallDir`""
            )
        Write-Host "Aren $Tag will be activated after the current Aren process exits."
    } else {
        foreach ($fileName in @("aren.exe", "aren-update.ps1")) {
            Move-Item -Force `
                (Join-Path $InstallDir ".$fileName.new") `
                (Join-Path $InstallDir $fileName)
        }
        Write-Host "Aren $Tag installed at $(Join-Path $InstallDir 'aren.exe')"
    }
} finally {
    if (Test-Path $temporaryDir) {
        Remove-Item -Recurse -Force $temporaryDir
    }
}
