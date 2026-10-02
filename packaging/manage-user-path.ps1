# SPDX-License-Identifier: MIT
# Changes only this user's PATH. Called by the optional installer component.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateSet('Add', 'Remove')][string]$Action,
    [Parameter(Mandatory = $true)][string]$InstallDir
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Normalize-PathEntry([string]$Entry) {
    return [Environment]::ExpandEnvironmentVariables($Entry.Trim().Trim('"')).TrimEnd('\', '/')
}

$environment = $null
$application = $null
try {
    $directory = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\', '/')
    if ($directory.Contains(';')) { throw 'An installation path cannot contain a semicolon.' }
    $environment = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
    $application = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Software\spark-code')
    $options = [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
    $original = [string]$environment.GetValue('Path', '', $options)
    $kind = [Microsoft.Win32.RegistryValueKind]::ExpandString
    if ($null -ne $environment.GetValue('Path', $null, $options)) {
        $kind = $environment.GetValueKind('Path')
        if ($kind -notin @([Microsoft.Win32.RegistryValueKind]::String, [Microsoft.Win32.RegistryValueKind]::ExpandString)) {
            throw 'The user PATH has an unexpected registry type; leaving it unchanged.'
        }
    }
    $owned = [string]$application.GetValue('PathAdded', '', $options)
    $entries = @($original.Split(';'))
    $normalized = Normalize-PathEntry $directory
    $comparer = [StringComparer]::OrdinalIgnoreCase
    if ($Action -eq 'Add') {
        $present = @($entries | Where-Object { $comparer.Equals((Normalize-PathEntry $_), $normalized) }).Count -gt 0
        if (-not $present) {
            if ($owned -and -not $comparer.Equals((Normalize-PathEntry $owned), $normalized)) {
                throw 'A different spark-code installation owns a PATH entry. Uninstall it before changing installation folders.'
            }
            $updated = if ([string]::IsNullOrEmpty($original)) { $directory } else { $original + ';' + $directory }
            if ($updated.Length -gt 32760) { throw 'Adding this directory would exceed the supported user PATH length; leaving it unchanged.' }
            # Save ownership first. On an interrupted write, Remove remains harmless.
            $application.SetValue('PathAdded', $directory, [Microsoft.Win32.RegistryValueKind]::String)
            $environment.SetValue('Path', $updated, $kind)
        }
        # An entry that already existed is never claimed by this installer.
    } elseif ($owned -and $comparer.Equals((Normalize-PathEntry $owned), $normalized)) {
        $remaining = [Collections.Generic.List[string]]::new()
        $removed = $false
        foreach ($entry in $entries) {
            if (-not $removed -and $comparer.Equals((Normalize-PathEntry $entry), (Normalize-PathEntry $owned))) {
                $removed = $true
            } else {
                $remaining.Add($entry)
            }
        }
        if ($removed) { $environment.SetValue('Path', ($remaining -join ';'), $kind) }
        $application.DeleteValue('PathAdded', $false)
    }
    exit 0
} catch {
    Write-Error $_
    exit 1
} finally {
    if ($null -ne $environment) { $environment.Dispose() }
    if ($null -ne $application) { $application.Dispose() }
}
