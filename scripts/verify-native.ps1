param([Parameter(Mandatory)][string]$Executable, [string]$LiveHost, [string]$LivePageSlug, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$exePath = (Resolve-Path -LiteralPath $Executable).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path (Split-Path -Parent $PSScriptRoot) 'artifacts\native' }
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$outputPath = (Resolve-Path -LiteralPath $OutputDirectory).Path
$keys = @('GLIA_STATE_DIR','GLIA_SNAPSHOT','GLIA_SNAPSHOT_FOCUS','GLIA_SNAPSHOT_REPLAY','GLIA_SNAPSHOT_SEARCH','GLIA_SNAPSHOT_EXPORT','GLIA_SNAPSHOT_PANEL','GLIA_SNAPSHOT_STYLE','GLIA_SNAPSHOT_REFRESH')
$previous = @{}
foreach ($key in $keys) { $previous[$key] = [Environment]::GetEnvironmentVariable($key,'Process'); [Environment]::SetEnvironmentVariable($key,[NullString]::Value,'Process') }
try {
    $env:GLIA_STATE_DIR = Join-Path $outputPath 'state'
    $cases = @(
        @{Name='default';Arguments=@('--demo');Default=$true},
        @{Name='overview';Arguments=@('--demo')},
        @{Name='focus';Arguments=@('--demo');Focus='people/ada-lovelace'},
        @{Name='replay';Arguments=@('--demo');Replay='0.3'},
        @{Name='search';Arguments=@('--demo');Search='pessoa Ada'},
        @{Name='settings';Arguments=@('--demo');Panel='settings'},
        @{Name='help';Arguments=@('--demo');Panel='help'},
        @{Name='appearance';Arguments=@('--demo');Panel='appearance'},
        @{Name='constellation';Arguments=@('--demo');Style='constellation'}
    )
    if ($LiveHost) {
        $cases += @{Name='aws';Arguments=@('--live',$LiveHost)}
        $cases += @{Name='aws-cached';Arguments=@('--live',$LiveHost)}
        if ($LivePageSlug) {
            $cases += @{Name='aws-refresh';Arguments=@('--live',$LiveHost);Focus=$LivePageSlug;Refresh='1'}
            $cases += @{Name='aws-page';Arguments=@('--live',$LiveHost);Focus=$LivePageSlug}
            $cases += @{Name='aws-export';Arguments=@('--live',$LiveHost);Focus=$LivePageSlug;Export='1'}
        }
        $cases += @{Name='aws-constellation';Arguments=@('--live',$LiveHost);Style='constellation'}
    }
    foreach ($case in $cases) {
        $env:GLIA_STATE_DIR = Join-Path $outputPath $(if ($case.Default) { 'state-default' } else { 'state' })
        if ($case.Default) {
            $settingsPath = Join-Path $env:GLIA_STATE_DIR 'settings.json'
            if (Test-Path -LiteralPath $settingsPath) { throw 'Default verification requires a fresh output directory' }
        }
        $env:GLIA_SNAPSHOT = Join-Path $outputPath ($case.Name + '.png')
        $env:GLIA_SNAPSHOT_FOCUS = $case.Focus
        $env:GLIA_SNAPSHOT_REPLAY = $case.Replay
        $env:GLIA_SNAPSHOT_SEARCH = $case.Search
        $env:GLIA_SNAPSHOT_EXPORT = $case.Export
        $env:GLIA_SNAPSHOT_REFRESH = $case.Refresh
        $env:GLIA_SNAPSHOT_PANEL = $case.Panel
        $env:GLIA_SNAPSHOT_STYLE = if ($case.Default) { $null } elseif ($case.Style) { $case.Style } else { 'gephi' }
        $process = Start-Process -FilePath $exePath -ArgumentList $case.Arguments -WindowStyle Hidden -PassThru
        if (-not $process.WaitForExit(60000)) { Stop-Process -Id $process.Id; throw "$($case.Name) native rendering timed out" }
        if ($process.ExitCode -ne 0) { throw "$($case.Name) failed with exit $($process.ExitCode)" }
        $report = Get-Content -LiteralPath (Join-Path $outputPath ($case.Name + '.json')) -Raw | ConvertFrom-Json
        if ($report.app_name -ne 'AGI Brain' -or $report.language -ne 'pt-BR') { throw 'App name or default language was not applied' }
        if (-not $report.screenshot_saved -or $report.error -or $report.pages -le 0 -or $report.drawn_nodes -le 0) { throw "$($case.Name) invalid native report: $($report | ConvertTo-Json -Compress)" }
        if ($case.Focus -and $report.selected -ne $case.Focus) { throw 'Focus was not applied' }
        if ($case.Replay -and ($report.visible -ge $report.pages -or $null -eq $report.replay)) { throw 'Replay did not filter the graph' }
        if ($case.Search -and -not $report.palette) { throw 'Search palette did not open' }
        if ($case.Export -and ($report.export_pages -le 0 -or $report.export_error)) { throw 'Hosted export is unavailable' }
        if ($case.Name -eq 'aws-page' -and ($report.page_chars -le 0 -or $report.page_error)) { throw 'Hosted inspector did not read its page' }
        if ($case.Name -like 'aws*' -and (-not $report.last_sync -or $report.checking)) { throw 'AWS has not completed its live read' }
        if ($case.Name -eq 'aws-refresh' -and ($report.page_chars -le 0 -or $report.page_error -or $report.map_updates -lt 1)) { throw 'Manual refresh lost the map or page preview' }
        if (-not $report.tray) { throw 'Native Windows tray could not be created' }
        $expectedStyle = if ($case.Default) { 'constellation' } else { $env:GLIA_SNAPSHOT_STYLE }
        if ($report.appearance.style -ne $expectedStyle) { throw 'Requested or default graph style was not applied' }
        if ($case.Panel -eq 'appearance' -and -not $report.appearance_open) { throw 'Appearance panel did not open' }
        if ($case.Name -eq 'aws' -and ($report.drawn_nodes -ne $report.pages -or $report.drawn_labels -le 0)) { throw 'Gephi overview is missing nodes or labels' }
        if ($case.Name -in @('default','overview','constellation','aws','aws-constellation')) {
            $image = [System.Drawing.Bitmap]::new($env:GLIA_SNAPSHOT)
            try {
                $white = 0; $samples = 0
                for ($x = 16; $x -lt $image.Width; $x += 32) {
                    for ($y = 16; $y -lt $image.Height; $y += 32) {
                        $pixel = $image.GetPixel($x,$y); $samples++
                        if ($pixel.R -eq 255 -and $pixel.G -eq 255 -and $pixel.B -eq 255) { $white++ }
                    }
                }
                $fraction = $white / $samples
                if ($report.appearance.style -eq 'gephi' -and $fraction -lt 0.2) { throw 'Gephi screenshot does not have a white canvas' }
                if ($report.appearance.style -eq 'constellation' -and $fraction -gt 0.05) { throw 'Constellation screenshot does not have a dark canvas' }
            } finally { $image.Dispose() }
        }
        Write-Output "$($case.Name): $($report.pages) pages, $($report.drawn_nodes) drawn, $($report.drawn_edges) edges; screenshot passed"
    }
} finally {
    foreach ($key in $keys) {
        $value = if ($null -eq $previous[$key]) { [NullString]::Value } else { $previous[$key] }
        [Environment]::SetEnvironmentVariable($key,$value,'Process')
    }
}
