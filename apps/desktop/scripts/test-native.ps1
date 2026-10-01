param([string]$RenderDirectory)
$ErrorActionPreference = 'Stop'
$desktop = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $desktop 'native/windows/publish/Doze.Settings.exe'
$snapshot = @{
    settings = @{
        agents = @{ enabled = $true; leaseSeconds = 300; defaultCompletion = 'sleep'; keepAliveWhileConnected = $true; clients = @(
            @{ id = 'test-client'; name = 'Codex'; secret = 'test-only'; keepAwake = $true; actions = @('sleep') }
        ) }
        theme = 'system' 
        launchAtStartup = $false; startMinimized = $true; notifications = $true
        defaultAction = 'sleep'; playbackAction = 'sleep'; silenceSeconds = 60
        idleSeconds = 300; countdownSeconds = 300; logging = $false
        allowDisplaySleep = $false; defaultAwakeMinutes = 30; defaultTimerMinutes = 30
    }
    agentNow = 600
    agentSessions = @(
        @{ session_id = 'test-pending'; client_name = 'Claude Code'; reason = 'Running tests'; status = 'awaiting_authorization'; completion_action = 'sleep'; last_heartbeat = 500 },
        @{ session_id = 'test-lost'; client_name = 'Codex'; reason = 'Refactoring'; status = 'connection_lost'; completion_action = 'sleep'; last_heartbeat = 0 },
        @{ session_id = 'test-job'; client_name = 'Command line'; reason = 'Running ffmpeg -i in.mov out.mp4'; status = 'active'; completion_action = 'sleep'; last_heartbeat = 590 }
    )
    session = @{
        awake = $true; awakeRemaining = 2520; whileAudio = $false; holdingAwake = $true
        playbackEnabled = $true; playbackPhase = 'playing'; selectedAction = 'sleep'
        timer = @{ action = 'sleep'; remaining = 3900 }; countdown = $null; error = $null
        message = 'Keep Awake started for 45 minutes'
    }
    executable = 'C:\Users\example\AppData\Local\Doze\doze.exe'
    iconPath = (Join-Path $desktop 'src-tauri/icons/128x128@2x.png')
    clypyIconPath = (Join-Path $desktop 'src-tauri/icons/clypy.png')
    agentConnections = @(@{ name = 'Codex'; codex = 'Test configuration'; generic = '{}'; claude = '{}' })
    agentSkills = @(@{ name = 'Codex'; path = 'C:\Users\example\.agents\skills\doze'; status = 'not_installed' })
    actions = @('sleep', 'hibernate', 'shutdown', 'lock', 'displayOff')
    settingsPath = (Join-Path $desktop 'native-test/settings.json')
    audioSupported = $true; startupSupported = $true
    status = "Keeping awake $([char]0xB7) 42m left"; timerStatus = 'Sleep in 65 minutes'; version = '0.1.0'
}
$start = [System.Diagnostics.ProcessStartInfo]::new($executable)
$start.Arguments = '--verify-ui'
if ($RenderDirectory) { $start.Arguments += ' --render-dir "' + $RenderDirectory + '"' }
$start.UseShellExecute = $false
$start.WorkingDirectory = [System.IO.Path]::GetTempPath()
$start.CreateNoWindow = $true
$start.WindowStyle = [System.Diagnostics.ProcessWindowStyle]::Hidden
$start.RedirectStandardInput = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$process = [System.Diagnostics.Process]::Start($start)
try {
    $process.StandardInput.WriteLine((@{ type = 'open'; view = 'settings'; snapshot = $snapshot } | ConvertTo-Json -Depth 10 -Compress))
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    if (-not $process.WaitForExit($(if ($RenderDirectory) { 300000 } else { 30000 }))) {
        $process.Kill()
        $process.WaitForExit()
        throw "Native WinUI page verification timed out: $($stderr.GetAwaiter().GetResult()) $($stdout.GetAwaiter().GetResult())"
    }
    $output = $stdout.GetAwaiter().GetResult()
    $errors = $stderr.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0 -or $output -notmatch '"command":"verified"' -or $errors) {
        throw "Native WinUI verification failed (exit $($process.ExitCode)): $errors $output"
    }
    Write-Output 'Verified: nine native pages and the Overview control center (idle, active and final-warning sessions), four Mica custom session forms and duration validation, agent controls, immediate changes/rollback, 72-point countdown, topmost preview/real warnings, safe preview dismissal, and Cancel/Snooze/Stay Awake routing. No power actions or settings writes.'
}
finally { $process.Dispose() }
