param([Parameter(ValueFromRemainingArguments = $true)][string[]]$CargoArguments)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
$cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
$env:PATH = "$cargoBin;$env:PATH"
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$vsRoot = & $vswhere -latest -products '*' -property installationPath
$vcRoot = Join-Path $vsRoot 'VC\Tools\MSVC'
$vc = Get-ChildItem -LiteralPath $vcRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
if (-not $vc) { throw 'Install Visual Studio C++ Build Tools.' }
$env:PATH = (Join-Path $vc.FullName 'bin\Hostx64\x64') + ';' + $env:PATH
$sdkLibraries = Join-Path $workspace '.tools\windows-sdk-x64\c'
if (Test-Path -LiteralPath $sdkLibraries) {
    $env:LIB = (Join-Path $vc.FullName 'lib\x64') + ';' + (Join-Path $sdkLibraries 'um\x64') + ';' + (Join-Path $sdkLibraries 'ucrt\x64')
    $sdkIncludes = Join-Path $workspace '.tools\windows-sdk\c\Include'
    if (Test-Path -LiteralPath $sdkIncludes) {
        $includeVersion = Get-ChildItem -LiteralPath $sdkIncludes -Directory | Sort-Object Name -Descending | Select-Object -First 1
        $env:INCLUDE = (Join-Path $vc.FullName 'include') + ';' + (Join-Path $includeVersion.FullName 'um') + ';' + (Join-Path $includeVersion.FullName 'shared') + ';' + (Join-Path $includeVersion.FullName 'ucrt')
    }
} else {
    $devCmd = Join-Path $vsRoot 'Common7\Tools\VsDevCmd.bat'
    $toolEnv = & cmd.exe /d /s /c "`"$devCmd`" -arch=x64 -host_arch=x64 >nul && set"
    foreach ($line in $toolEnv) {
        if ($line -match '^([^=]+)=(.*)$') { [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process') }
    }
}
& (Join-Path $cargoBin 'cargo.exe') @CargoArguments
exit $LASTEXITCODE
