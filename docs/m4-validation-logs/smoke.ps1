$ErrorActionPreference = 'Stop'
$workspace = 'D:\code\verslot'
$verslotBinary = Join-Path $workspace 'target/debug/verslot.exe'
$logDirectory = Join-Path $workspace 'docs/m4-validation-logs'
$smokeBase = Join-Path $workspace ('target/m4-smoke-' + [Guid]::NewGuid().ToString('N'))
$smokeRoot = Join-Path $smokeBase 'verslot'
$smokeLog = Join-Path $logDirectory 'smoke.txt'
$utf8 = [Text.UTF8Encoding]::new($false)
[IO.File]::WriteAllText($smokeLog, "M4 Windows x86_64 smoke $(Get-Date -Format o)`nbase=$smokeBase`nbinary=$verslotBinary`n", $utf8)
New-Item -ItemType Directory -Path $smokeBase | Out-Null
$env:HOME = $smokeBase
$env:LOCALAPPDATA = $smokeBase
$originalSmokePath = $env:PATH
Set-Location -LiteralPath $smokeBase

function Invoke-SmokeCommand {
    param(
        [string]$Executable,
        [string[]]$Arguments,
        [string]$ExpectedStdout,
        [int]$ExpectedExit = 0,
        [string]$ExpectedError = ''
    )
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $Executable
    foreach ($argument in $Arguments) { $startInfo.ArgumentList.Add($argument) }
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.WorkingDirectory = $smokeBase
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $child = [Diagnostics.Process]::Start($startInfo)
    $stdoutTask = $child.StandardOutput.ReadToEndAsync()
    $stderrTask = $child.StandardError.ReadToEndAsync()
    $child.WaitForExit()
    $stdout = $stdoutTask.GetAwaiter().GetResult()
    $stderr = $stderrTask.GetAwaiter().GetResult()
    $code = $child.ExitCode
    $child.Dispose()
    $record = [ordered]@{ executable = $Executable; arguments = $Arguments; exit = $code; stdout = $stdout; stderr = $stderr }
    $line = $record | ConvertTo-Json -Compress
    [IO.File]::AppendAllText($smokeLog, "$line`n", $utf8)
    Write-Output $line
    if ($code -ne $ExpectedExit -or $stdout.Replace("`r`n", "`n") -cne $ExpectedStdout) {
        throw 'Unexpected command exit or stdout; see smoke log'
    }
    if ($ExpectedError) {
        if (-not $stderr.Contains($ExpectedError)) { throw 'Expected diagnostic missing' }
    } elseif ($stderr -ne '') {
        throw 'Unexpected stderr; see smoke log'
    }
}

