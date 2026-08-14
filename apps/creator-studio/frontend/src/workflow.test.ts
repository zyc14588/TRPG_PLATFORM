/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

import { describe, expect, it, vi } from 'vitest'

import type { CreatorBridge, Inspection } from './bridge'
import {
  applyEditResult,
  applyExportResult,
  exportCreatorArchive,
  importCreatorArchive,
  inspectCreatorSession,
  reimportCreatorExport,
  validateAndApplyCreatorEdit,
} from './workflow'

const inputToken = `sha256:${'1'.repeat(64)}`
const outputToken = `sha256:${'2'.repeat(64)}`
const initial: Inspection = {
  source_path: '/tmp/source.trpgpkg',
  conflict_token: inputToken,
  content_hash: `sha256:${'3'.repeat(64)}`,
  extensions: [
    {
      descriptor: {
        namespace: 'third.party.probe',
        required: true,
        contract_version: 1,
        schema_path: 'extensions/third.party.probe/value.schema.json',
        schema_sha256: `sha256:${'4'.repeat(64)}`,
        payload_path: 'extensions/third.party.probe/value.json',
        host_api_major: 1,
        host_api_min_minor: 0,
        host_api_max_minor: 0,
      },
      status: 'SUPPORTED_EDITABLE',
      editable: true,
      canonical_json: '{"count":1}',
    },
    {
      descriptor: {
        namespace: 'third.party.opaque',
        required: false,
        contract_version: 2,
        schema_path: 'extensions/third.party.opaque/value.schema.json',
        schema_sha256: `sha256:${'6'.repeat(64)}`,
        payload_path: 'extensions/third.party.opaque/value.json',
        host_api_major: 1,
        host_api_min_minor: 0,
        host_api_max_minor: 0,
      },
      status: 'UNSUPPORTED_OPTIONAL_READ_ONLY',
      editable: false,
      read_only_reason: 'contract version is unavailable',
      raw_payload_sha256: `sha256:${'7'.repeat(64)}`,
      raw_payload_bytes: 41,
      raw_payload_base64: 'opaque-preserved-value',
    },
  ],
}

describe('Creator UI workflow', () => {
  it('calls the shared bridge for every boundary and preserves fields across edit/export transitions', async () => {
    const editedResult = {
      namespace: 'third.party.probe',
      conflict_token: inputToken,
      content_hash: `sha256:${'5'.repeat(64)}`,
      canonical_json: '{"count":2}',
    }
    const exportedResult = {
      path: '/tmp/output.trpgpkg',
      conflict_token: outputToken,
      archive_hash: outputToken,
      content_hash: editedResult.content_hash,
    }
    const reimported: Inspection = {
      ...initial,
      source_path: exportedResult.path,
      conflict_token: outputToken,
      content_hash: editedResult.content_hash,
      extensions: [
        { ...initial.extensions[0], canonical_json: editedResult.canonical_json },
        initial.extensions[1],
      ],
    }
    const bridge: CreatorBridge = {
      importArchive: vi.fn(async (path) => (path === exportedResult.path ? reimported : initial)),
      inspect: vi.fn(async () => initial),
      edit: vi.fn(async () => editedResult),
      exportArchive: vi.fn(async () => exportedResult),
    }

    const imported = await importCreatorArchive(bridge, initial.source_path)
    const inspected = await inspectCreatorSession(bridge)
    const edit = await validateAndApplyCreatorEdit(bridge, inspected, inspected.extensions[0], ' { "count" : 2 } ')
    const afterEdit = applyEditResult(inspected, edit)
    const exported = await exportCreatorArchive(bridge, afterEdit, exportedResult.path)
    const afterExport = applyExportResult(afterEdit, exported)
    const loadedExport = await reimportCreatorExport(bridge, exported.path)

    expect(imported).toBe(initial)
    expect(bridge.importArchive).toHaveBeenNthCalledWith(1, initial.source_path)
    expect(bridge.inspect).toHaveBeenCalledOnce()
    expect(bridge.edit).toHaveBeenCalledWith(inputToken, 'third.party.probe', ' { "count" : 2 } ')
    expect(afterEdit.conflict_token).toBe(edit.conflict_token)
    expect(afterEdit.extensions[0].canonical_json).toBe('{"count":2}')
    expect(afterEdit.extensions[0].descriptor).toBe(initial.extensions[0].descriptor)
    expect(afterEdit.extensions[1]).toBe(initial.extensions[1])
    expect(afterEdit.extensions[1].raw_payload_base64).toBe('opaque-preserved-value')
    expect(bridge.exportArchive).toHaveBeenCalledWith(inputToken, exportedResult.path)
    expect(afterExport.source_path).toBe(exportedResult.path)
    expect(afterExport.conflict_token).toBe(outputToken)
    expect(afterExport.extensions).toBe(afterEdit.extensions)
    expect(bridge.importArchive).toHaveBeenNthCalledWith(2, exportedResult.path)
    expect(loadedExport).toBe(reimported)
  })
})
