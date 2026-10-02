# SPDX-License-Identifier: MIT
# No provider is started and no account is needed. Use only on a test machine.
[CmdletBinding()]
param(
    [string]$BinaryDir = 'target/x86_64-pc-windows-msvc/release',
    [string]$OutputFile = 'dist/windows-smoke.json'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$binaryRoot = (Resolve-Path $BinaryDir).Path
$oldData = $env:SPARK_CODE_DATA_DIR
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('spark-code-smoke-' + [Guid]::NewGuid())
$process = $null
$report = [ordered]@{
    os = [Environment]::OSVersion.VersionString
    measuredAtUtc = [DateTime]::UtcNow.ToString('o')
    scenario = 'No-account GUI launch and idle; no provider task started'
    cliHelpPassed = $false
    guiOpened = $false
    guiClosedCleanly = $false
    samples = @()
    tuiMemory = 'Not measured: an interactive terminal host is required'
    windows10Compatibility = 'Not established by a Windows Server or Windows 11 runner'
}
try {
    New-Item -ItemType Directory -Force -Path $scratch | Out-Null
    $env:SPARK_CODE_DATA_DIR = $scratch
    $help = & (Join-Path $binaryRoot 'spark-code.exe') --help
    if ($LASTEXITCODE -ne 0 -or ($help -join "`n") -notmatch 'spark-code') { throw 'CLI --help failed' }
    $report.cliHelpPassed = $true
    $process = Start-Process -FilePath (Join-Path $binaryRoot 'spark-code-desktop.exe') -WorkingDirectory $scratch -PassThru
    Start-Sleep -Seconds 2
    for ($sample = 0; $sample -lt 15; $sample++) {
        $process.Refresh()
        if ($process.HasExited) { throw "GUI exited early with code $($process.ExitCode)" }
        $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($process.Id)" | ForEach-Object {
            $child = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue
            if ($null -ne $child) {
                [ordered]@{ pid = $child.Id; name = $child.ProcessName; workingSetBytes = $child.WorkingSet64; privateBytes = $child.PrivateMemorySize64 }
            }
        })
        $report.samples += [ordered]@{
            elapsedSeconds = $sample + 2
            pid = $process.Id
            workingSetBytes = $process.WorkingSet64
            privateBytes = $process.PrivateMemorySize64
            cpuSeconds = $process.TotalProcessorTime.TotalSeconds
            mainWindowPresent = ($process.MainWindowHandle -ne [IntPtr]::Zero)
            children = $children
        }
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { $report.guiOpened = $true }
        Start-Sleep -Seconds 1
    }
    if (-not $report.guiOpened) { throw 'GUI process stayed alive but no main window was found' }
    if (-not $process.CloseMainWindow()) { throw 'GUI did not accept a normal close request' }
    if (-not $process.WaitForExit(10000)) { throw 'GUI did not exit within 10 seconds after normal close' }
    if ($process.ExitCode -ne 0) { throw "GUI closed with exit code $($process.ExitCode)" }
    $report.guiClosedCleanly = $true
    $workingSet = @($report.samples | ForEach-Object { $_.workingSetBytes }) | Measure-Object -Minimum -Maximum -Average
    $private = @($report.samples | ForEach-Object { $_.privateBytes }) | Measure-Object -Minimum -Maximum -Average
    $report.workingSetBytes = @{ minimum = $workingSet.Minimum; maximum = $workingSet.Maximum; average = $workingSet.Average }
    $report.privateBytes = @{ minimum = $private.Minimum; maximum = $private.Maximum; average = $private.Average }
} catch {
    $report['error'] = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) {
        $process.Refresh()
        if (-not $process.HasExited) {
            $null = $process.CloseMainWindow()
            if (-not $process.WaitForExit(3000)) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
        }
        $process.Dispose()
    }
    $env:SPARK_CODE_DATA_DIR = $oldData
    $outputRoot = Split-Path -Parent $OutputFile
    if ($outputRoot) { New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null }
    $report | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -Path $OutputFile
    # Only the uniquely named directory created by this invocation is removed.
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
