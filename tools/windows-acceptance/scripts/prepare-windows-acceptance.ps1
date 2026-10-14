[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$OutputDirectory,
    [Parameter(Mandatory = $true)][ValidatePattern('^[a-f0-9]{40}$')][string]$Commit,
    [string]$CandidateDirectory,
    [string]$WorkerDirectory,
    [ValidatePattern('^$|^https://github\.com/luckysus/Suna/actions/runs/[0-9]+$')][string]$ActionsUrl = ''
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'lib/acceptance-evidence.ps1')

# This tool inspects only explicitly selected artifacts. No app, installer,
# Worker, credential store, private database or external service is started.
try {
    Assert-LocalAcceptanceDirectory $OutputDirectory -MayBeMissing
    $directory = [System.IO.Path]::GetFullPath($OutputDirectory)
    if (Test-Path -LiteralPath $directory) { throw 'acceptance_output_already_exists' }
    $candidate = [ordered]@{ state = 'not-provided' }
    $worker = [ordered]@{ state = 'not-provided' }
    if ($CandidateDirectory) { $candidate = Test-CandidateEvidence $CandidateDirectory $Commit }
    if ($WorkerDirectory) { $worker = Test-WorkerEvidence $WorkerDirectory }
    $report = [ordered]@{
        schemaVersion = 'windows-native-acceptance.v1'; commit = $Commit; actionsUrl = $ActionsUrl
        ciConclusion = 'not-verified-by-this-tool'; createdAtUtc = [DateTime]::UtcNow.ToString('o')
        platform = [System.Environment]::OSVersion.Platform.ToString(); powershellVersion = $PSVersionTable.PSVersion.ToString()
        candidate = $candidate; worker = $worker; cases = @(New-AcceptanceCases)
        stableReleaseAccepted = $false; containsCredentials = $false; containsOriginalData = $false
    }
    [System.IO.Directory]::CreateDirectory($directory) | Out-Null
    $json = $report | ConvertTo-Json -Depth 12
    $lines = @('# Suna native acceptance record', '', "Commit: $Commit", "Actions: $ActionsUrl",
        '', 'Artifact hashes are not installation or native acceptance.',
        'Record actual evidence manually; never turn unrun cases into PASS.', '',
        '| ID | Result | Category | Evidence / issue |', '|---|---|---|---|')
    foreach ($case in $report.cases) { $lines += "| $($case.id) | $($case.result) | $($case.category) | $($case.reason) |" }
    foreach ($file in @(@{name='acceptance.json';body=$json}, @{name='acceptance.md';body=($lines -join "`n")})) {
        $stream = [System.IO.File]::Open((Join-Path $directory $file.name), [System.IO.FileMode]::CreateNew)
        try { $bytes = [System.Text.UTF8Encoding]::new($false).GetBytes($file.body); $stream.Write($bytes,0,$bytes.Length); $stream.Flush() }
        finally { $stream.Dispose() }
    }
    Write-Output 'Acceptance templates prepared; all native cases remain NOT_RUN/BLOCKED.'
    $global:LASTEXITCODE = 0
} catch {
    # Avoid exposing upstream filesystem exceptions, usernames or selected paths.
    $code = [string]$_.Exception.Message
    if ($code -notmatch '^acceptance_[a-z_]+$') { $code = 'acceptance_preparation_failed' }
    Write-Error $code
    exit 1
}