Invoke-SmokeCommand $verslotBinary @('list') ''
Invoke-SmokeCommand $verslotBinary @('current') ''
if (Test-Path -LiteralPath $smokeRoot) { throw 'Empty queries created storage' }
$hashes = @{}
foreach ($version in @('22.0.0', '24.0.0')) {
    Invoke-SmokeCommand $verslotBinary @('install', "node@$version") "installed node@$version`n"
    $archive = "node-v$version-win-x64.zip"
    $checksumUrl = "https://nodejs.org/dist/v$version/SHASUMS256.txt"
    $checksumPath = Join-Path $logDirectory "SHASUMS256-$version.txt"
    Invoke-WebRequest -Uri $checksumUrl -OutFile $checksumPath
    $matching = @(Get-Content -LiteralPath $checksumPath | Where-Object { $_ -match ('^[a-f0-9]{64}  ' + [regex]::Escape($archive) + '$') })
    if ($matching.Count -ne 1) { throw "Missing or ambiguous official checksum for $archive" }
    $digest = $matching[0].Substring(0, 64)
    $installation = Join-Path $smokeRoot "installs/node/$version"
    $receipt = Join-Path $installation '.verslot-install'
    $expectedReceipt = "verslot-install-v1`nnode@$version`n$archive`n$digest`n"
    if ([IO.File]::ReadAllText($receipt) -cne $expectedReceipt) { throw 'Receipt differs from official checksum' }
    $hashes[$version] = @{
        receipt = (Get-FileHash -LiteralPath $receipt -Algorithm SHA256).Hash
        executable = (Get-FileHash -LiteralPath (Join-Path $installation 'node.exe') -Algorithm SHA256).Hash
    }
    [IO.File]::AppendAllText($smokeLog, "official=https://nodejs.org/dist/v$version/$archive checksum=$checksumUrl sha256=$digest receiptHash=$($hashes[$version].receipt) executableHash=$($hashes[$version].executable)`n", $utf8)
    Invoke-SmokeCommand $verslotBinary @('current') ''
}
Invoke-SmokeCommand $verslotBinary @('list') "node@22.0.0`nnode@24.0.0`n"
$entryDirectory = Join-Path $smokeRoot 'current/node'
$entryExecutable = Join-Path $entryDirectory 'node.exe'
$env:PATH = "$entryDirectory;$originalSmokePath"
foreach ($version in @('22.0.0', '24.0.0')) {
    Invoke-SmokeCommand $verslotBinary @('use', "node@$version") "using node@$version`n"
    Invoke-SmokeCommand $verslotBinary @('current') "node@$version`n"
    Invoke-SmokeCommand $entryExecutable @('--version') "v$version`n"
    $resolvedNode = (Get-Command node -CommandType Application | Select-Object -First 1).Source
    [IO.File]::AppendAllText($smokeLog, "resolvedNode=$resolvedNode`n", $utf8)
    if ($resolvedNode -ine $entryExecutable) { throw 'PATH did not resolve the fixed entry first' }
    Invoke-SmokeCommand (Join-Path $env:SystemRoot 'System32/where.exe') @('node') ((& (Join-Path $env:SystemRoot 'System32/where.exe') node | ForEach-Object { "$_`n" }) -join '')
    Invoke-SmokeCommand 'node.exe' @('--version') "v$version`n"
}
Invoke-SmokeCommand $verslotBinary @('use', 'node@24.0.0') "already using node@24.0.0`n"
$competingDirectory = Join-Path $smokeRoot 'installs/node/22.0.0'
$env:PATH = "$competingDirectory;$entryDirectory;$originalSmokePath"
$competingNode = (Get-Command node -CommandType Application | Select-Object -First 1).Source
if ($competingNode -ine (Join-Path $competingDirectory 'node.exe')) { throw 'Competing PATH fixture did not resolve A' }
[IO.File]::AppendAllText($smokeLog, "competingNode=$competingNode`n", $utf8)
Invoke-SmokeCommand 'node.exe' @('--version') "v22.0.0`n"
Invoke-SmokeCommand $verslotBinary @('current') "node@24.0.0`n"
$env:PATH = "$entryDirectory;$originalSmokePath"
Invoke-SmokeCommand 'node.exe' @('--version') "v24.0.0`n"
foreach ($version in @('22.0.0', '24.0.0')) {
    $installation = Join-Path $smokeRoot "installs/node/$version"
    if ((Get-FileHash -LiteralPath (Join-Path $installation '.verslot-install') -Algorithm SHA256).Hash -ne $hashes[$version].receipt) { throw "Receipt changed for $version" }
    if ((Get-FileHash -LiteralPath (Join-Path $installation 'node.exe') -Algorithm SHA256).Hash -ne $hashes[$version].executable) { throw "Executable changed for $version" }
}
[IO.File]::AppendAllText($smokeLog, "A and B receipt/executable hashes unchanged after switching and no-op.`n", $utf8)
Invoke-SmokeCommand $verslotBinary @('uninstall', 'node@24.0.0') '' 1 'cannot uninstall current version: node@24.0.0'
Invoke-SmokeCommand $verslotBinary @('uninstall', 'node@22.0.0') "uninstalled node@22.0.0`n"
Invoke-SmokeCommand $verslotBinary @('list') "node@24.0.0`n"
Invoke-SmokeCommand $verslotBinary @('current') "node@24.0.0`n"
Invoke-SmokeCommand $entryExecutable @('--version') "v24.0.0`n"
Invoke-SmokeCommand 'node.exe' @('--version') "v24.0.0`n"
if (Test-Path -LiteralPath (Join-Path $smokeRoot 'installs/node/22.0.0')) { throw 'Inactive version remains installed' }
$remaining = Join-Path $smokeRoot 'installs/node/24.0.0'
if ((Get-FileHash -LiteralPath (Join-Path $remaining '.verslot-install') -Algorithm SHA256).Hash -ne $hashes['24.0.0'].receipt) { throw 'B receipt changed' }
if ((Get-FileHash -LiteralPath (Join-Path $remaining 'node.exe') -Algorithm SHA256).Hash -ne $hashes['24.0.0'].executable) { throw 'B executable changed' }
foreach ($name in @('.node-next', '.node-previous')) {
    if (Get-Item -LiteralPath (Join-Path $smokeRoot "current/$name") -Force -ErrorAction SilentlyContinue) { throw 'Reserved switch residue remains' }
}
if (@(Get-ChildItem -LiteralPath (Join-Path $smokeRoot 'tmp') -Force).Count -ne 0) { throw 'Temporary operations remain' }
[IO.File]::AppendAllText($smokeLog, "PASS: B selected/executable; A absent; B receipt/executable unchanged; tmp empty; no reserved siblings. Owned fixture retained for evidence; no cleanup or user PATH changes.`n", $utf8)
Write-Output "SMOKE PASS: $smokeBase"
