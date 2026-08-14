/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

import type {
  CreatorBridge,
  EditResult,
  ExportResult,
  ExtensionInspection,
  Inspection,
} from './bridge'

export function importCreatorArchive(bridge: CreatorBridge, path: string): Promise<Inspection> {
  return bridge.importArchive(path)
}

export function inspectCreatorSession(bridge: CreatorBridge): Promise<Inspection> {
  return bridge.inspect()
}

export function validateAndApplyCreatorEdit(
  bridge: CreatorBridge,
  inspection: Inspection,
  extension: ExtensionInspection,
  jsonText: string,
): Promise<EditResult> {
  return bridge.edit(inspection.conflict_token, extension.descriptor.namespace, jsonText)
}

export function applyEditResult(inspection: Inspection, result: EditResult): Inspection {
  return {
    ...inspection,
    conflict_token: result.conflict_token,
    content_hash: result.content_hash,
    extensions: inspection.extensions.map((extension) =>
      extension.descriptor.namespace === result.namespace
        ? { ...extension, canonical_json: result.canonical_json }
        : extension,
    ),
  }
}

export function exportCreatorArchive(
  bridge: CreatorBridge,
  inspection: Inspection,
  targetPath: string,
): Promise<ExportResult> {
  return bridge.exportArchive(inspection.conflict_token, targetPath)
}

export function applyExportResult(inspection: Inspection, result: ExportResult): Inspection {
  return {
    ...inspection,
    source_path: result.path,
    conflict_token: result.conflict_token,
    content_hash: result.content_hash,
  }
}

export function reimportCreatorExport(bridge: CreatorBridge, outputPath: string): Promise<Inspection> {
  return bridge.importArchive(outputPath)
}
