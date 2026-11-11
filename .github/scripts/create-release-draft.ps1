$ErrorActionPreference = 'Stop'

function Get-GitHubResponse([string] $path) {
    $headers = @{
        Authorization = "Bearer $env:GH_TOKEN"
        Accept = 'application/vnd.github+json'
    }
    Invoke-WebRequest -Uri "$env:GITHUB_API_URL/repos/$env:GITHUB_REPOSITORY/$path" `
        -Headers $headers -SkipHttpErrorCheck
}

function Get-GitHubObject([string] $path) {
    $response = Get-GitHubResponse $path
    if ($response.StatusCode -ne 200) { throw "GitHub request failed: $path ($($response.StatusCode))" }
    $response.Content | ConvertFrom-Json
}

function Get-ReleaseByTag([string] $tag) {
    $page = 1
    do {
        # GitHub's release-by-tag endpoint does not return drafts.
        $releases = @(Get-GitHubObject "releases?per_page=100&page=$page")
        $matching = @($releases | Where-Object { $_.tag_name -ceq $tag })
        if ($matching.Count -gt 1) { throw "Multiple Releases use $tag" }
        if ($matching.Count -eq 1) { return $matching[0] }
        $page++
    } while ($releases.Count -eq 100)
    return $null
}

$tag = $env:GITHUB_REF_NAME
$commit = $env:GITHUB_SHA
if ($env:GITHUB_REF_TYPE -cne 'tag' -or $tag -cnotmatch '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$') {
    throw 'Expected a stable version tag'
}
if ($commit -cnotmatch '^[0-9a-f]{40}$') { throw 'Expected the tagged commit SHA' }
$version = $tag.Substring(1)
$artifactRoot = Join-Path $env:RUNNER_TEMP 'verslot-artifacts'
$targets = [ordered]@{
    'x86_64-pc-windows-msvc' = 'zip'
    'aarch64-apple-darwin' = 'tar.gz'
    'x86_64-unknown-linux-gnu' = 'tar.gz'
}
$files = [ordered]@{}
$buildEvidence = [ordered]@{}
$artifactDigests = @()
foreach ($target in $targets.Keys) {
    $directory = Join-Path $artifactRoot "verslot-$target"
    $archiveName = "verslot-$tag-$target.$($targets[$target])"
    $archive = Join-Path $directory $archiveName
    $checksumPath = "$archive.sha256"
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    $checksum = [IO.File]::ReadAllText($checksumPath).TrimEnd("`r", "`n")
    if ($checksum -cne "$digest  $archiveName") { throw "Checksum mismatch for $archiveName" }
    $evidencePath = Join-Path $directory 'build-info.txt'
    $evidence = [IO.File]::ReadAllText($evidencePath)
    $lines = $evidence -split '\r?\n'
    if ($lines -cnotcontains "commit=$commit" -or $lines -cnotcontains "version=$version" -or $lines -cnotcontains "target=$target") {
        throw "Build evidence does not match $tag for $target"
    }
    $files[$archiveName] = $archive
    $files["$archiveName.sha256"] = $checksumPath
    $buildEvidence[$target] = $evidence.TrimEnd()
    $evidenceDigest = (Get-FileHash -LiteralPath $evidencePath -Algorithm SHA256).Hash.ToLowerInvariant()
    $artifactDigests += "$digest/$evidenceDigest"
}

$tagObject = (Get-GitHubObject "git/ref/tags/$tag").object
if ($tagObject.type -ceq 'tag') {
    $tagObject = (Get-GitHubObject "git/tags/$($tagObject.sha)").object
}
if ($tagObject.type -cne 'commit' -or $tagObject.sha -cne $commit) {
    throw 'The remote tag no longer points to the workflow commit'
}
$runs = Get-GitHubObject "actions/workflows/ci.yml/runs?head_sha=$commit&event=push&per_page=100"
$ci = @($runs.workflow_runs | Where-Object {
    $_.head_sha -ceq $commit -and $_.head_branch -ceq 'main' -and
    $_.event -ceq 'push' -and $_.status -ceq 'completed' -and $_.conclusion -ceq 'success'
} | Select-Object -First 1)
if ($ci.Count -ne 1) { throw 'No successful main CI run was found for the tagged commit' }

$runMarker = "<!-- verslot-release-run:$env:GITHUB_RUN_ID $($artifactDigests -join ' ') -->"
$fence = '```'
$notes = @(
    "## Verslot $tag"
    ''
    "Changes: $env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/blob/$commit/CHANGELOG.md"
    "Commit: $commit"
    "CI: $($ci[0].html_url)"
    "Release build: $env:GITHUB_SERVER_URL/$env:GITHUB_REPOSITORY/actions/runs/$env:GITHUB_RUN_ID"
    $runMarker
    ''
    '## Build environments and runtime dependencies'
)
foreach ($target in $targets.Keys) {
    $notes += @('', "<details><summary>$target</summary>", '', "${fence}text", $buildEvidence[$target], $fence, '</details>')
}
$notes += @('', 'Record the independently tested environments before publishing this draft.', '')
$notesPath = Join-Path $env:RUNNER_TEMP 'verslot-release-notes.md'
[IO.File]::WriteAllText($notesPath, ($notes -join "`n"), [Text.UTF8Encoding]::new($false))

$release = Get-ReleaseByTag $tag
if ($null -eq $release) {
    & gh release create $tag --repo $env:GITHUB_REPOSITORY --draft --verify-tag --title "Verslot $tag" --notes-file $notesPath
    if ($LASTEXITCODE -ne 0) { throw 'Could not create the Release draft' }
    # A new draft may not appear in the releases list immediately.
    for ($attempt = 0; $attempt -lt 10; $attempt++) {
        $release = Get-ReleaseByTag $tag
        if ($null -ne $release) { break }
        if ($attempt -lt 9) { Start-Sleep -Seconds 1 }
    }
}
if ($null -eq $release -or -not $release.draft -or $release.tag_name -cne $tag) {
    throw 'The Release is not a draft for this tag'
}
if (-not $release.body -or -not $release.body.Contains($runMarker)) {
    throw 'The existing draft belongs to different artifacts or a different workflow run'
}
$existingNames = @($release.assets | Where-Object { $null -ne $_ } | ForEach-Object { $_.name })
if (@($existingNames | Where-Object { -not $files.Contains($_) }).Count -ne 0) {
    throw 'The draft contains unexpected assets'
}
$downloadDirectory = Join-Path $env:RUNNER_TEMP "verslot-existing-assets-$([guid]::NewGuid().ToString('N'))"
New-Item -ItemType Directory -Path $downloadDirectory | Out-Null
foreach ($name in $existingNames) {
    & gh release download $tag --repo $env:GITHUB_REPOSITORY --pattern $name --dir $downloadDirectory
    if ($LASTEXITCODE -ne 0) { throw "Could not download existing asset $name" }
    $existingHash = (Get-FileHash -LiteralPath (Join-Path $downloadDirectory $name) -Algorithm SHA256).Hash
    $expectedHash = (Get-FileHash -LiteralPath $files[$name] -Algorithm SHA256).Hash
    if ($existingHash -cne $expectedHash) { throw "Existing asset differs from this run: $name" }
}
$missing = @($files.Keys | Where-Object { $existingNames -cnotcontains $_ } | ForEach-Object { $files[$_] })
if ($missing.Count -ne 0) {
    $release = Get-ReleaseByTag $tag
    if ($null -eq $release -or -not $release.draft) { throw 'The Release is no longer a draft' }
    & gh release upload $tag @missing --repo $env:GITHUB_REPOSITORY
    if ($LASTEXITCODE -ne 0) { throw 'Could not upload all Release assets' }
}
$release = Get-ReleaseByTag $tag
$finalNames = @($release.assets | Where-Object { $null -ne $_ } | ForEach-Object { $_.name })
if (-not $release.draft -or $finalNames.Count -ne 6 -or @($files.Keys | Where-Object { $finalNames -cnotcontains $_ }).Count -ne 0) {
    throw 'The draft does not contain the expected six assets'
}
Write-Output "Draft Release $tag is ready for manual review."
