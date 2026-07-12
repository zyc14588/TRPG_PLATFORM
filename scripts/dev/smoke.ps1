$ErrorActionPreference = "Stop"

$python = if (Get-Command python -ErrorAction SilentlyContinue) { "python" } else { "python3" }
$report = Join-Path ([System.IO.Path]::GetTempPath()) "coc-ai-trpg-release-readiness.json"

& $python scripts/ci/release_readiness.py --report $report --require-ready
if ($LASTEXITCODE -ne 0) {
    throw "Release readiness is BLOCKED; see $report"
}

$challenge = [guid]::NewGuid().ToString("N")
$response = Invoke-RestMethod `
    -Uri "http://localhost:8080/healthz" `
    -Headers @{ "X-Smoke-Challenge" = $challenge } `
    -TimeoutSec 15

if ($response.placeholder -eq $true -or $response.status -ne "ready") {
    throw "Smoke rejected placeholder or non-ready response"
}
if ($response.challenge -ne $challenge -or [string]::IsNullOrWhiteSpace($response.provenance)) {
    throw "Smoke requires a dynamic challenge and provenance"
}
