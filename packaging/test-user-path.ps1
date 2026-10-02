# SPDX-License-Identifier: MIT
# Run ONLY on disposable Windows CI. Saves/restores the current user's values.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($env:CI -ne 'true') { throw 'This registry integration test is restricted to disposable CI.' }
$environment = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
$application = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\spark-code')
$options = [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
$before = $environment.GetValue('Path', $null, $options)
$beforeKind = if ($null -ne $before) { $environment.GetValueKind('Path') } else { [Microsoft.Win32.RegistryValueKind]::ExpandString }
$ownedBefore = $application.GetValue('PathAdded', $null, $options)
$ownedKind = if ($null -ne $ownedBefore) { $application.GetValueKind('PathAdded') } else { [Microsoft.Win32.RegistryValueKind]::String }
$directory = Join-Path $env:LOCALAPPDATA ('Programs\spark-path-test-' + [Guid]::NewGuid())
$helper = Join-Path $PSScriptRoot 'manage-user-path.ps1'
function Invoke-Helper([string]$Action) {
    & "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $helper -Action $Action -InstallDir $directory
    if ($LASTEXITCODE -ne 0) { throw "PATH helper $Action failed" }
}
function Assert-Equal($Actual, $Expected, [string]$Label) {
    if ($Actual -cne $Expected) { throw "$Label differs from its expected value" }
}
try {
    $application.DeleteValue('PathAdded', $false)
    # Longer than typical NSIS string limits, with raw expansion and empty entries.
    $original = '%USERPROFILE%\tools;;' + ((1..150 | ForEach-Object { "C:\path-test\segment-$_" }) -join ';') + ';'
    $environment.SetValue('Path', $original, [Microsoft.Win32.RegistryValueKind]::ExpandString)
    Invoke-Helper Add
    Assert-Equal ($environment.GetValue('Path', '', $options)) ($original + ';' + $directory) 'Appended PATH'
    Assert-Equal ($environment.GetValueKind('Path')) ([Microsoft.Win32.RegistryValueKind]::ExpandString) 'Registry type'
    Invoke-Helper Add
    Assert-Equal ($environment.GetValue('Path', '', $options)) ($original + ';' + $directory) 'Repeated addition'
    Invoke-Helper Remove
    Assert-Equal ($environment.GetValue('Path', '', $options)) $original 'Restored long raw PATH'
    Assert-Equal ($application.GetValue('PathAdded', $null, $options)) $null 'Removed ownership marker'
    Invoke-Helper Remove
    Assert-Equal ($environment.GetValue('Path', '', $options)) $original 'Repeated removal'

    $preexisting = 'C:\first;' + $directory + ';C:\last'
    $environment.SetValue('Path', $preexisting, [Microsoft.Win32.RegistryValueKind]::String)
    Invoke-Helper Add
    Assert-Equal ($application.GetValue('PathAdded', $null, $options)) $null 'Pre-existing entry is unowned'
    Invoke-Helper Remove
    Assert-Equal ($environment.GetValue('Path', '', $options)) $preexisting 'Pre-existing entry retained'
    Assert-Equal ($environment.GetValueKind('Path')) ([Microsoft.Win32.RegistryValueKind]::String) 'String registry type retained'
    Write-Output 'PATH add/remove, idempotence, long raw values, empty segments and pre-existing ownership checks passed.'
} finally {
    if ($null -eq $before) { $environment.DeleteValue('Path', $false) }
    else { $environment.SetValue('Path', $before, $beforeKind) }
    if ($null -eq $ownedBefore) { $application.DeleteValue('PathAdded', $false) }
    else { $application.SetValue('PathAdded', $ownedBefore, $ownedKind) }
    $environment.Dispose()
    $application.Dispose()
}
