$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path "$PSScriptRoot/../..").Path
$binary = Join-Path $workspace 'target/debug/verslot.exe'
$base = Join-Path $workspace ('target/m3-smoke-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $base | Out-Null
$root = Join-Path $base 'verslot'
$log = Join-Path $PSScriptRoot 'smoke.txt'
"Isolated base: $base" | Set-Content $log
function Invoke-SmokeCommand($executable, $arguments, $expected) {
    $info = [System.Diagnostics.ProcessStartInfo]::new()
    $info.FileName = $executable
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.Environment['HOME'] = $base
    $info.Environment['LOCALAPPDATA'] = $base
    foreach ($argument in $arguments) { $info.ArgumentList.Add($argument) }
    $process = [System.Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    "Command: $executable $arguments`nExit: $($process.ExitCode)`nStdout: $stdout`nStderr: $stderr" | Add-Content $log
    if ($process.ExitCode -ne 0 -or $stderr -ne '' -or $stdout.Replace("`r`n", "`n") -ne $expected) {
        throw "Smoke command failed; see $log"
    }
    $process.Dispose()
}
Invoke-SmokeCommand $binary @('list') ''
if (Test-Path $root) { throw 'Empty list created storage' }
Invoke-SmokeCommand $binary @('install', 'node@22.0.0') "installed node@22.0.0`n"
$installation = Join-Path $root 'installs/node/22.0.0'
$receipt = Get-Content (Join-Path $installation '.verslot-install') -Raw
"Archive URL: https://nodejs.org/dist/v22.0.0/node-v22.0.0-win-x64.zip`nChecksum URL: https://nodejs.org/dist/v22.0.0/SHASUMS256.txt`nReceipt:`n$receipt" | Add-Content $log
$checksumText = Get-Content (Join-Path $PSScriptRoot 'SHASUMS256.txt') -Raw
$checksum = ($checksumText -split "`n" | Where-Object { $_ -match '^[0-9a-f]{64}\s+node-v22\.0\.0-win-x64\.zip\s*$' })
if (@($checksum).Count -ne 1 -or ($checksum -split '\s+')[0] -ne ($receipt -split "`n")[3]) { throw 'Receipt digest does not match official checksum' }
Invoke-SmokeCommand $binary @('list') "node@22.0.0`n"
$node = Join-Path $installation 'node.exe'
$payloadHash = (Get-FileHash $node -Algorithm SHA256).Hash
Invoke-SmokeCommand $binary @('install', 'node@22.0.0') "already installed node@22.0.0`n"
if ((Get-Content (Join-Path $installation '.verslot-install') -Raw) -ne $receipt -or (Get-FileHash $node -Algorithm SHA256).Hash -ne $payloadHash) { throw 'Duplicate changed payload or receipt' }
Invoke-SmokeCommand $node @('--version') "v22.0.0`n"
Invoke-SmokeCommand $binary @('uninstall', 'node@22.0.0') "uninstalled node@22.0.0`n"
Invoke-SmokeCommand $binary @('list') ''
if ((Test-Path $installation) -or (Test-Path (Join-Path $root 'current')) -or @(Get-ChildItem (Join-Path $root 'tmp')).Count -ne 0) { throw 'Unexpected remaining installation, current state, or operation' }
'PASS: all smoke steps; duplicate unchanged; installation absent; tmp empty; current absent.' | Add-Content $log
Get-Content $log
