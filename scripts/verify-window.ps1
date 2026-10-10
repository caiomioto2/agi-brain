param([Parameter(Mandatory)][string]$Executable, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path -LiteralPath $Executable).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts\window-startup' }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$statePath = (Resolve-Path -LiteralPath $OutputDirectory).Path
if (-not ('AGIBrainWindowProbe' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class AGIBrainWindowProbe {
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr window, int command);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr window, int x, int y, int width, int height, bool repaint);
}
'@
}
$keys = @('GLIA_STATE_DIR','GLIA_SNAPSHOT')
$previous = @{}
foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key,'Process'); [Environment]::SetEnvironmentVariable($key,[NullString]::Value,'Process') }
$ownedProcess = $null
try {
    $env:GLIA_STATE_DIR = $statePath
    $tiny = @{window='(inner_position_pixels:Some((x:8.0,y:31.0)),outer_position_pixels:Some((x:0.0,y:0.0)),fullscreen:false,maximized:false,inner_size_points:Some((x:64.0,y:64.0)))'}
    [IO.File]::WriteAllText((Join-Path $statePath 'window.ron'),($tiny | ConvertTo-Json),[Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText((Join-Path $statePath 'settings.json'),'{"minimize_to_tray":false}',[Text.UTF8Encoding]::new($false))
    foreach ($attempt in 1..2) {
        $ownedProcess = Start-Process -FilePath $exePath -ArgumentList '--demo' -WindowStyle Hidden -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds(30)
        $passed = $false
        while ([DateTime]::UtcNow -lt $deadline) {
            $ownedProcess.Refresh()
            if ($ownedProcess.HasExited) { throw "App exited during window verification: $($ownedProcess.ExitCode)" }
            $handle = $ownedProcess.MainWindowHandle
            if ($handle -ne [IntPtr]::Zero -and [AGIBrainWindowProbe]::IsWindowVisible($handle) -and [AGIBrainWindowProbe]::IsZoomed($handle) -and -not [AGIBrainWindowProbe]::IsIconic($handle)) { $passed = $true; break }
            Start-Sleep -Milliseconds 100
        }
        if (-not $passed) { throw "Launch $attempt did not produce a visible maximized window" }
        Write-Output "Launch ${attempt}: visible, maximized, not minimized; saved 64x64 geometry ignored"
        if ($attempt -eq 1) {
            [AGIBrainWindowProbe]::ShowWindow($handle,9) | Out-Null
            [AGIBrainWindowProbe]::MoveWindow($handle,80,80,800,580,$true) | Out-Null
            Start-Sleep -Milliseconds 200
            if ([AGIBrainWindowProbe]::IsZoomed($handle)) { throw 'Users cannot restore the window during a session' }
        }
        $ownedProcess.CloseMainWindow() | Out-Null
        if (-not $ownedProcess.WaitForExit(10000)) { throw 'Test-owned application did not close normally' }
        $ownedProcess = $null
    }
} finally {
    if ($ownedProcess -and -not $ownedProcess.HasExited) { Stop-Process -Id $ownedProcess.Id -ErrorAction SilentlyContinue }
    foreach ($key in $keys) {
        $value = if ($null -eq $previous[$key]) { [NullString]::Value } else { $previous[$key] }
        [Environment]::SetEnvironmentVariable($key,$value,'Process')
    }
}
