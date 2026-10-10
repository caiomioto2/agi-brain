param([Parameter(Mandatory)][string]$SshHost)
$ErrorActionPreference = 'Stop'
if ($SshHost -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]{0,100}$') { throw 'Invalid SSH host alias' }
$serverFiles = Join-Path (Split-Path -Parent $PSScriptRoot) 'server'
& ssh -T -o BatchMode=yes $SshHost 'mkdir -p /root/.local/share/glia && chmod 700 /root/.local/share/glia'
if ($LASTEXITCODE) { throw 'Cannot reach the existing AWS SSH host' }
& scp -q -o BatchMode=yes (Join-Path $serverFiles 'read_only_bridge.py') (Join-Path $serverFiles 'test_bridge.py') "${SshHost}:/root/.local/share/glia/"
if ($LASTEXITCODE) { throw 'Reader upload failed' }
& ssh -T -o BatchMode=yes $SshHost 'chmod 600 /root/.local/share/glia/*.py && cd /root/.local/share/glia && python3 -m unittest -v test_bridge'
if ($LASTEXITCODE) { throw 'Hosted reader tests failed' }
