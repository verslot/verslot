param([string]$Source, [string]$Executable, [string]$Shim, [string]$Directory)
$ErrorActionPreference = 'Stop'
# Windows PowerShell's provider cannot resolve Rust's extended-length prefix.
function Convert-TestPath([string]$Path) {
    if ($Path.StartsWith('\\?\')) { return $Path.Substring(4) }
    return $Path
}
$Executable = Convert-TestPath $Executable
$Shim = Convert-TestPath $Shim
$Directory = Convert-TestPath $Directory
Add-Type -Path $Source -OutputAssembly $Executable -OutputType ConsoleApplication
$wrapper = $null
try {
    $wrapper = Start-Process -FilePath $Shim -WorkingDirectory $Directory -WindowStyle Hidden -PassThru
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while (!(Test-Path -LiteralPath (Join-Path $Directory 'interrupted'))) {
        if ($wrapper.HasExited) { throw "Shim exited before receiving Ctrl+C: $($wrapper.ExitCode)" }
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Timeout waiting for Ctrl+C' }
        Start-Sleep -Milliseconds 10
    }
    if ($wrapper.HasExited) { throw 'Shim exited before tool cleanup' }
    [IO.File]::WriteAllText((Join-Path $Directory 'wrapper-alive'), 'waiting')
    while (!(Test-Path -LiteralPath (Join-Path $Directory 'release'))) {
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Timeout waiting for cleanup release' }
        Start-Sleep -Milliseconds 10
    }
    if (!$wrapper.WaitForExit(15000)) { throw 'Shim did not finish after tool cleanup' }
    if ($wrapper.ExitCode -ne 37) { throw "Unexpected shim exit: $($wrapper.ExitCode)" }
} finally {
    if ($wrapper -and !$wrapper.HasExited) { Stop-Process -Id $wrapper.Id -Force -ErrorAction SilentlyContinue }
    $ready = Join-Path $Directory 'ready'
    if (Test-Path -LiteralPath $ready) {
        $toolPid = [int][IO.File]::ReadAllText($ready)
        Stop-Process -Id $toolPid -Force -ErrorAction SilentlyContinue
    }
}
