$ErrorActionPreference = 'Stop'
$draftScript = Join-Path $PSScriptRoot 'create-release-draft.ps1'
$parent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar, [IO.Path]::AltDirectorySeparatorChar)
$scratch = [IO.Path]::GetFullPath((Join-Path $parent "verslot-release-test-$([guid]::NewGuid().ToString('N'))"))
if ([IO.Path]::GetDirectoryName($scratch) -cne $parent) { throw 'Invalid test directory' }

$global:releaseTest = @{
    commit = 'a' * 40
    tagObject = 'b' * 40
    ciConclusion = 'success'
    release = $null
    assetBytes = @{}
    calls = [Collections.Generic.List[string]]::new()
    uploadsBeforeFailure = -1
}

function global:Invoke-WebRequest {
    param([string] $Uri, [hashtable] $Headers, [switch] $SkipHttpErrorCheck)
    if ($Uri -match '/git/ref/tags/v1\.2\.3$') {
        $data = @{ object = @{ type = 'tag'; sha = $global:releaseTest.tagObject } }
    } elseif ($Uri -match '/git/tags/') {
        $data = @{ object = @{ type = 'commit'; sha = $global:releaseTest.commit } }
    } elseif ($Uri -match '/actions/workflows/ci\.yml/runs\?') {
        $data = @{ workflow_runs = @(@{
            head_sha = $global:releaseTest.commit
            head_branch = 'main'
            event = 'push'
            status = 'completed'
            conclusion = $global:releaseTest.ciConclusion
            html_url = 'https://github.test/ci/1'
        }) }
    } elseif ($Uri -match '/releases\?per_page=100&page=1$') {
        $data = @()
        if ($null -ne $global:releaseTest.release) { $data += $global:releaseTest.release }
    } else {
        throw "Unexpected API request: $Uri"
    }
    [pscustomobject]@{ StatusCode = 200; Content = (ConvertTo-Json -InputObject $data -Depth 10 -Compress) }
}

function global:gh {
    $arguments = @($args)
    $global:releaseTest.calls.Add(($arguments[0..1] -join ' '))
    $global:LASTEXITCODE = 0
    if ($arguments[1] -ceq 'create') {
        $notesIndex = [array]::IndexOf($arguments, '--notes-file')
        $global:releaseTest.release = @{
            tag_name = $arguments[2]
            draft = $true
            body = [IO.File]::ReadAllText($arguments[$notesIndex + 1])
            assets = @()
        }
    } elseif ($arguments[1] -ceq 'upload') {
        $repoIndex = [array]::IndexOf($arguments, '--repo')
        foreach ($file in $arguments[3..($repoIndex - 1)]) {
            if ($global:releaseTest.uploadsBeforeFailure -eq 0) {
                $global:LASTEXITCODE = 1
                return
            }
            $name = [IO.Path]::GetFileName($file)
            $global:releaseTest.assetBytes[$name] = [IO.File]::ReadAllBytes($file)
            $global:releaseTest.release.assets += @{ name = $name }
            if ($global:releaseTest.uploadsBeforeFailure -gt 0) { $global:releaseTest.uploadsBeforeFailure-- }
        }
    } elseif ($arguments[1] -ceq 'download') {
        $name = $arguments[[array]::IndexOf($arguments, '--pattern') + 1]
        $directory = $arguments[[array]::IndexOf($arguments, '--dir') + 1]
        [IO.File]::WriteAllBytes((Join-Path $directory $name), $global:releaseTest.assetBytes[$name])
    } else {
        throw "Unexpected gh command: $arguments"
    }
}

function Expect-DraftFailure([string] $message) {
    $failed = $false
    try {
        & $draftScript
    } catch {
        if (-not $_.Exception.Message.Contains($message)) { throw }
        $failed = $true
    }
    if (-not $failed) { throw "Expected failure: $message" }
}

