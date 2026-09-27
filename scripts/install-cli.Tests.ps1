$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/install-cli.ps1"

function Assert($Condition, $Message) { if (-not $Condition) { throw $Message } }
$root = Join-Path ([IO.Path]::GetTempPath()) ([Guid]::NewGuid().ToString())
$oldLocal = $env:LOCALAPPDATA
$oldPath = $env:PATH
$oldOS = $env:OS
$oldArch = $env:PROCESSOR_ARCHITECTURE
$oldArch64 = $env:PROCESSOR_ARCHITEW6432
try {
    $env:LOCALAPPDATA = $root
    $env:OS = 'Windows_NT'
    $env:PROCESSOR_ARCHITECTURE = 'AMD64'
    $env:PROCESSOR_ARCHITEW6432 = ''
    $script:userPath = 'C:\existing'
    $script:failure = ''
    $script:tag = 'v0.11.0'
    $script:payload = [Text.Encoding]::UTF8.GetBytes('fixture-cli')
    function Get-SkillsHubUserPath { return $script:userPath }
    function Set-SkillsHubUserPath($Value) { $script:userPath = $Value }
    function Invoke-RestMethod { return @{ tag_name = $script:tag } }
    function Invoke-WebRequest($Uri, $OutFile) {
        if ($script:failure -eq 'download') { throw 'Mock network failure' }
        if ($Uri.EndsWith('.sha256')) {
            $sha = [Security.Cryptography.SHA256]::Create()
            try { $hash = ([BitConverter]::ToString($sha.ComputeHash($script:payload))).Replace('-', '').ToLowerInvariant() }
            finally { $sha.Dispose() }
            $name = 'skillshub-cli-0.11.0-win32-x64.exe'
            if ($script:failure -eq 'filename') { $name = 'wrong.exe' }
            [IO.File]::WriteAllText($OutFile, "$hash  $name`n")
        } else {
            Assert ($Uri.EndsWith('skillshub-cli-0.11.0-win32-x64.exe')) 'Wrong release asset'
            [IO.File]::WriteAllBytes($OutFile, $script:payload)
            if ($script:failure -eq 'checksum') { [IO.File]::AppendAllText($OutFile, 'corrupt') }
        }
    }
    Install-SkillsHubCli
    Install-SkillsHubCli
    $dest = Join-Path $root 'SkillsHub/bin/skillshub-cli.exe'
    Assert ([IO.File]::ReadAllText($dest) -eq 'fixture-cli') 'Install/upgrade failed'
    Assert (($script:userPath.Split(';') | Where-Object { $_ -like '*SkillsHub*' }).Count -eq 1) 'PATH duplicated'
    Assert ($env:PATH.Contains((Split-Path $dest))) 'Current shell PATH not updated'
    foreach ($failure in @('download', 'checksum', 'filename')) {
        $script:failure = $failure
        $failed = $false
        try { Install-SkillsHubCli } catch { $failed = $true }
        Assert $failed "Expected $failure failure"
        Assert ([IO.File]::ReadAllText($dest) -eq 'fixture-cli') 'Existing CLI changed on failure'
        Assert (@(Get-ChildItem (Split-Path $dest) -Filter '.install-*' -Force).Count -eq 0) 'Staging files leaked'
    }
    $script:failure = ''
    if ($PSVersionTable.PSEdition -eq 'Desktop' -or $IsWindows) {
        $held = [IO.File]::Open($dest, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        try {
            $failed = $false
            try { Install-SkillsHubCli } catch { $failed = $true }
            Assert $failed 'Replacing a busy CLI must fail'
            Assert ([IO.File]::ReadAllText($dest) -eq 'fixture-cli') 'Busy CLI changed on failure'
        } finally { $held.Dispose() }
    }
    $script:tag = 'v0.11.0/evil' 
    $failed = $false
    try { Install-SkillsHubCli } catch { $failed = $true }
    Assert $failed 'Malformed release tag accepted'
    $script:tag = 'v0.11.0'
    $env:PROCESSOR_ARCHITECTURE = 'ARM64'
    $failed = $false
    try { Install-SkillsHubCli } catch { $failed = $true }
    Assert $failed 'Unsupported architecture accepted'
    Write-Host 'All Windows CLI installer tests passed.'
} finally {
    $env:LOCALAPPDATA = $oldLocal; $env:PATH = $oldPath; $env:OS = $oldOS
    $env:PROCESSOR_ARCHITECTURE = $oldArch; $env:PROCESSOR_ARCHITEW6432 = $oldArch64
    if (Test-Path $root) { Remove-Item $root -Recurse -Force }
}
