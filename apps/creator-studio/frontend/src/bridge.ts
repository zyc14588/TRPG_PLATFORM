/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

export const CREATOR_BRIDGE_UNAVAILABLE = 'ERR_CREATOR_BRIDGE_UNAVAILABLE'

export type ExtensionStatus = 'SUPPORTED_EDITABLE' | 'UNSUPPORTED_OPTIONAL_READ_ONLY'

export interface ExtensionDescriptor {
  namespace: string
  required: boolean
  contract_version: number
  schema_path: string
  schema_sha256: string
  payload_path: string
  host_api_major: number
  host_api_min_minor: number
  host_api_max_minor: number
}

export interface ExtensionInspection {
  descriptor: ExtensionDescriptor
  status: ExtensionStatus
  editable: boolean
  read_only_reason?: string
  canonical_json?: string
  raw_payload_base64?: string
  raw_payload_sha256?: string
  raw_payload_bytes?: number
}

export interface Inspection {
  source_path: string
  conflict_token: string
  content_hash: string
  extensions: ExtensionInspection[]
}

export interface EditResult {
  namespace: string
  conflict_token: string
  content_hash: string
  canonical_json: string
}

export interface ExportResult {
  path: string
  conflict_token: string
  archive_hash: string
  content_hash: string
}

export interface CreatorServiceBinding {
  ImportArchive(path: string): Promise<Inspection>
  Inspect(): Promise<Inspection>
  Edit(conflictToken: string, namespace: string, jsonText: string): Promise<EditResult>
  Export(conflictToken: string, targetPath: string): Promise<ExportResult>
}

export interface CreatorBridge {
  importArchive(path: string): Promise<Inspection>
  inspect(): Promise<Inspection>
  edit(conflictToken: string, namespace: string, jsonText: string): Promise<EditResult>
  exportArchive(conflictToken: string, targetPath: string): Promise<ExportResult>
}

export interface CreatorWailsHost {
  go?: {
    creator?: {
      Service?: CreatorServiceBinding
    }
  }
}

export class CreatorBridgeError extends Error {
  readonly code = CREATOR_BRIDGE_UNAVAILABLE

  constructor() {
    super('Creator Studio service bridge is unavailable')
    this.name = 'CreatorBridgeError'
  }
}

type ServiceMethod = keyof CreatorServiceBinding

function invokeService<Result>(
  resolveHost: () => CreatorWailsHost | undefined,
  methodName: ServiceMethod,
  args: unknown[],
): Promise<Result> {
  const service = resolveHost()?.go?.creator?.Service
  const method = service?.[methodName]
  if (service === undefined || typeof method !== 'function') {
    return Promise.reject(new CreatorBridgeError())
  }
  const callable = method as unknown as (
    this: CreatorServiceBinding,
    ...values: unknown[]
  ) => Promise<Result>
  try {
    return Promise.resolve(callable.apply(service, args))
  } catch (error) {
    return Promise.reject(error)
  }
}

export function createCreatorBridge(resolveHost: () => CreatorWailsHost | undefined): CreatorBridge {
  return {
    importArchive: (path) => invokeService(resolveHost, 'ImportArchive', [path]),
    inspect: () => invokeService(resolveHost, 'Inspect', []),
    edit: (conflictToken, namespace, jsonText) =>
      invokeService(resolveHost, 'Edit', [conflictToken, namespace, jsonText]),
    exportArchive: (conflictToken, targetPath) =>
      invokeService(resolveHost, 'Export', [conflictToken, targetPath]),
  }
}

export const creatorBridge = createCreatorBridge(() =>
  typeof window === 'undefined' ? undefined : (window as unknown as CreatorWailsHost),
)
