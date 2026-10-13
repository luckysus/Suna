[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$previousReport = $env:SUNA_RESEARCH_BENCHMARK_REPORT
$env:SUNA_RESEARCH_BENCHMARK_REPORT = Join-Path $repoRoot "artifacts/research-benchmark.json"
Push-Location (Join-Path $repoRoot "src-tauri")
try {
    cargo test --locked --test research_benchmark -- --ignored --nocapture
    if ($LASTEXITCODE -ne 0) { throw "科研工作区性能基准失败" }
    Write-Output ("合成基准结果: " + $env:SUNA_RESEARCH_BENCHMARK_REPORT)
}
finally {
    Pop-Location
    $env:SUNA_RESEARCH_BENCHMARK_REPORT = $previousReport
}
