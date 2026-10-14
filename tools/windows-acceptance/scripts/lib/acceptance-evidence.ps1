Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Assert-LocalAcceptanceDirectory {
    param([string]$Directory, [switch]$MayBeMissing)
    # IsPathRooted accepts drive-relative paths such as \folder on Windows.
    if ([System.IO.Path]::DirectorySeparatorChar -eq '\' -and
        $Directory -notmatch '^[a-zA-Z]:[\\/]') { throw 'acceptance_directory_not_local_absolute' }
    if (-not [System.IO.Path]::IsPathRooted($Directory) -or $Directory -match '^(\\\\|//|\\Device\\)' -or
        $Directory -match '^[a-zA-Z]:($|[^\\/])' -or
        $Directory -match '(^|[\\/])\.\.([\\/]|$)') { throw 'acceptance_directory_not_local_absolute' }
    $tail = $Directory
    if ($tail -match '^[a-zA-Z]:[\\/]') { $tail = $tail.Substring(2) }
    if ($tail.Contains(':')) { throw 'acceptance_directory_not_local_absolute' }
    $cursor = [System.IO.Path]::GetFullPath($Directory)
    if (-not $MayBeMissing -and -not [System.IO.Directory]::Exists($cursor)) { throw 'acceptance_directory_missing' }
    while ($cursor) {
        if (Test-Path -LiteralPath $cursor) {
            $item = Get-Item -LiteralPath $cursor -ErrorAction Stop
            if (-not $item.PSIsContainer -or ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
                throw 'acceptance_directory_not_regular'
            }
        }
        $cursor = [System.IO.Path]::GetDirectoryName($cursor)
    }
}

function Get-SafeAcceptanceFile {
    param([string]$Directory, [string]$Name)
    if ([string]::IsNullOrWhiteSpace($Name) -or $Name -match '[\\/:]' -or
        $Name -match '[<>|?*\x00-\x1f]' -or $Name -match '[. ]$' -or
        $Name -match '^(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)' -or
        $Name -in @('.', '..') -or [System.IO.Path]::GetFileName($Name) -ne $Name) {
        throw 'acceptance_unsafe_artifact_name'
    }
    $path = Join-Path $Directory $Name
    if (-not (Test-Path -LiteralPath $path)) { throw 'acceptance_artifact_missing' }
    $item = Get-Item -LiteralPath $path -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint)) {
        throw 'acceptance_artifact_not_regular_file'
    }
    return $item
}

function Read-BoundedAcceptanceJson {
    param([string]$Directory, [string]$Name)
    $file = Get-SafeAcceptanceFile $Directory $Name
    if ($file.Length -gt 1024 * 1024) { throw 'acceptance_manifest_too_large' }
    $reader = [System.IO.StreamReader]::new($file.FullName)
    try {
        $buffer = [char[]]::new(1024 * 1024 + 1)
        $count = $reader.ReadBlock($buffer, 0, $buffer.Length)
        if ($count -gt 1024 * 1024) { throw 'acceptance_manifest_too_large' }
        return ([string]::new($buffer,0,$count) | ConvertFrom-Json)
    }
    catch {
        if ($_.Exception.Message -eq 'acceptance_manifest_too_large') { throw }
        throw 'acceptance_invalid_manifest_json'
    }
    finally { $reader.Dispose() }
}

function Test-CandidateEvidence {
    param([string]$Directory, [string]$ExpectedCommit)
    Assert-LocalAcceptanceDirectory $Directory
    $manifest = Read-BoundedAcceptanceJson $Directory 'release-manifest.json'
    if ([string]$manifest.product -ne 'Suna' -or
        [string]$manifest.commit -notmatch '^[a-f0-9]{40}$' -or
        [string]$manifest.commit -ne $ExpectedCommit -or
        [string]$manifest.version -notmatch '^\d+\.\d+\.\d+$' -or
        [string]$manifest.signing -notin @('signed', 'unsigned')) {
        throw 'acceptance_release_identity_mismatch'
    }
    $entries = @($manifest.artifacts)
    if ($entries.Count -lt 1 -or $entries.Count -gt 512) { throw 'acceptance_artifact_count_invalid' }
    $seen = @{}
    $totalBytes = [long]0
    $items = foreach ($entry in $entries) {
        $name = [string]$entry.name
        if ($seen.ContainsKey($name.ToLowerInvariant())) { throw 'acceptance_duplicate_artifact' }
        $seen[$name.ToLowerInvariant()] = $true
        $file = Get-SafeAcceptanceFile $Directory $name
        $totalBytes += $file.Length
        if ($file.Length -gt 2GB -or $totalBytes -gt 8GB) { throw 'acceptance_artifact_size_limit' }
        if ($file.Length -le 0 -or [string]$entry.sha256 -notmatch '^[a-fA-F0-9]{64}$' -or
            [long]$entry.bytes -ne $file.Length -or
            (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash -ne [string]$entry.sha256) {
            throw 'acceptance_artifact_hash_mismatch'
        }
        [ordered]@{ label = "artifact-$($seen.Count)"; bytes = $file.Length; sha256 = ([string]$entry.sha256).ToLowerInvariant() }
    }
    return [ordered]@{
        state = 'hashes-verified'; commit = $ExpectedCommit; version = [string]$manifest.version
        claimedSigning = [string]$manifest.signing; authenticode = 'pending-native'
        artifacts = @($items); executableStarted = $false; installerRun = $false
    }
}

function Test-WorkerEvidence {
    param([string]$Directory)
    Assert-LocalAcceptanceDirectory $Directory
    $manifest = Read-BoundedAcceptanceJson $Directory 'worker-artifact-manifest.json'
    if ([string]$manifest.schema_version -ne '1.0.0' -or
        [string]$manifest.artifact -ne 'suna-compute-worker' -or
        [string]$manifest.executable -ne 'suna-compute-worker.exe') { throw 'acceptance_worker_identity_mismatch' }
    $file = Get-SafeAcceptanceFile $Directory 'suna-compute-worker.exe'
    if ($file.Length -gt 2GB) { throw 'acceptance_artifact_size_limit' }
    if ($file.Length -le 0 -or [string]$manifest.sha256 -notmatch '^[a-fA-F0-9]{64}$' -or
        (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash -ne [string]$manifest.sha256) {
        throw 'acceptance_worker_hash_mismatch'
    }
    return [ordered]@{
        state = 'hashes-verified'; sha256 = ([string]$manifest.sha256).ToLowerInvariant()
        protocol = 'pending-native'; dependencies = 'pending-native'; executableStarted = $false
    }
}

function New-AcceptanceCases {
    return @(1..13 | ForEach-Object {
        $external = $_ -in @(8, 9)
        [ordered]@{ id = ('W{0:D2}' -f $_); result = $(if ($external) { 'BLOCKED' } else { 'NOT_RUN' })
            category = $(if ($external) { 'pending-network' } else { 'pending-native' })
            evidence = @(); reason = $(if ($external) { 'authorized-service-not-verified' } else { 'native-acceptance-not-executed' }) }
    })
}
