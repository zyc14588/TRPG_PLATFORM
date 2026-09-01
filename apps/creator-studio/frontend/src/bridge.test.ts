/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

import { describe, expect, it, vi } from 'vitest'

import {
  CREATOR_BRIDGE_UNAVAILABLE,
  createCreatorBridge,
  type CreatorServiceBinding,
  type CreatorWailsHost,
  type Inspection,
} from './bridge'

const emptyInspection: Inspection = {
  source_path: '/tmp/input.trpgpkg',
  conflict_token: `sha256:${'1'.repeat(64)}`,
  content_hash: `sha256:${'2'.repeat(64)}`,
  extensions: [],
}

describe('Creator Wails bridge', () => {
  it('fails deterministically when rendered without window or Wails bindings', async () => {
    const bridge = createCreatorBridge(() => undefined)

    await expect(bridge.importArchive('/tmp/input.trpgpkg')).rejects.toMatchObject({
      code: CREATOR_BRIDGE_UNAVAILABLE,
      message: 'Creator Studio service bridge is unavailable',
    })
    await expect(bridge.inspect()).rejects.toMatchObject({ code: CREATOR_BRIDGE_UNAVAILABLE })
  })

  it('resolves dynamically and calls every method with the Service receiver', async () => {
    let activeHost: CreatorWailsHost | undefined
    const calls: string[] = []
    const service: CreatorServiceBinding & { marker: string } = {
      marker: 'bound-service',
      ImportArchive: vi.fn(async function (this: typeof service, path: string) {
        calls.push(`${this.marker}:import:${path}`)
        return emptyInspection
      }),
      Inspect: vi.fn(async function (this: typeof service) {
        calls.push(`${this.marker}:inspect`)
        return emptyInspection
      }),
      Edit: vi.fn(async function (
        this: typeof service,
        conflictToken: string,
        namespace: string,
        jsonText: string,
      ) {
        calls.push(`${this.marker}:edit:${conflictToken}:${namespace}:${jsonText}`)
        return {
          namespace,
          conflict_token: conflictToken,
          content_hash: emptyInspection.content_hash,
          canonical_json: jsonText,
        }
      }),
      Export: vi.fn(async function (this: typeof service, conflictToken: string, targetPath: string) {
        calls.push(`${this.marker}:export:${conflictToken}:${targetPath}`)
        return {
          path: targetPath,
          conflict_token: conflictToken,
          archive_hash: conflictToken,
          content_hash: emptyInspection.content_hash,
        }
      }),
    }
    const bridge = createCreatorBridge(() => activeHost)

    await expect(bridge.inspect()).rejects.toMatchObject({ code: CREATOR_BRIDGE_UNAVAILABLE })
    activeHost = { go: { creator: { Service: service } } }

    await bridge.importArchive('/tmp/input.trpgpkg')
    await bridge.inspect()
    await bridge.edit('token', 'third.party.probe', '{"count":2}')
    await bridge.exportArchive('token', '/tmp/output.trpgpkg')

    expect(calls).toEqual([
      'bound-service:import:/tmp/input.trpgpkg',
      'bound-service:inspect',
      'bound-service:edit:token:third.party.probe:{"count":2}',
      'bound-service:export:token:/tmp/output.trpgpkg',
    ])
  })
})
