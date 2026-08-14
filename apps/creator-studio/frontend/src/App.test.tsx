/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { App, DiagnosticBanner, diagnosticFromError } from './App'
import type { Inspection } from './bridge'

const supportedInspection: Inspection = {
  source_path: '/tmp/source.trpgpkg',
  conflict_token: `sha256:${'1'.repeat(64)}`,
  content_hash: `sha256:${'2'.repeat(64)}`,
  extensions: [
    {
      descriptor: {
        namespace: 'third.party.probe',
        required: true,
        contract_version: 1,
        schema_path: 'extensions/third.party.probe/value.schema.json',
        schema_sha256: `sha256:${'3'.repeat(64)}`,
        payload_path: 'extensions/third.party.probe/value.json',
        host_api_major: 1,
        host_api_min_minor: 0,
        host_api_max_minor: 2,
      },
      status: 'SUPPORTED_EDITABLE',
      editable: true,
      canonical_json: '{"count":1}',
    },
  ],
}

describe('Creator Studio generic JSON editor', () => {
  it('renders the complete supported import, inspect, edit, export, and re-import surface', () => {
    const markup = renderToStaticMarkup(<App initialInspection={supportedInspection} />)

    expect(markup).toContain('Schema-validated generic JSON')
    expect(markup).toContain('Package-generic JSON by design')
    expect(markup).toContain('No game-specific forms or templates')
    expect(markup).toContain('No playable preview or runtime execution')
    expect(markup).toContain('third.party.probe')
    expect(markup).toContain('SUPPORTED_EDITABLE')
    expect(markup).toContain('Schema digest')
    expect(markup).toContain('Host compatibility')
    for (const control of [
      'archive-path',
      'import-archive',
      'refresh-inspection',
      'extension-picker',
      'json-editor',
      'validate-edit',
      'output-path',
      'export-archive',
      'reimport-export',
    ]) {
      expect(markup).toContain(`data-control="${control}"`)
    }
    expect(markup).toContain('Validate &amp; apply edit')
    expect(markup).toContain('{&quot;count&quot;:1}')
    for (const token of ['ru' + 'les', 'equip' + 'ment']) {
      expect(markup.toLowerCase()).not.toContain(token)
    }
  })

  it('renders optional unsupported payload metadata without an editable value or opaque bytes', () => {
    const inspection: Inspection = {
      ...supportedInspection,
      extensions: [
        {
          ...supportedInspection.extensions[0],
          descriptor: { ...supportedInspection.extensions[0].descriptor, required: false, contract_version: 2 },
          status: 'UNSUPPORTED_OPTIONAL_READ_ONLY',
          editable: false,
          canonical_json: undefined,
          read_only_reason: 'contract version is unavailable',
          raw_payload_sha256: `sha256:${'4'.repeat(64)}`,
          raw_payload_bytes: 37,
          raw_payload_base64: 'SECRET-OPAQUE-BYTES',
        },
      ],
    }

    const markup = renderToStaticMarkup(<App initialInspection={inspection} />)

    expect(markup).toContain('Preserved read-only')
    expect(markup).toContain('contract version is unavailable')
    expect(markup).toContain(`sha256:${'4'.repeat(64)}`)
    expect(markup).toContain('37 bytes')
    expect(markup).not.toContain('data-control="json-editor"')
    expect(markup).not.toContain('data-control="validate-edit"')
    expect(markup).not.toContain('SECRET-OPAQUE-BYTES')
  })

  it('shows typed required-contract failures as an accessible bounded alert', () => {
    const diagnostic = diagnosticFromError({
      code: 'ERR_EXTENSION_REQUIRED_UNSUPPORTED',
      detail: 'required contract version is unavailable',
    })
    const markup = renderToStaticMarkup(<DiagnosticBanner diagnostic={diagnostic} />)

    expect(markup).toContain('role="alert"')
    expect(markup).toContain('ERR_EXTENSION_REQUIRED_UNSUPPORTED')
    expect(markup).toContain('required contract version is unavailable')
  })

  it('recovers typed failure codes from the bounded Wails string form', () => {
    const diagnostic = diagnosticFromError(
      new Error(
        'ERR_EXTENSION_REQUIRED_UNSUPPORTED third.party.probe: required contract version is unavailable',
      ),
    )

    expect(diagnostic).toEqual({
      kind: 'error',
      code: 'ERR_EXTENSION_REQUIRED_UNSUPPORTED',
      detail: 'third.party.probe: required contract version is unavailable',
    })
  })

  it('bounds untrusted diagnostic fields before rendering', () => {
    const diagnostic = diagnosticFromError({ code: 'E'.repeat(500), detail: 'D'.repeat(500) })

    expect(diagnostic.code?.length).toBeLessThanOrEqual(321)
    expect(diagnostic.detail.length).toBeLessThanOrEqual(321)
  })
})
