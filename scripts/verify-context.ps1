param([Parameter(Mandatory)][string]$Executable, [string]$LiveHost)
$ErrorActionPreference = 'Stop'
$exePath = (Resolve-Path -LiteralPath $Executable).Path
$workspace = Split-Path -Parent $PSScriptRoot
$fixturePath = Join-Path $workspace 'artifacts\context-fixture'
New-Item -ItemType Directory -Force -Path (Join-Path $fixturePath 'source-default\people') | Out-Null
'{"nodes":[{"id":1,"slug":"people/ada","type":"person","title":"Ada Lovelace"}],"links":[]}' | Set-Content -LiteralPath (Join-Path $fixturePath 'graph.json') -Encoding utf8NoBOM
'# Ada Lovelace
Synthetic test identity and computing pioneer.' | Set-Content -LiteralPath (Join-Path $fixturePath 'source-default\people\ada.md') -Encoding utf8NoBOM
'# Synthetic Psyche
I prefer clear, practical explanations.' | Set-Content -LiteralPath (Join-Path $fixturePath 'psyche.md') -Encoding utf8NoBOM
function Invoke-Protocol([hashtable]$Variables, [string]$Query) {
    $info = [Diagnostics.ProcessStartInfo]::new($exePath)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($key in $Variables.Keys) { if ([string]::IsNullOrEmpty($Variables[$key])) { $info.Environment.Remove($key) | Out-Null } else { $info.Environment[$key] = $Variables[$key] } }
    $p = [Diagnostics.Process]::Start($info)
    $requests = @(
        @{jsonrpc='2.0';id=1;method='initialize';params=@{protocolVersion='2025-06-18'}},
        @{jsonrpc='2.0';method='notifications/initialized'},
        @{jsonrpc='2.0';id=2;method='tools/list'},
        @{jsonrpc='2.0';id=3;method='tools/call';params=@{name='health';arguments=@{}}},
        @{jsonrpc='2.0';id=4;method='tools/call';params=@{name='who_am_i';arguments=@{}}},
        @{jsonrpc='2.0';id=5;method='tools/call';params=@{name='recall';arguments=@{query=$Query;max_tokens=2000}}},
        @{jsonrpc='2.0';id=6;method='tools/call';params=@{name='prime_context';arguments=@{task=$Query;max_tokens=4000}}},
        @{jsonrpc='2.0';id=7;method='tools/call';params=@{name='explain_context';arguments=@{task=$Query}}}
    )
    foreach ($request in $requests) { $p.StandardInput.WriteLine(($request | ConvertTo-Json -Depth 10 -Compress)) }
    $p.StandardInput.Close()
    $outputTask = $p.StandardOutput.ReadToEndAsync()
    $errorTask = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit(60000)) { $p.Kill(); throw 'MCP process timed out' }
    $output = $outputTask.GetAwaiter().GetResult()
    $errors = $errorTask.GetAwaiter().GetResult()
    if ($p.ExitCode -ne 0 -or $errors) { throw "MCP failed: $errors" }
    $rows = @($output.Trim() -split '\r?\n' | ForEach-Object { $_ | ConvertFrom-Json })
    if ($rows.Count -ne 7 -or $rows[1].result.tools.Count -ne 5) { throw 'Invalid stdio handshake or tool catalog' }
    foreach ($row in $rows) { if ($row.error -or $row.result.isError) { throw "MCP request $($row.id) failed: $($row.result.content[0].text)" } }
    if ($rows[3].result.content[0].text -notlike '*Synthetic Psyche*') { throw 'Explicit identity was not read' }
    if ($rows[4].result.content[0].text -notmatch '[1-9][0-9]* readable pages') { throw 'Retrieval returned no readable pages' }
    if ($rows[5].result.content[0].text -notlike '*status: OK*') { throw 'Identity and retrieval were not combined' }
    $health = $rows[2].result.content[0].text | ConvertFrom-Json
    Write-Output "MCP passed: handshake, 5 tools, $($health.graph.pages) graph pages, identity, recall, combined context, explanation"
}
Invoke-Protocol @{GLIA_STATE_DIR=$fixturePath;GLIA_DATA=(Join-Path $fixturePath 'graph.json');GLIA_PSYCHE=(Join-Path $fixturePath 'psyche.md')} 'Ada'
if ($LiveHost) {
    $livePath = Join-Path $fixturePath 'aws'
    New-Item -ItemType Directory -Force -Path $livePath | Out-Null
    @{live=$true;ssh_host=$LiveHost} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $livePath 'settings.json') -Encoding utf8NoBOM
    Invoke-Protocol @{GLIA_STATE_DIR=$livePath;GLIA_DATA='';GLIA_PSYCHE=(Join-Path $fixturePath 'psyche.md')} 'gbrain'
}
