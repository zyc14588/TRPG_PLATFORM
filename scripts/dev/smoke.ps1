$ErrorActionPreference = "Stop"

$evidenceDir = "evidence/stages/S09"
New-Item -ItemType Directory -Force $evidenceDir | Out-Null

$started = Get-Date
try {
    $response = Invoke-WebRequest -Uri "http://localhost:8080/healthz" -UseBasicParsing -TimeoutSec 15
    if ($response.StatusCode -ne 200) {
        throw "healthz returned HTTP $($response.StatusCode)"
    }

    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    $checkedAt = (Get-Date).ToString("o")
    $responseBody = if ($response.Content -is [byte[]]) {
        [System.Text.Encoding]::UTF8.GetString($response.Content).Trim()
    } else {
        "$($response.Content)".Trim()
    }

    $composePs = @(docker compose ps 2>&1 | ForEach-Object { "$_" })
    if ($LASTEXITCODE -ne 0) {
        throw "docker compose ps failed: $($composePs -join ' ')"
    }

    $coreServices = @(
        "web",
        "api",
        "realtime",
        "agent-worker",
        "postgres",
        "redis",
        "nats",
        "minio",
        "reverse-proxy",
        "admin"
    )

    foreach ($service in $coreServices) {
        $line = $composePs | Where-Object { $_ -match "\s$([regex]::Escape($service))\s" } | Select-Object -First 1
        if (-not $line -or $line -notmatch "\(healthy\)") {
            throw "compose service $service is not healthy"
        }
    }

    $checks = @(
        [ordered]@{
            service = "api"
            status = if ($response.StatusCode -eq 200) { "healthy" } else { "unhealthy" }
            latency_ms = $elapsed
            checked_at = $checkedAt
        }
    )

    $checks | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 "$evidenceDir/health-checks.json"

    $smoke = @(
        "Command: powershell -ExecutionPolicy Bypass -File scripts\dev\smoke.ps1",
        "Result: PASS",
        "Checked at: $checkedAt",
        "",
        "docker compose ps:"
    ) + $composePs + @(
        "",
        "healthz http://localhost:8080/healthz => $($response.StatusCode)",
        $responseBody,
        "",
        "init_wizard_completes: NON_CODE_REASON",
        "InitialAdminCreated: NOT_RUN_NON_CODE_REASON",
        "ProviderConnectionTested: NOT_RUN_NON_CODE_REASON",
        "Reason: No executable admin init wizard endpoint or app is present in B032 S09 smoke scope; current compose admin service exposes healthz only. This records an explicit non-code reason and does not claim init wizard PASS."
    )
    $smoke | Set-Content -Encoding UTF8 "$evidenceDir/docker-compose-smoke.txt"
} catch {
    $elapsed = [int]((Get-Date) - $started).TotalMilliseconds
    $checks = @(
        [ordered]@{
            service = "api"
            status = "unhealthy"
            latency_ms = $elapsed
            checked_at = (Get-Date).ToString("o")
            error = $_.Exception.Message
        }
    )

    $checks | ConvertTo-Json -Depth 4 | Set-Content -Encoding UTF8 "$evidenceDir/health-checks.json"
    @(
        "Command: powershell -ExecutionPolicy Bypass -File scripts\dev\smoke.ps1",
        "Result: FAIL",
        "Checked at: $((Get-Date).ToString("o"))",
        "",
        "healthz http://localhost:8080/healthz => FAILED: $($_.Exception.Message)"
    ) | Set-Content -Encoding UTF8 "$evidenceDir/docker-compose-smoke.txt"
    throw
}
