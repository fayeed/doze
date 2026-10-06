param([string]$RenderDirectory)
$ErrorActionPreference = 'Stop'
$desktop = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $desktop 'native/windows/publish/Doze.Settings.exe'
$snapshot = @{
    settings = @{
        agents = @{ enabled = $true; leaseSeconds = 300; defaultCompletion = 'sleep'; keepAliveWhileConnected = $true
            askBeforeNew = $true; trusted = @('claude-code'); processTools = @(@{ id = 'cursor'; detect = $true; keepAwake = $false })
            clients = @(@{ id = 'test-client'; name = 'Generic MCP client'; secret = 'test-only'; keepAwake = $true; actions = @('sleep') }) }
        theme = 'system'
        launchAtStartup = $true; startMinimized = $true; notifications = $true
        defaultAction = 'sleep'; playbackAction = 'sleep'; silenceSeconds = 60
        idleSeconds = 600; countdownSeconds = 300; logging = $false
        allowDisplaySleep = $false; defaultAwakeMinutes = 30; defaultTimerMinutes = 30
        menuBarTime = $true; menuBarAgentCount = $true; iconClickOpens = 'panel'; lidClosedKeepAwake = $false
        batteryFloorPercent = 15; rememberLastCustomDuration = $true; lastCustomAwakeMinutes = $null
        snoozeMinutes = 15; stayAwakeMinutes = $null; warningSound = $true; warningAllDisplays = $false
        notifyAgentApproval = $true; notifyAgentsFinished = $true; notifyAgentStalled = $true; notifyKeepAwakeEnded = $false
        mcpServerEnabled = $true
    }
    now = 600
    agentNow = 600
    agents = @{
        working = 1; idle = 1; pending = 1; done = 0
        sessions = @(
            @{ id = 's1'; agent = 'claude-code'; name = 'Claude Code'; monogram = 'CC'; project = 'doze-app'; task = 'Running the test suite'; state = 'working'; source = 'hooks'; startedAt = 0; lastEventAt = 590; stateSince = 0; workingSeconds = 120; holdsAssertion = $true },
            @{ id = 's2'; agent = 'opencode'; name = 'OpenCode'; monogram = 'OC'; project = 'api'; task = 'Refactor'; state = 'idle'; source = 'plugin'; startedAt = 0; lastEventAt = 420; stateSince = 420; workingSeconds = 300; holdsAssertion = $false },
            @{ id = 's4'; agent = 'cursor'; name = 'Cursor'; monogram = 'Cu'; task = 'Detected by process'; state = 'idle'; source = 'process'; startedAt = 0; lastEventAt = 590; stateSince = 0; workingSeconds = 0; holdsAssertion = $false },
            @{ id = 's3'; agent = 'codex'; name = 'Codex'; monogram = 'Cx'; project = 'website'; task = 'Migrating the database'; state = 'needsApproval'; source = 'hooks'; startedAt = 590; lastEventAt = 590; stateSince = 590; workingSeconds = 0; holdsAssertion = $false }
        )
    }
    agentLinks = @(
        @{ id = 'claude-code'; name = 'Claude Code'; monogram = 'CC'; method = 'Hooks'; installed = $true; connected = $true; path = 'C:\Users\example\.claude\settings.json' },
        @{ id = 'codex'; name = 'Codex'; monogram = 'Cx'; method = 'Hooks'; installed = $true; connected = $true; path = 'C:\Users\example\.codex\hooks.json' },
        @{ id = 'opencode'; name = 'OpenCode'; monogram = 'OC'; method = 'Plugin'; installed = $true; connected = $true; path = 'C:\Users\example\.config\opencode\plugins\doze.js' },
        @{ id = 'gemini-cli'; name = 'Gemini CLI'; monogram = 'Ge'; method = 'Hooks'; installed = $false; connected = $false; path = 'C:\Users\example\.gemini\settings.json' }
    )
    agentSessions = @()
    session = @{
        awake = $true; awakeRemaining = 2520; awakeDeadline = 3120; whileAudio = $false; audioActive = $false; holdingAwake = $true
        playbackEnabled = $true; playbackPhase = 'playing'; selectedAction = 'sleep'
        timer = @{ action = 'sleep'; remaining = 3900; deadline = 4500 }; countdown = $null; error = $null; batteryLow = $false
        message = 'Keep Awake started for 45 minutes'
    }
    executable = 'C:\Users\example\AppData\Local\Doze\doze.exe'
    cliPath = 'C:/Users/example/AppData/Local/Doze/doze-cli.exe'
    iconPath = (Join-Path $desktop 'src-tauri/icons/128x128@2x.png')
    clypyIconPath = (Join-Path $desktop 'src-tauri/icons/clypy.png')
    actions = @('sleep', 'hibernate', 'shutdown', 'lock', 'displayOff')
    settingsPath = (Join-Path $desktop 'native-test/settings.json')
    audioSupported = $true; startupSupported = $true; lidClosedSupported = $false
    status = "Keeping awake $([char]0xB7) agents working"; statusShort = 'Keeping awake'; statusDetail = 'For Claude Code'; iconState = 'attention'
    timerStatus = 'Sleep in 65 minutes'; version = '0.2.0'
    battery = @{ percent = 82; onBattery = $true }
    assertions = @("System required $([char]0xB7) ES_SYSTEM_REQUIRED | ES_CONTINUOUS")
    mcpAddress = '127.0.0.1:53817'
    links = @(@{ title = 'getdoze.app'; url = 'https://getdoze.app' }, @{ title = 'MCP guide'; url = 'https://getdoze.app/mcp' })
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
    Write-Output 'Verified: the tray flyout and its four pages, nine native pages and the Overview control center (idle, active and final-warning sessions), four Mica custom session forms and duration validation, agent controls, immediate changes/rollback, 72-point countdown, topmost preview/real warnings, safe preview dismissal, and Cancel/Snooze/Stay Awake routing. No power actions or settings writes.'
}
finally { $process.Dispose() }
