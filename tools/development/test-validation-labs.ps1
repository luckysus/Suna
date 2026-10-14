[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$oldTarget = $env:CARGO_TARGET_DIR
try {
    $env:CARGO_TARGET_DIR = Join-Path $repoRoot 'src-tauri/target'
    foreach ($module in @('research-workflow', 'research-package')) {
        $manifest = Join-Path $PSScriptRoot "$module/Cargo.toml"
        & cargo test --locked --manifest-path $manifest --lib --jobs 1 -- --test-threads=1
        if ($LASTEXITCODE -ne 0) { throw "Isolated Rust validation failed: $module" }
    }
    & node (Join-Path $PSScriptRoot 'test-workflow-ui.mjs')
    if ($LASTEXITCODE -ne 0) { throw 'Isolated frontend validation failed' }
    $python = $env:SUNA_COMPUTE_WORKER_PYTHON
    if (-not $python) {
        $pythonRelative = if ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT) { 'compute-worker/.venv/Scripts/python.exe' } else { 'compute-worker/.venv/bin/python' }
        $python = Join-Path $repoRoot $pythonRelative
    }
    if (-not (Test-Path -LiteralPath $python -PathType Leaf)) { throw 'Install the locked compute-worker test environment first' }
    & $python -m pytest -q (Join-Path $PSScriptRoot 'research-package-reference')
    if ($LASTEXITCODE -ne 0) { throw 'Archive protocol reference validation failed' }
    & (Join-Path $repoRoot 'tools/windows-acceptance/scripts/tests/acceptance-evidence.contract.ps1')
} finally {
    $env:CARGO_TARGET_DIR = $oldTarget
}
