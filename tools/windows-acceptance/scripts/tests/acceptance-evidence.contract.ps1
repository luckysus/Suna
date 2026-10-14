[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $root 'scripts/lib/acceptance-evidence.ps1')
$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ('suna-acceptance-contract-' + [Guid]::NewGuid().ToString('N'))
$commit = 'a' * 40
$passed = 0
function Assert-Contract { param([bool]$Value, [string]$Name); if (-not $Value) { throw $Name }; $script:passed++ }
function Assert-Rejected {
    param([scriptblock]$Operation, [string]$Code)
    $errorMessage = $null
    try { & $Operation | Out-Null } catch { $errorMessage = $_.Exception.Message }
    Assert-Contract ($null -ne $errorMessage -and $errorMessage -match [regex]::Escape($Code)) $Code
}
try {
    [System.IO.Directory]::CreateDirectory($temporary) | Out-Null
    $package = Join-Path $temporary 'candidate'
    [System.IO.Directory]::CreateDirectory($package) | Out-Null
    $asset = Join-Path $package 'fixture.zip'
    [System.IO.File]::WriteAllText($asset, 'synthetic fixture, not a Windows binary')
    $entry = @{name='fixture.zip';bytes=(Get-Item -LiteralPath $asset).Length;sha256=(Get-FileHash -LiteralPath $asset).Hash.ToLowerInvariant()}
    $manifest = @{product='Suna';version='1.0.0';commit=$commit;signing='unsigned';artifacts=@($entry)}
    $manifestPath = Join-Path $package 'release-manifest.json'
    function Save-Manifest { [System.IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json -Depth 8)) }
    Save-Manifest
    $evidence = Test-CandidateEvidence $package $commit
    Assert-Contract ($evidence.state -eq 'hashes-verified' -and -not $evidence.installerRun -and $evidence.authenticode -eq 'pending-native') 'hash evidence must not claim installation'
    Assert-Contract (-not (($evidence | ConvertTo-Json -Depth 8).Contains($temporary))) 'report must omit absolute paths'
    Assert-Rejected { Test-CandidateEvidence $package ('b'*40) } 'acceptance_release_identity_mismatch'
    [System.IO.File]::AppendAllText($asset, 'tampered')
    Assert-Rejected { Test-CandidateEvidence $package $commit } 'acceptance_artifact_hash_mismatch'
    $entry.bytes=(Get-Item -LiteralPath $asset).Length; $entry.sha256=(Get-FileHash -LiteralPath $asset).Hash.ToLowerInvariant(); Save-Manifest
    $entry.name='../fixture.zip'; Save-Manifest
    Assert-Rejected { Test-CandidateEvidence $package $commit } 'acceptance_unsafe_artifact_name'
    $entry.name='fixture.zip'; $manifest.artifacts=@($entry, $entry); Save-Manifest
    Assert-Rejected { Test-CandidateEvidence $package $commit } 'acceptance_duplicate_artifact'
    $manifest.artifacts=@($entry); Save-Manifest
    Assert-Rejected { Get-SafeAcceptanceFile $package 'fixture.zip:secret' } 'acceptance_unsafe_artifact_name'
    foreach ($unsafeName in @('NUL', 'CON.zip', 'COM1.csv', 'file.zip.', 'file.zip ', 'file?.zip')) {
        Assert-Rejected { Get-SafeAcceptanceFile $package $unsafeName } 'acceptance_unsafe_artifact_name'
    }
    $unicodeAsset = Join-Path $package '中文 空格.zip'
    [System.IO.File]::WriteAllText($unicodeAsset, 'synthetic unicode fixture')
    $unicodeEntry = @{name='中文 空格.zip';bytes=(Get-Item -LiteralPath $unicodeAsset).Length;sha256=(Get-FileHash -LiteralPath $unicodeAsset).Hash.ToLowerInvariant()}
    $manifest.artifacts=@($entry, $unicodeEntry); Save-Manifest
    $unicodeEvidence = Test-CandidateEvidence $package $commit
    Assert-Contract ($unicodeEvidence.artifacts.Count -eq 2 -and $unicodeEvidence.state -eq 'hashes-verified') 'unicode and spaces must retain valid hash evidence'
    Assert-Contract (-not (($unicodeEvidence | ConvertTo-Json -Depth 8).Contains('中文 空格'))) 'selected artifact names must not leak into reports'
    $manifest.artifacts=@($entry); Save-Manifest
    $workerDir=Join-Path $temporary 'worker'; [System.IO.Directory]::CreateDirectory($workerDir) | Out-Null
    Assert-Rejected { Test-WorkerEvidence $workerDir } 'acceptance_artifact_missing'
    $workerExe=Join-Path $workerDir 'suna-compute-worker.exe'; [System.IO.File]::WriteAllText($workerExe, 'fixture, never executed')
    $workerManifest=@{schema_version='1.0.0';artifact='suna-compute-worker';executable='suna-compute-worker.exe';sha256=(Get-FileHash -LiteralPath $workerExe).Hash}
    $workerManifestPath=Join-Path $workerDir 'worker-artifact-manifest.json'; [System.IO.File]::WriteAllText($workerManifestPath,($workerManifest | ConvertTo-Json))
    $worker=Test-WorkerEvidence $workerDir
    Assert-Contract ($worker.protocol -eq 'pending-native' -and -not $worker.executableStarted) 'worker hash must not claim protocol success'
    $workerManifest.schema_version='2.0.0'; [System.IO.File]::WriteAllText($workerManifestPath,($workerManifest | ConvertTo-Json))
    Assert-Rejected { Test-WorkerEvidence $workerDir } 'acceptance_worker_identity_mismatch'
    $workerManifest.schema_version='1.0.0'; [System.IO.File]::WriteAllText($workerManifestPath,($workerManifest | ConvertTo-Json))
    [System.IO.File]::AppendAllText($workerExe,'tampered')
    Assert-Rejected { Test-WorkerEvidence $workerDir } 'acceptance_worker_hash_mismatch'
    Assert-Rejected { Assert-LocalAcceptanceDirectory 'relative/path' } 'acceptance_directory_not_local_absolute'
    Assert-Rejected { Assert-LocalAcceptanceDirectory '\\server\share' } 'acceptance_directory_not_local_absolute'
    Assert-Rejected { Assert-LocalAcceptanceDirectory (Join-Path $temporary '../escape') -MayBeMissing } 'acceptance_directory_not_local_absolute'
    [System.IO.File]::WriteAllText($manifestPath, '{broken json')
    Assert-Rejected { Test-CandidateEvidence $package $commit } 'acceptance_invalid_manifest_json'
    Save-Manifest
    $cases=@(New-AcceptanceCases)
    Assert-Contract ($cases.Count -eq 13 -and @($cases | Where-Object result -eq 'PASS').Count -eq 0) 'unrun native cases must never pass'
    Assert-Contract ($cases[7].result -eq 'BLOCKED' -and $cases[8].result -eq 'BLOCKED') 'external authorization must remain blocked'
    $report=Join-Path $temporary '中文 验收记录'
    & (Join-Path $root 'scripts/prepare-windows-acceptance.ps1') -OutputDirectory $report -Commit $commit -CandidateDirectory $package -ActionsUrl 'https://github.com/luckysus/Suna/actions/runs/38423460599'
    if ($LASTEXITCODE -ne 0) { throw 'report generator failed' }
    $actual=Read-BoundedAcceptanceJson $report 'acceptance.json'
    Assert-Contract (-not $actual.stableReleaseAccepted -and $actual.ciConclusion -eq 'not-verified-by-this-tool' -and $actual.cases.Count -eq 13) 'report status must persist honestly'
    $before=(Get-FileHash -LiteralPath (Join-Path $report 'acceptance.json')).Hash
    # Existing output must remain unchanged, including when invoked separately.
    $pwsh=(Get-Process -Id $PID).Path
    & $pwsh -NoProfile -NonInteractive -File (Join-Path $root 'scripts/prepare-windows-acceptance.ps1') -OutputDirectory $report -Commit $commit 2>$null | Out-Null
    Assert-Contract ($LASTEXITCODE -ne 0 -and (Get-FileHash -LiteralPath (Join-Path $report 'acceptance.json')).Hash -eq $before) 'existing evidence must not be overwritten'
    $global:LASTEXITCODE=0
    Write-Output "Acceptance evidence contract passed ($passed assertions; fixture-only, no native acceptance)."
} finally {
    if ([System.IO.Directory]::Exists($temporary)) { [System.IO.Directory]::Delete($temporary, $true) }
}
