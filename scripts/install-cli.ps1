# Standalone installation; desktop-managed bridge files are never modified.
function Get-SkillsHubUserPath { [Environment]::GetEnvironmentVariable('Path', 'User') }
function Set-SkillsHubUserPath($Value) { [Environment]::SetEnvironmentVariable('Path', $Value, 'User') }

function Install-SkillsHubCli {
    $ErrorActionPreference = 'Stop'
    if ($env:OS -ne 'Windows_NT') { throw 'Use install-cli.sh on macOS or Linux.' }
    $arch = $env:PROCESSOR_ARCHITEW6432
    if (-not $arch) { $arch = $env:PROCESSOR_ARCHITECTURE }
    if ($arch -ne 'AMD64') { throw "Unsupported Windows architecture: $arch. A Windows x64 release is required." }
    if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is unavailable.' }
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    $binDir = Join-Path $env:LOCALAPPDATA 'SkillsHub/bin'
    $dest = Join-Path $binDir 'skillshub-cli.exe'
    if (Test-Path -LiteralPath $dest) {
        $item = Get-Item -LiteralPath $dest -Force
        if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Refusing to replace a link or non-file: $dest"
        }
    }
    Write-Host 'Finding the latest Skills Hub release...'
    $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/qufei1993/skills-hub/releases/latest' -TimeoutSec 120
    $tag = $release.tag_name
    if ($tag -cnotmatch '^v[0-9]+\.[0-9]+\.[0-9]+$') { throw 'Could not resolve a stable release version.' }
    $version = $tag.Substring(1)
    $asset = "skillshub-cli-$version-win32-x64.exe"
    $base = "https://github.com/qufei1993/skills-hub/releases/download/$tag"
    New-Item -ItemType Directory -Force -Path $binDir | Out-Null
    $work = Join-Path $binDir ('.install-' + [Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $stage = Join-Path $work $asset
        $checksumFile = Join-Path $work 'checksum'
        Write-Host "Downloading $asset..."
        try {
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $stage -TimeoutSec 300
            Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset.sha256" -OutFile $checksumFile -TimeoutSec 120
        } catch {
            throw "Download failed. This release may not contain a Windows x64 CLI (CLI assets require v0.11.0 or later). $($_.Exception.Message)"
        }
        $checksum = [IO.File]::ReadAllText($checksumFile)
        if ($checksum -notmatch ('\A([a-fA-F0-9]{64})  ' + [regex]::Escape($asset) + '\r?\n?\z')) {
            throw 'Invalid SHA-256 file.'
        }
        $expected = $Matches[1]
        if ((Get-FileHash -LiteralPath $stage -Algorithm SHA256).Hash -ne $expected) {
            throw 'SHA-256 mismatch; existing CLI was not changed.'
        }
        if ([IO.File]::Exists($dest)) { [IO.File]::Replace($stage, $dest, (Join-Path $work 'previous-cli')) }
        else { [IO.File]::Move($stage, $dest) }
        Write-Host "Installed Skills Hub CLI ${version}: $dest"
    } finally { Remove-Item -LiteralPath $work -Recurse -Force }
    try {
        $userPath = Get-SkillsHubUserPath
        if ($binDir -notin ($userPath -split ';')) {
            Set-SkillsHubUserPath (($binDir, $userPath | Where-Object { $_ }) -join ';')
        }
    } catch { Write-Warning "Could not update user PATH. Add $binDir manually." }
    if ($binDir -notin ($env:PATH -split ';')) { $env:PATH = "$binDir;$env:PATH" }
    Write-Host 'Ready: skillshub-cli version --json. Run this installer again to upgrade.'
}

# Dot-sourcing exposes the installer for isolated tests without starting it.
if ($MyInvocation.InvocationName -ne '.') { Install-SkillsHubCli }
