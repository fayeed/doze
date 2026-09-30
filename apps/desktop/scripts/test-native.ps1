param([string]$RenderDirectory)
$ErrorActionPreference = 'Stop'
$desktop = Split-Path $PSScriptRoot -Parent
$executable = Join-Path $desktop 'native/windows/publish/Doze.Settings.exe'
$snapshot = @{
    settings = @{
        launchAtStartup = $false; startMinimized = $true; notifications = $true
        defaultAction = 'sleep'; playbackAction = 'sleep'; silenceSeconds = 60
        idleSeconds = 300; countdownSeconds = 300; logging = $false
        allowDisplaySleep = $false; defaultAwakeMinutes = 30; defaultTimerMinutes = 30
    }
    actions = @('sleep', 'hibernate', 'shutdown', 'lock', 'displayOff')
    settingsPath = (Join-Path $desktop 'native-test/settings.json')
    audioSupported = $true; startupSupported = $true
    status = 'Normal sleep allowed'; timerStatus = 'No power action scheduled'; version = '0.1.0'
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
    if (-not $process.WaitForExit(30000)) {
        $process.Kill()
        throw 'Native WinUI page verification timed out.'
    }
    $output = $stdout.GetAwaiter().GetResult()
    $errors = $stderr.GetAwaiter().GetResult()
    if ($process.ExitCode -ne 0 -or $output -notmatch '"command":"verified"' -or $errors) {
        throw "Native WinUI verification failed (exit $($process.ExitCode)): $errors $output"
    }
    Write-Output 'Verified: all seven native WinUI pages in light/dark themes, draft preservation, and reset defaults. No power actions or settings writes.'
}
finally { $process.Dispose() }
