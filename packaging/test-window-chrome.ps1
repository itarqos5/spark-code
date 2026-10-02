# SPDX-License-Identifier: MIT
# No accounts, providers, real projects, registry changes, or simulated input.
# Window commands are sent only to the GUI process started by this test.
[CmdletBinding()]
param(
    [string]$BinaryDir = 'target/x86_64-pc-windows-msvc/release',
    [string]$OutputFile = 'dist/windows-chrome.json'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'This check requires a Windows desktop session.'
}

if (-not ('SparkChromeTest.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
namespace SparkChromeTest {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }
    public static class Native {
        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);
        [DllImport("user32.dll", EntryPoint="GetWindowLongW")]
        public static extern int GetWindowLong(IntPtr window, int index);
        [DllImport("user32.dll")]
        public static extern IntPtr GetSystemMenu(IntPtr window, bool revert);
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetWindowRect(IntPtr window, out Rect rect);
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool IsIconic(IntPtr window);
        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool IsZoomed(IntPtr window);
        [DllImport("user32.dll", EntryPoint="PostMessageW", SetLastError=true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool PostMessage(IntPtr window, uint message, UIntPtr wparam, IntPtr lparam);
    }
}
'@
}

$binaryRoot = (Resolve-Path $BinaryDir).Path
$oldData = $env:SPARK_CODE_DATA_DIR
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('spark-code-chrome-' + [Guid]::NewGuid())
$process = $null
$window = [IntPtr]::Zero
$report = [ordered]@{
    os = [Environment]::OSVersion.VersionString
    measuredAtUtc = [DateTime]::UtcNow.ToString('o')
    scenario = 'Isolated test-owned GUI: style checks and two minimize/restore/maximize/restore cycles'
    guiOpened = $false
    nativeWindowStylesPassed = $false
    nativeSystemMenuPresent = $false
    transitions = @()
    guiClosedCleanly = $false
    manualChecksRequired = @(
        'Custom title-bar button clicks, right-click menu and Alt+Space interaction',
        'Drag, double-click maximize/restore, all resize edges/corners, DPI and multiple monitors',
        'Windows 10 taskbar, Alt+Tab, Win+arrow snap and close behavior',
        'Windows 11 Snap Layout hover is not established by these checks'
    )
    windows10Compatibility = 'Only a run on actual Windows 10 establishes Windows 10 runtime coverage'
}

function Assert-OwnedWindow {
    $process.Refresh()
    if ($process.HasExited) { throw "Test-owned GUI exited with code $($process.ExitCode)" }
    [uint32]$ownerId = 0
    $threadId = [SparkChromeTest.Native]::GetWindowThreadProcessId($window, [ref]$ownerId)
    if ($window -eq [IntPtr]::Zero -or $threadId -eq 0 -or $ownerId -ne $process.Id) {
        throw 'Refusing window operation: window does not belong to the process started by this test'
    }
}

function Get-OwnedRect {
    Assert-OwnedWindow
    $rect = New-Object SparkChromeTest.Rect
    if (-not [SparkChromeTest.Native]::GetWindowRect($window, [ref]$rect)) {
        throw 'Could not inspect the test window bounds'
    }
    return $rect
}

function Invoke-WindowCommand([uint32]$Command, [string]$ExpectedState) {
    Assert-OwnedWindow
    # WM_SYSCOMMAND follows the same window procedure as native system-menu actions.
    if (-not [SparkChromeTest.Native]::PostMessage($window, 0x0112, [UIntPtr]::new($Command), [IntPtr]::Zero)) {
        throw "Could not post $ExpectedState to the test-owned window"
    }
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        Assert-OwnedWindow
        $iconic = [SparkChromeTest.Native]::IsIconic($window)
        $zoomed = [SparkChromeTest.Native]::IsZoomed($window)
        $reached = switch ($ExpectedState) {
            'minimized' { $iconic }
            'maximized' { $zoomed -and -not $iconic }
            'restored' { -not $zoomed -and -not $iconic }
            default { throw 'Unexpected test state' }
        }
        if ($reached) {
            $report.transitions += [ordered]@{ state = $ExpectedState; passed = $true }
            return
        }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Test-owned window did not become $ExpectedState within eight seconds"
}

try {
    New-Item -ItemType Directory -Force -Path $scratch | Out-Null
    $env:SPARK_CODE_DATA_DIR = $scratch
    $process = Start-Process -FilePath (Join-Path $binaryRoot 'spark-code-desktop.exe') -WorkingDirectory $scratch -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    do {
        $process.Refresh()
        if ($process.HasExited) { throw "GUI exited before opening a window with code $($process.ExitCode)" }
        $window = $process.MainWindowHandle
        if ($window -ne [IntPtr]::Zero) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    Assert-OwnedWindow
    $report.guiOpened = $true
    $report['windowTitle'] = $process.MainWindowTitle
    if ($process.MainWindowTitle -notlike 'Spark Code*') { throw 'Unexpected GUI window title' }

    $style = [SparkChromeTest.Native]::GetWindowLong($window, -16) # GWL_STYLE
    $required = 0x00040000 -bor 0x00020000 -bor 0x00010000 -bor 0x00080000
    $report['windowStyleHex'] = '0x{0:X8}' -f $style
    if (($style -band $required) -ne $required) {
        throw 'Missing WS_THICKFRAME, WS_MINIMIZEBOX, WS_MAXIMIZEBOX, or WS_SYSMENU'
    }
    $report.nativeWindowStylesPassed = $true
    if ([SparkChromeTest.Native]::GetSystemMenu($window, $false) -eq [IntPtr]::Zero) {
        throw 'Test-owned window has no native system menu'
    }
    $report.nativeSystemMenuPresent = $true

    Invoke-WindowCommand 0xF120 'restored'
    $before = Get-OwnedRect
    for ($cycle = 0; $cycle -lt 2; $cycle++) {
        Invoke-WindowCommand 0xF020 'minimized'
        Invoke-WindowCommand 0xF120 'restored'
        Invoke-WindowCommand 0xF030 'maximized'
        Invoke-WindowCommand 0xF120 'restored'
    }
    $after = Get-OwnedRect
    foreach ($side in @('Left', 'Top', 'Right', 'Bottom')) {
        if ([Math]::Abs($before.$side - $after.$side) -gt 2) {
            throw "Restored window bounds changed at $side"
        }
    }
    $report['restoredBoundsPassed'] = $true
    Assert-OwnedWindow
    # Only the captured test process receives the close request.
    if (-not $process.CloseMainWindow()) { throw 'GUI rejected a normal close request' }
    if (-not $process.WaitForExit(10000)) { throw 'GUI did not close within ten seconds' }
    if ($process.ExitCode -ne 0) { throw "GUI closed with code $($process.ExitCode)" }
    $report.guiClosedCleanly = $true
    Write-Output 'Native style, system-menu presence, repeated window-state transitions and clean close passed.'
} catch {
    $report['error'] = $_.Exception.Message
    throw
} finally {
    if ($null -ne $process) {
        try {
            $process.Refresh()
            if (-not $process.HasExited) {
                $null = $process.CloseMainWindow()
                if (-not $process.WaitForExit(3000)) { $process.Kill(); $null = $process.WaitForExit(3000) }
            }
        } catch {
            # A process can exit between the check and cleanup. Preserve the
            # test result and restore the environment even if cleanup races.
            $report['cleanupError'] = $_.Exception.Message
        } finally {
            $process.Dispose()
        }
    }
    $env:SPARK_CODE_DATA_DIR = $oldData
    $outputRoot = Split-Path -Parent $OutputFile
    if ($outputRoot) { New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null }
    $report | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 -Path $OutputFile
    # This uniquely named test directory is the only directory removed.
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
