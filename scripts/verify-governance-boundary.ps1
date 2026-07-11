[CmdletBinding()]
param(
    [string]$RepositoryRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Governance {
    param(
        [bool]$Condition,
        [string]$Code,
        [string]$Message
    )

    if (-not $Condition) {
        throw "${Code}: ${Message}"
    }
}

function Read-FencedJson {
    param([string]$Path)

    $raw = Get-Content -Raw -Encoding UTF8 -LiteralPath $Path
    $match = [regex]::Match($raw, '(?s)```json\s*(.*?)\s*```')
    Assert-Governance $match.Success 'GOVERNANCE_INPUT_MISSING' "JSON fixture fence missing: $Path"
    return $match.Groups[1].Value | ConvertFrom-Json
}

try {
    if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
        $RepositoryRoot = Split-Path -Parent $PSScriptRoot
    }
    $root = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $fixturePath = Join-Path $root 'fixtures/stages/detailed/S00_governance_onboarding.current.json.md'
    Assert-Governance (Test-Path -LiteralPath $fixturePath -PathType Leaf) 'GOVERNANCE_INPUT_MISSING' "Missing fixture: $fixturePath"
    $fixture = Read-FencedJson $fixturePath

    Assert-Governance ($fixture.stage -eq 'S00') 'GOVERNANCE_INPUT_MISSING' 'Expected the S00 detailed fixture.'

    $requiredFiles = @($fixture.inputs.required_files)
    foreach ($relativePath in $requiredFiles) {
        $path = Join-Path $root $relativePath
        Assert-Governance (Test-Path -LiteralPath $path -PathType Leaf) 'GOVERNANCE_INPUT_MISSING' "Missing authority file: $relativePath"
        [void](Get-Content -Raw -Encoding UTF8 -LiteralPath $path)
    }

    $event = @($fixture.expected_events)[0]
    Assert-Governance ($event.visibility -eq 'system_only') 'GOVERNANCE_INPUT_MISSING' 'Governance readiness evidence must be system_only.'
    foreach ($field in @('authority_order', 'normalized_overlay', 'provenance_boundary')) {
        Assert-Governance ($event.must_include -contains $field) 'GOVERNANCE_INPUT_MISSING' "Expected event field missing: $field"
    }

    $record = @($fixture.expected_records)[0]
    $recordPath = Join-Path $root $record.path
    Assert-Governance (Test-Path -LiteralPath $recordPath -PathType Leaf) 'GOVERNANCE_INPUT_MISSING' "Missing readiness record: $($record.path)"
    $recordBody = Get-Content -Raw -Encoding UTF8 -LiteralPath $recordPath
    foreach ($relativePath in $requiredFiles) {
        Assert-Governance ($recordBody.Contains($relativePath)) 'GOVERNANCE_INPUT_MISSING' "Readiness input missing: $relativePath"
    }
    foreach ($field in @($record.required_fields)) {
        Assert-Governance ($recordBody -match "(?m)^$([regex]::Escape($field))\s*:") 'GOVERNANCE_INPUT_MISSING' "Readiness field missing: $field"
    }

    $evidenceAction = @($fixture.actions | Where-Object { $_.id -eq 'create_evidence_root' })[0]
    Assert-Governance (Test-Path -LiteralPath (Join-Path $root $evidenceAction.path) -PathType Container) 'GOVERNANCE_INPUT_MISSING' "Missing evidence root: $($evidenceAction.path)"
    foreach ($relativePath in @($fixture.required_evidence)) {
        Assert-Governance (Test-Path -LiteralPath (Join-Path $root $relativePath)) 'GOVERNANCE_INPUT_MISSING' "Missing S00 evidence: $relativePath"
    }

    $missingFileCase = @($fixture.expected_errors | Where-Object { $_.case -eq 'missing_authority_file' })[0]
    Assert-Governance ($missingFileCase.error -eq 'GOVERNANCE_INPUT_MISSING') 'GOVERNANCE_INPUT_MISSING' 'Missing-file error contract changed.'
    $provenanceCase = @($fixture.failure_cases | Where-Object { $_.id -eq 'uses_source_materials_as_current' })[0]
    Assert-Governance ($provenanceCase.expected_error -eq 'PROVENANCE_USED_AS_AUTHORITY') 'GOVERNANCE_INPUT_MISSING' 'Provenance error contract changed.'
    Assert-Governance ($event.type -eq 'GovernanceReadinessRecorded') 'GOVERNANCE_INPUT_MISSING' 'Governance event type changed.'
    Assert-Governance ($fixture.automation_target -match 'scripts/verify-governance-boundary\.ps1') 'GOVERNANCE_INPUT_MISSING' 'Fixture automation target does not name this verifier.'
    $passCriteria = @($fixture.pass_criteria)
    Assert-Governance ($passCriteria.Count -eq 3 -and $passCriteria -contains 'all_required_files_read' -and $passCriteria -contains 'source_materials_not_current' -and $passCriteria -contains 'evidence_root_created') 'GOVERNANCE_INPUT_MISSING' 'S00 pass criteria changed.'

    $normalizedPath = Join-Path $root 'docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md'
    $safePath = Join-Path $root 'docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md'
    $tokenPath = Join-Path $root 'docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md'
    foreach ($path in @($normalizedPath, $safePath, $tokenPath)) {
        Assert-Governance (Test-Path -LiteralPath $path -PathType Leaf) 'GOVERNANCE_INPUT_MISSING' "Missing normalized overlay file: $path"
    }
    $normalizedRows = @(Get-Content -Encoding UTF8 -LiteralPath $normalizedPath | Where-Object { $_ -match '^\| `CODEX-' })
    $safeRows = @(Get-Content -Encoding UTF8 -LiteralPath $safePath | Where-Object { $_ -match '^\| `CODEX-' })
    Assert-Governance ($normalizedRows.Count -gt 0 -and $normalizedRows.Count -eq $safeRows.Count) 'GOVERNANCE_INPUT_MISSING' 'Current-safe maps are empty or inconsistent.'

    foreach ($row in $normalizedRows) {
        $target = $row.Split('|')[6].Trim().Trim([char]96)
        Assert-Governance ($target -notmatch '(^|/)source-archive/|(^|/)fix-history/|\.legacy(?:[-_.]|$)') 'PROVENANCE_USED_AS_AUTHORITY' "Normalized output uses provenance as current: $target"
    }
    foreach ($row in $safeRows) {
        $target = $row.Split('|')[4].Trim().Trim([char]96)
        Assert-Governance ($target -notmatch '(^|/)source-archive/|(^|/)fix-history/|\.legacy(?:[-_.]|$)') 'PROVENANCE_USED_AS_AUTHORITY' "Safe output uses provenance as current: $target"
    }

    $boundaryPath = Join-Path $root 'DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md'
    $boundaryBody = Get-Content -Raw -Encoding UTF8 -LiteralPath $boundaryPath
    $currentAreas = [regex]::Match($boundaryBody, '(?s)## Current execution areas(.*?)## Design and audit areas')
    $provenanceAreas = [regex]::Match($boundaryBody, '(?s)## Provenance-only areas(.*?)## Canonical token rewrite rule')
    Assert-Governance ($currentAreas.Success -and $currentAreas.Groups[1].Value -notmatch 'source-archive') 'PROVENANCE_USED_AS_AUTHORITY' 'source-archive appears in current execution areas.'
    Assert-Governance ($provenanceAreas.Success -and $provenanceAreas.Groups[1].Value -match 'source-archive/\*\*') 'PROVENANCE_USED_AS_AUTHORITY' 'source-archive is not confined to provenance-only areas.'

    Write-Output 'S00 governance boundary check'
    Write-Output "required_files=$($requiredFiles.Count) PASS"
    Write-Output "expected_event=$($event.type) visibility=$($event.visibility) PASS"
    Write-Output "expected_record=$($record.record) fields=$(@($record.required_fields).Count) PASS"
    Write-Output "required_evidence=$(@($fixture.required_evidence).Count) PASS"
    Write-Output 'normalized_overlay_files=3 PASS'
    Write-Output "normalized_rows=$($normalizedRows.Count) PASS"
    Write-Output "safe_rows=$($safeRows.Count) PASS"
    Write-Output 'provenance_boundary=PASS'
    Write-Output "pass_criteria=$($passCriteria.Count) PASS"
    Write-Output 'RESULT=PASS'
}
catch {
    Write-Error $_.Exception.Message
    exit 1
}