try {
    New-Item -ItemType Directory -Path $scratch | Out-Null
    $env:RUNNER_TEMP = $scratch
    $env:GITHUB_REF_TYPE = 'tag'
    $env:GITHUB_REF_NAME = 'v1.2.3'
    $env:GITHUB_SHA = $global:releaseTest.commit
    $env:GITHUB_API_URL = 'https://api.github.test'
    $env:GITHUB_SERVER_URL = 'https://github.test'
    $env:GITHUB_REPOSITORY = 'verslot/verslot'
    $env:GITHUB_RUN_ID = '12345'
    $env:GH_TOKEN = 'test-token'
    $targets = [ordered]@{
        'x86_64-pc-windows-msvc' = 'zip'
        'aarch64-apple-darwin' = 'tar.gz'
        'x86_64-unknown-linux-gnu' = 'tar.gz'
    }
    foreach ($target in $targets.Keys) {
        $directory = Join-Path $scratch "verslot-artifacts/verslot-$target"
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        $name = "verslot-v1.2.3-$target.$($targets[$target])"
        $archive = Join-Path $directory $name
        [IO.File]::WriteAllText($archive, "fixture-$target")
        $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        [IO.File]::WriteAllText("$archive.sha256", "$digest  $name`n")
        [IO.File]::WriteAllText((Join-Path $directory 'build-info.txt'), "commit=$env:GITHUB_SHA`nversion=1.2.3`ntarget=$target`n")
    }

    $windows = Join-Path $scratch 'verslot-artifacts/verslot-x86_64-pc-windows-msvc/verslot-v1.2.3-x86_64-pc-windows-msvc.zip'
    [IO.File]::AppendAllText($windows, 'corrupt')
    Expect-DraftFailure 'Checksum mismatch'
    [IO.File]::WriteAllText($windows, 'fixture-x86_64-pc-windows-msvc')
    if ($global:releaseTest.calls.Count -ne 0) { throw 'Corrupt artifacts reached the Release' }

    $evidencePath = Join-Path (Split-Path $windows) 'build-info.txt'
    $evidence = [IO.File]::ReadAllText($evidencePath)
    [IO.File]::WriteAllText($evidencePath, $evidence.Replace('version=1.2.3', 'version=1.2.4'))
    Expect-DraftFailure 'Build evidence does not match'
    [IO.File]::WriteAllText($evidencePath, $evidence)
    $global:releaseTest.commit = 'c' * 40
    Expect-DraftFailure 'remote tag no longer points'
    $global:releaseTest.commit = $env:GITHUB_SHA
    if ($global:releaseTest.calls.Count -ne 0) { throw 'Wrong commit or version reached the Release' }

    $global:releaseTest.ciConclusion = 'failure'
    Expect-DraftFailure 'No successful main CI run'
    $global:releaseTest.ciConclusion = 'success'
    if ($global:releaseTest.calls.Count -ne 0) { throw 'Failed CI reached the Release' }

    $global:releaseTest.uploadsBeforeFailure = 2
    Expect-DraftFailure 'Could not upload all Release assets'
    if ($global:releaseTest.release.assets.Count -ne 2) { throw 'Expected a partial draft' }
    $global:releaseTest.uploadsBeforeFailure = -1
    [IO.File]::AppendAllText($evidencePath, 'changed runner')
    Expect-DraftFailure 'different artifacts or a different workflow run'
    [IO.File]::WriteAllText($evidencePath, $evidence)
    & $draftScript
    if (-not $global:releaseTest.release.draft -or $global:releaseTest.release.assets.Count -ne 6) {
        throw 'Retry did not complete the six draft assets'
    }
    if (@($global:releaseTest.calls | Where-Object { $_ -ceq 'release create' }).Count -ne 1) {
        throw 'Retry created another draft'
    }

    $global:releaseTest.release.draft = $false
    $callCount = $global:releaseTest.calls.Count
    Expect-DraftFailure 'not a draft'
    if ($global:releaseTest.calls.Count -ne $callCount) { throw 'Published Release was modified' }

    $global:releaseTest.release.draft = $true
    $firstName = $global:releaseTest.release.assets[0].name
    $global:releaseTest.assetBytes[$firstName] = [Text.Encoding]::UTF8.GetBytes('different')
    $callCount = @($global:releaseTest.calls | Where-Object { $_ -ceq 'release upload' }).Count
    Expect-DraftFailure 'Existing asset differs'
    if (@($global:releaseTest.calls | Where-Object { $_ -ceq 'release upload' }).Count -ne $callCount) {
        throw 'Mismatched draft asset was overwritten'
    }
    Write-Output 'Draft release tests passed.'
} finally {
    if (Test-Path -LiteralPath $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
    Remove-Item Function:\Invoke-WebRequest -ErrorAction SilentlyContinue
    Remove-Item Function:\gh -ErrorAction SilentlyContinue
    Remove-Variable releaseTest -Scope Global -ErrorAction SilentlyContinue
}
