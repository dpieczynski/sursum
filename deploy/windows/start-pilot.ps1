# SPDX-License-Identifier: GPL-3.0-or-later
<#
.SYNOPSIS
    Starts sursum for the Windows pilot (P1): the server, the screen as a full-screen kiosk on the
    projector (the second monitor) and the remote in the default browser on the operator's monitor.

.DESCRIPTION
    Put this script next to sursum-server.exe (from `just cross-win` or the CI artifact) and run it
    from a shortcut: powershell.exe -ExecutionPolicy Bypass -File start-pilot.ps1
    Data (database, backups, pictures, config.toml) lives in %LOCALAPPDATA%\sursum.

.NOTES
    ⚠ Not yet tried on Windows. To check on the pilot PC (plan §12): Edge kiosk flags, the kiosk
    window landing on the second monitor, the firewall prompt on first start (allow "private
    networks" so a tablet can connect). Fallback if Edge misbehaves: Chrome with
    --app=<url> --start-fullscreen --window-position=<x>,<y>.
#>
param(
    [string]$DataDir = (Join-Path $env:LOCALAPPDATA 'sursum'),
    [int]$Port = 8080,
    [string]$Server = (Join-Path $PSScriptRoot 'sursum-server.exe')
)

$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null

# 1. The server, in its own window so its log stays visible.
Start-Process -FilePath $Server -ArgumentList @('--data-dir', "`"$DataDir`"", '--listen', "0.0.0.0:$Port") | Out-Null

# 2. Wait until it answers (at most 30 s).
$ready = $false
for ($attempt = 0; $attempt -lt 60 -and -not $ready; $attempt++) {
    try {
        $answer = Invoke-WebRequest -UseBasicParsing -TimeoutSec 1 "http://localhost:$Port/healthz"
        $ready = $answer.Content -eq 'ok'
    } catch {
        Start-Sleep -Milliseconds 500
    }
}
if (-not $ready) {
    throw 'sursum-server did not start; look at its window for the reason.'
}

# 3. The screen: full screen without any browser interface, on the monitor that is not primary
#    (the projector). Set Windows to "Extend" with the operator's monitor as the main display.
Add-Type -AssemblyName System.Windows.Forms
$projector = [System.Windows.Forms.Screen]::AllScreens | Where-Object { -not $_.Primary } | Select-Object -First 1
if ($null -eq $projector) {
    Write-Warning 'Only one monitor found: the screen opens on it. Connect the projector and run again.'
    $projector = [System.Windows.Forms.Screen]::PrimaryScreen
}
$bounds = $projector.Bounds
$edge = Join-Path ${env:ProgramFiles(x86)} 'Microsoft\Edge\Application\msedge.exe'
$kioskProfile = Join-Path $DataDir 'kiosk-profile'
Start-Process -FilePath $edge -ArgumentList @(
    '--kiosk', "http://localhost:$Port/screen/",
    '--edge-kiosk-type=fullscreen',
    "--window-position=$($bounds.X),$($bounds.Y)",
    "--user-data-dir=`"$kioskProfile`"",
    '--no-first-run',
    '--disable-features=Translate'
) | Out-Null

# 4. The remote, in the default browser on the operator's monitor.
Start-Process "http://localhost:$Port/remote/"
