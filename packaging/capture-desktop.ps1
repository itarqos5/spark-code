# Capture the native app with synthetic local data; no providers or login flows run.
[CmdletBinding()]
param(
    [string]$BinaryDir = 'target/release',
    [string]$OutputDir = 'dist/ui-review'
)
$ErrorActionPreference = 'Stop'
$binary = Join-Path (Resolve-Path -LiteralPath $BinaryDir).Path 'spark-code-desktop.exe'
New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null
$output = (Resolve-Path -LiteralPath $OutputDir).Path
$scratch = Join-Path $output 'synthetic-workspace'
New-Item -ItemType Directory -Force -Path $scratch | Out-Null
@'
import json, pathlib, sqlite3, sys, time
root=pathlib.Path(sys.argv[1]); db=sqlite3.connect(root/'spark-code.db')
db.executescript('''
CREATE TABLE IF NOT EXISTS settings(id INTEGER PRIMARY KEY CHECK(id=1),json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,name TEXT NOT NULL,path TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,title TEXT NOT NULL,provider TEXT NOT NULL,model TEXT NOT NULL,remote_id TEXT,updated INTEGER NOT NULL);
CREATE TABLE IF NOT EXISTS messages(id TEXT PRIMARY KEY,session_id TEXT NOT NULL,role TEXT NOT NULL,text TEXT NOT NULL,created INTEGER NOT NULL);
''')
db.execute('INSERT OR REPLACE INTO projects VALUES(?,?,?)',('preview-project','spark-code',str(root)))
for i,title in enumerate(['Improve the workspace UI','Understand the auth flow','Plan a keyboard shortcut','Investigate slow rendering']):
    db.execute('INSERT OR REPLACE INTO sessions VALUES(?,?,?,?,?,?,?)',('preview-chat' if i==0 else f'preview-{i}','preview-project',title,'"Codex"','',None,int(time.time())-i*100000))
messages=[('user','How can we make this workspace feel more responsive?'),('assistant','Start with the input path. Keep conversation history in memory while typing, then load it only when switching conversations or completing a turn.\n\n### A focused implementation\n\n- Cache rendered Markdown for completed messages.\n- Batch streamed text into a single update.\n- Render the interface on the GPU.\n\n```rust\nlet refresh = Duration::from_millis(33);\n```\n\nYou can adjust response refresh and transcript length in **Settings / Performance**. This conversation is synthetic preview content.')]
for i,(role,text) in enumerate(messages):
    db.execute('INSERT OR REPLACE INTO messages VALUES(?,?,?,?,?)',(f'preview-message-{i}','preview-chat',role,text,int(time.time())-60+i))
db.commit(); db.close()
'@ | python - $scratch
if ($LASTEXITCODE -ne 0) { throw 'Could not prepare isolated preview data.' }
$names = @('SPARK_CODE_DATA_DIR', 'SPARK_CODE_CAPTURE', 'SPARK_CODE_CAPTURE_TAB', 'SPARK_CODE_CAPTURE_SESSION', 'SPARK_CODE_CAPTURE_SIZE', 'ICED_BACKEND')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$scenes = @(
    @{ Name='welcome-dark'; Light=$false; Tab=''; Chat=$false },
    @{ Name='general-dark'; Light=$false; Tab='General'; Chat=$false },
    @{ Name='providers-dark'; Light=$false; Tab='Providers'; Chat=$false },
    @{ Name='performance-dark'; Light=$false; Tab='Performance'; Chat=$false },
    @{ Name='chatgpt-dark'; Light=$false; Tab='ChatGPT & Dots'; Chat=$false },
    @{ Name='chat-light'; Light=$true; Tab=''; Chat=$true },
    @{ Name='appearance-light'; Light=$true; Tab='Appearance'; Chat=$false },
    @{ Name='data-light'; Light=$true; Tab='Data & history'; Chat=$false },
    @{ Name='shortcuts-dark'; Light=$false; Tab='Keyboard shortcuts'; Chat=$false },
    @{ Name='appearance-compact-dark'; Light=$false; Tab='Appearance'; Chat=$false; Size='920x640' },
    @{ Name='projects-compact-dark'; Light=$false; Tab=''; Chat=$false; Size='920x640'; Projects=12 }
)
$report = @()
try {
    $env:SPARK_CODE_DATA_DIR = $scratch
    foreach ($scene in $scenes) {
        @'
import json, pathlib, sqlite3, sys
db=sqlite3.connect(pathlib.Path(sys.argv[1])/'spark-code.db')
settings={'light_theme':sys.argv[2]=='True','auto_detect_cli':False,'auto_refresh_providers':False,'show_timestamps':True}
db.execute('INSERT OR REPLACE INTO settings VALUES(1,?)',(json.dumps(settings),)); db.commit(); db.close()
db=sqlite3.connect(pathlib.Path(sys.argv[1])/'spark-code.db')
db.execute("DELETE FROM projects WHERE id LIKE 'preview-extra-%'")
names=['web-client','api-service','design-system','auth-gateway','worker-service','mobile-app','docs-site','test-runner','data-pipeline','shared-components','payments','infrastructure']
for i in range(int(sys.argv[3])):
    folder=pathlib.Path(sys.argv[1])/names[i]; folder.mkdir(exist_ok=True)
    db.execute('INSERT INTO projects VALUES(?,?,?)',(f'preview-extra-{i}',names[i],str(folder)))
db.commit(); db.close()
'@ | python - $scratch $scene.Light $(if ($scene.Projects) { $scene.Projects } else { 0 })
        $env:SPARK_CODE_CAPTURE = Join-Path $output ($scene.Name + '.png')
        $env:SPARK_CODE_CAPTURE_TAB = $scene.Tab
        $env:SPARK_CODE_CAPTURE_SIZE = if ($scene.Size) { $scene.Size } else { '1280x840' }
        $env:SPARK_CODE_CAPTURE_SESSION = if ($scene.Chat) { 'preview-chat' } else { '' }
        $destination = $env:SPARK_CODE_CAPTURE
        if ($scene.Tab) { $destination = $destination.Replace('.png', '-settings.png') }
        elseif ($scene.Chat) { $destination = $destination.Replace('.png', '-chat.png') }
        if (Test-Path -LiteralPath $destination) { Remove-Item -LiteralPath $destination -Force }
        $process = Start-Process -FilePath $binary -WorkingDirectory $scratch -WindowStyle Hidden -PassThru
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(25)
            while (-not (Test-Path -LiteralPath $destination) -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 200
                $process.Refresh()
                if ($process.HasExited) { throw "Preview exited with code $($process.ExitCode)" }
            }
            if (-not (Test-Path -LiteralPath $destination)) { throw "Capture did not finish: $($scene.Name)" }
            Start-Sleep -Seconds 2
            $process.Refresh()
            $cpuStart = $process.TotalProcessorTime.TotalSeconds
            Start-Sleep -Seconds 2
            $process.Refresh()
            $report += [ordered]@{ scene=$scene.Name; screenshot=$destination; memoryMiB=[Math]::Round($process.WorkingSet64/1MB,1); idleSampleSeconds=2; idleCpuSeconds=[Math]::Round($process.TotalProcessorTime.TotalSeconds-$cpuStart,3) }
        } finally {
            if (-not $process.HasExited) {
                $null = $process.CloseMainWindow()
                if (-not $process.WaitForExit(4000)) { $process.Kill(); $process.WaitForExit() }
            }
        }
    }
    $report | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $output 'capture-report.json') -Encoding utf8
    $report | Format-Table -AutoSize
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
}
