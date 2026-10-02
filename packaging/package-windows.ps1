# SPDX-License-Identifier: MIT
[CmdletBinding()]
param(
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$BinaryDir,
    [string]$OutputDir = 'dist',
    [string]$MakeNsis,
    [string[]]$DllDir = @(),
    [string[]]$RuntimeNotice = @()
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = Split-Path -Parent $PSScriptRoot
Push-Location $repository
try {
    if (-not $BinaryDir) { $BinaryDir = "target/$Target/release" }
    if (-not $MakeNsis) {
        $candidate = Join-Path ${env:ProgramFiles(x86)} 'NSIS\makensis.exe'
        if (Test-Path $candidate) { $MakeNsis = $candidate }
        else { $MakeNsis = (Get-Command makensis.exe -ErrorAction Stop).Source }
    }
    New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
    $metadata = Join-Path $OutputDir 'cargo-metadata.json'
    $metadataText = & cargo metadata --locked --format-version 1 --filter-platform $Target
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    [IO.File]::WriteAllText([IO.Path]::GetFullPath($metadata), ($metadataText -join "`n"), [Text.UTF8Encoding]::new($false))
    $arguments = @('packaging/package_windows.py', '--binary-dir', $BinaryDir, '--output-dir', $OutputDir, '--cargo-metadata', $metadata, '--makensis', $MakeNsis)
    foreach ($directory in $DllDir) { $arguments += @('--dll-dir', $directory) }
    foreach ($notice in $RuntimeNotice) { $arguments += @('--runtime-notice', $notice) }
    & python @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Windows packaging failed' }
} finally {
    Pop-Location
}
