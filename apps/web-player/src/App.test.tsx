// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { App } from './App'
import { API, APIError, attempt } from './api'
import { Player, presentationMatches } from './player'
import { encodeValue, GameValue } from './value'
import type { Connection, Control, Lobby, Presentation, Room, Snapshot } from './types'

// Select the real controller that App creates without replacing its methods or
// state transitions. Each regression below crosses the actual API envelope.
const held = vi.hoisted(() => ({ instance: undefined as Player | undefined }))
vi.mock('./player', async importOriginal => {
  const actual = await importOriginal<typeof import('./player')>()
  return { ...actual, Player: class extends actual.Player {
    constructor(api?: API) { super(api); if (held.instance) return held.instance }
  } }
})
afterEach(() => { held.instance = undefined })

async function unknownCommand(transport: typeof fetch) {
  const actual = await vi.importActual<typeof import('./player')>('./player')
  const control: Control = { revision: '3', paused: false, reason: 'none', own_resume_required: false }
  const connection: Connection = { connection_id: 'original-connection', session_id: 'session', seat_id: 'player', expires_in_seconds: 30, control }
  const snapshot: Snapshot = { session_id: 'session', seat_id: 'player', state_version: '1', event_cursor: '0', view: { kind: 'nil' }, events: [], more: false, ended: false, control }
  const player = new actual.Player(new API(transport))
  player.api.csrf = 'fixture-csrf'
  player.state = { ...player.state,
    context: { state: 'authenticated', csrf_token: 'fixture-csrf', principal: { kind: 'account', account_id: 'participant', display_name: 'Participant' } },
    room: { workspace_id: 'w', room_id: 'r', game_id: 'game', name: 'Private room', state: 'launched', owner_account_id: 'owner', caller_roles: [], participants: [] },
    connection, snapshot, control,
  }
  held.instance = player
  await player.run(() => player.command('increment', { kind: 'table', table: {} }))
  expect(player.state.pending).toBeDefined()
  expect(player.state.connection).toBeUndefined()
  expect(player.state.snapshot).toBeUndefined()
  return player
}
function response(data: unknown) { return new Response(JSON.stringify({ schema_version: 1, data }), { headers: { 'Content-Type': 'application/json' } }) }
function refusal(code: string, status: number) { return new Response(JSON.stringify({ schema_version: 1, error: { code } }), { status, headers: { 'Content-Type': 'application/json' } }) }

describe('Player authority boundaries', () => {
  it('starts with game language and a recoverable server connection', () => {
    const markup = renderToStaticMarkup(<App />)
    expect(markup).toContain('今晚，玩一场。')
    expect(markup).toContain('重新获取登录状态')
    expect(markup).not.toContain('No playable functionality')
    expect(markup).not.toContain('API Key')
  })
  it('escapes private view text instead of interpreting game content as HTML', () => {
    expect(renderToStaticMarkup(<GameValue value={{ kind: 'string', string: '<script>attack()</script>' }} />)).toContain('&lt;script&gt;')
  })
  it('rejects precision loss, nonfinite values and capability handles', () => {
    expect(() => encodeValue(Number.MAX_SAFE_INTEGER + 1)).toThrow()
    expect(() => encodeValue(Infinity)).toThrow()
    expect(() => encodeValue({ 'cap:private': 'x' })).toThrow()
    expect(() => encodeValue('cap:private')).toThrow()
    expect(encodeValue({ delta: 1 })).toEqual({ kind: 'table', table: { delta: { kind: 'integer', number: '1' } } })
  })
  it('requires workspace, game, configuration and both digests for consent', () => {
    const room = { workspace_id: 'w', game_id: 'game' } as Room
    const lobby = { configuration_id: 'c', configuration_hash: 'config', graph_hash: 'graph' } as Lobby
    const p = { workspace_id: 'w', game_id: 'game', configuration_id: 'c', configuration_hash: 'config', graph_hash: 'graph', packages: [{}] } as Presentation
    expect(presentationMatches(room, lobby, p)).toBe(true)
    for (const key of ['workspace_id', 'game_id', 'configuration_id', 'configuration_hash', 'graph_hash']) expect(presentationMatches(room, lobby, { ...p, [key]: 'other' })).toBe(false)
    expect(presentationMatches(room, lobby, { ...p, packages: [] })).toBe(false)
  })
  it('sends only same-origin requests and retries the identical bounded operation', async () => {
    const transport = vi.fn().mockRejectedValueOnce(new Error('secret transport detail')).mockResolvedValue(new Response(JSON.stringify({ schema_version: 1, data: { applied: true } }), { headers: { 'Content-Type': 'application/json' } }))
    const api = new API(transport); api.csrf = 'test-csrf'
    const operation = attempt('/api/v1/workspaces/w/rooms/r/consent', { consent: true })
    await expect(api.send(operation)).rejects.toEqual(new APIError('OUTCOME_UNKNOWN'))
    await expect(api.send(operation)).resolves.toEqual({ applied: true })
    expect(transport.mock.calls[0][0]).toEqual(transport.mock.calls[1][0])
    expect(transport.mock.calls[0][1].body).toEqual(transport.mock.calls[1][1].body)
    expect(transport.mock.calls[0][1].headers).toEqual(transport.mock.calls[1][1].headers)
    const options = transport.mock.calls[0][1]
    expect(options.credentials).toBe('same-origin')
    expect(options.redirect).toBe('error')
    expect(options.cache).toBe('no-store')
    await expect(api.get('https://foreign.invalid/api/v1/auth/context')).rejects.toThrow()
    expect(() => attempt('/api/v1/room-admissions', { value: 'a'.repeat(17000) })).toThrow()
  })
  it('treats a truncated successful mutation response as unknown', async () => {
    const api = new API(vi.fn().mockResolvedValue(new Response('{', { headers: { 'Content-Type': 'application/json' } })))
    await expect(api.send(attempt('/api/v1/auth/logout'))).rejects.toEqual(new APIError('OUTCOME_UNKNOWN'))
  })
  it('clears all private projections when current authorization is revoked', async () => {
    const player = new Player(new API(vi.fn().mockResolvedValue(new Response(JSON.stringify({ schema_version: 1, error: { code: 'DENIED' } }), { status: 403, headers: { 'Content-Type': 'application/json' } }))))
    player.state = { ...player.state, room: { workspace_id: 'w', room_id: 'r' } as Room, exported: {} as never, snapshot: {} as never, presentation: {} as Presentation }
    await player.run(() => player.refresh())
    expect(player.state.room).toBeUndefined()
    expect(player.state.snapshot).toBeUndefined()
    expect(player.state.exported).toBeUndefined()
    expect(player.state.presentation).toBeUndefined()
    expect(player.state.error).toContain('没有这项权限')
  })
  it.each(['different-account', 'rotated-session', 'anonymous'])('drops private state and uncertain requests on %s context', async change => {
    const previous = { state: 'authenticated' as const, csrf_token: 'old-context', principal: { kind: 'account' as const, account_id: 'first', display_name: 'First' } }
    const context = change === 'anonymous' ? { state: 'anonymous', csrf_token: 'new-context' } : { ...previous, csrf_token: 'new-context', principal: { ...previous.principal, account_id: change === 'different-account' ? 'second' : 'first' } }
    const transport = vi.fn().mockResolvedValue(new Response(JSON.stringify({ schema_version: 1, data: context }), { headers: { 'Content-Type': 'application/json' } }))
    const player = new Player(new API(transport))
    player.state = { ...player.state, context: previous, workspace: 'private-workspace', room: {} as Room, lobby: {} as Lobby, admission: {} as never, presentation: {} as Presentation, snapshot: {} as never, exported: {} as never, pending: { attempt: attempt('/api/v1/auth/logout'), finish: async () => {} } }
    await player.context()
    expect(player.state.context).toEqual(context)
    expect(player.state.workspace).toBe('')
    for (const key of ['room', 'lobby', 'admission', 'presentation', 'snapshot', 'exported', 'pending'] as const) expect(player.state[key]).toBeUndefined()
    expect(transport).toHaveBeenCalledTimes(1)
  })
  it('refreshes no idle snapshot immediately after a successful authoritative read', async () => {
    const snapshot = { session_id: 'session', seat_id: 'seat', event_cursor: '0', control: { paused: true } }
    const transport = vi.fn().mockResolvedValue(new Response(JSON.stringify({ schema_version: 1, data: snapshot }), { headers: { 'Content-Type': 'application/json' } }))
    const player = new Player(new API(transport))
    player.state = { ...player.state, room: { workspace_id: 'w', room_id: 'r' } as Room, connection: { session_id: 'session', seat_id: 'seat', connection_id: 'connection' } as never }
    await player.poll()
    await player.poll()
    expect(transport).toHaveBeenCalledTimes(1)
    expect(player.state.snapshot).toEqual(snapshot)
  })
  it('continues invited account room consent without workspace catalog access', async () => {
    const room = { workspace_id: 'w', room_id: 'r', game_id: 'game', state: 'lobby' } as Room
    const lobby = { configuration_id: 'c', configuration_hash: 'config', graph_hash: 'graph', revision: '2', can_manage: false } as Lobby
    const presentation = { workspace_id: 'w', game_id: 'game', configuration_id: 'c', configuration_hash: 'config', graph_hash: 'graph', packages: [{}] } as Presentation
    const transport = vi.fn(async (path: string) => {
      const data = path.endsWith('/presentation') ? presentation : path.endsWith('/preparation') ? lobby : path.endsWith('/consent') ? { applied: true } : room
      if (path.includes('/games')) throw new Error('non-member catalog access')
      return new Response(JSON.stringify({ schema_version: 1, data }), { headers: { 'Content-Type': 'application/json' } })
    })
    const player = new Player(new API(transport as typeof fetch))
    await player.loadRoom('w', 'r')
    expect(player.state.presentation).toEqual(presentation)
    await player.consent({ consent: true, ready: true, safety_confirmed: true, boundaries: [] })
    expect(transport.mock.calls.map(([path]) => path)).toEqual([
      '/api/v1/workspaces/w/rooms/r', '/api/v1/workspaces/w/rooms/r/preparation', '/api/v1/workspaces/w/rooms/r/presentation',
      '/api/v1/workspaces/w/rooms/r/presentation', '/api/v1/workspaces/w/rooms/r/consent', '/api/v1/workspaces/w/rooms/r/preparation',
    ])
  })
  it('refreshes a concurrent resume refusal and requires another explicit confirmation', async () => {
    const control = { revision: '2', paused: true, reason: 'disconnect', own_resume_required: true }
    const transport = vi.fn()
      .mockResolvedValueOnce(new Response(JSON.stringify({ schema_version: 1, error: { code: 'RATE_LIMITED' } }), { status: 429, headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ schema_version: 1, data: { session_id: 'session', seat_id: 'seat', event_cursor: '7', control } }), { headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(new Response(JSON.stringify({ schema_version: 1, data: { ...control, revision: '3', paused: false, own_resume_required: false } }), { headers: { 'Content-Type': 'application/json' } }))
    const player = new Player(new API(transport))
    player.state = { ...player.state, room: { workspace_id: 'w', room_id: 'r' } as Room, control: { ...control, revision: '1' } as never, connection: { session_id: 'session', seat_id: 'seat', connection_id: 'connection' } as never }
    await player.run(() => player.safety('resume'))
    expect(player.state.error).toContain('操作过于频繁')
    expect(player.state.control).toEqual(control)
    expect(player.state.pending).toBeUndefined()
    expect(transport).toHaveBeenCalledTimes(2)
    await player.run(() => player.safety('resume'))
    expect(JSON.parse(transport.mock.calls[2][1].body).expected_control_revision).toBe('2')
    expect(player.state.control?.paused).toBe(false)
  })
  it('reconnects with the last server cursor after removing the disconnected view', async () => {
    const context = { state: 'authenticated', csrf_token: 'csrf', principal: { kind: 'account', account_id: 'account', display_name: 'Player' } }
    const control = { revision: '1', paused: true, reason: 'disconnect', own_resume_required: true }
    const connection = { session_id: 'session', seat_id: 'seat', connection_id: 'connection' }
    const transport = vi.fn(async (path: string) => {
      const data = path.endsWith('/context') ? context : path.endsWith('/disconnect') ? control : path.endsWith('/connect') ? { ...connection, control } : { ...connection, event_cursor: '7', control }
      return new Response(JSON.stringify({ schema_version: 1, data }), { headers: { 'Content-Type': 'application/json' } })
    })
    const player = new Player(new API(transport as typeof fetch))
    player.state = { ...player.state, context: context as never, room: { workspace_id: 'w', room_id: 'r' } as Room, connection: connection as never }
    await player.poll()
    await player.disconnect()
    expect(player.state.snapshot).toBeUndefined()
    await player.connect()
    const options = (transport.mock.calls as unknown as [string, RequestInit][]).find(([path]) => path.endsWith('/connect'))![1]
    expect(JSON.parse(options.body as string).after_cursor).toBe('7')
    expect(player.state.snapshot?.event_cursor).toBe('7')
  })

  it('keeps immediate pause reachable after the actual command response becomes unknown', async () => {
    const transport = vi.fn()
      .mockResolvedValueOnce(new Response('{', { headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(response({ revision: '4', paused: true, reason: 'safety', own_resume_required: true }))
    const player = await unknownCommand(transport)
    const original = player.state.pending
    let markup = renderToStaticMarkup(<App />)
    expect(markup).toContain('查询原操作结果')
    expect(markup).toMatch(/<button class="danger">立即安全暂停<\/button>/)
    expect(markup).toMatch(/<button disabled=""[^>]*>连接 \/ 重新连接<\/button>/)
    expect(markup).not.toContain('<legend>提交行动</legend>')
    await player.run(() => player.command('increment', { kind: 'table', table: {} }))
    expect(transport).toHaveBeenCalledTimes(1)
    expect(player.state.pending).toBe(original)
    await player.run(() => player.safety('pause'))
    expect(player.state.pending).toBe(original)
    expect(player.state.control?.paused).toBe(true)
    expect(transport.mock.calls[1][0]).toBe('/api/v1/workspaces/w/rooms/r/session/pause')
    expect(JSON.parse(transport.mock.calls[1][1].body).expected_control_revision).toBe('3')
    markup = renderToStaticMarkup(<App />)
    expect(markup).toMatch(/<button>我已确认边界，继续游戏<\/button>/)
    player.state = { ...player.state, busy: true }
    expect(renderToStaticMarkup(<App />)).toMatch(/<button class="danger" disabled="">立即安全暂停<\/button>/)
  })
  it('retains the original pending command when its replay is legitimately paused', async () => {
    const transport = vi.fn()
      .mockResolvedValueOnce(new Response('{', { headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(response({ revision: '4', paused: true, reason: 'safety', own_resume_required: true }))
      .mockResolvedValueOnce(refusal('PLAYER_PAUSED', 409))
    const player = await unknownCommand(transport)
    const original = player.state.pending
    await player.run(() => player.safety('pause'))
    await player.retry()
    expect(player.state.error).toContain('游戏已暂停')
    expect(player.state.pending).toBe(original)
    expect(transport.mock.calls[2][0]).toBe(transport.mock.calls[0][0])
    expect(transport.mock.calls[2][1].body).toBe(transport.mock.calls[0][1].body)
    expect(transport.mock.calls[2][1].headers).toEqual(transport.mock.calls[0][1].headers)
  })
  it.each(['RATE_LIMITED', 'CONFLICT'])('recovers a %s safety confirmation through the original lease without replacing the pending command', async code => {
    const paused = { revision: '4', paused: true, reason: 'safety', own_resume_required: true }
    const current = { ...paused, revision: '5' }
    const resumed = { revision: '6', paused: false, reason: 'none', own_resume_required: false }
    const transport = vi.fn()
      .mockResolvedValueOnce(new Response('{', { headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(response(paused))
      .mockResolvedValueOnce(refusal(code, code === 'RATE_LIMITED' ? 429 : 409))
      .mockResolvedValueOnce(response({ session_id: 'session', seat_id: 'player', event_cursor: '2', control: current }))
      .mockResolvedValueOnce(response(resumed))
      .mockResolvedValueOnce(response({ command_id: 'original', state_version: '2', event_cursor: '2', replayed: true, result: { kind: 'nil' }, control: resumed }))
    const player = await unknownCommand(transport)
    const original = player.state.pending
    await player.run(() => player.safety('pause'))
    await player.run(() => player.safety('resume'))
    expect(player.state.error).toBe(new APIError(code).message)
    expect(player.state.control).toEqual(current)
    expect(player.state.pending).toBe(original)
    expect(player.state.snapshot).toBeUndefined()
    expect(player.state.connection).toBeUndefined()
    expect(transport).toHaveBeenCalledTimes(4)
    expect(JSON.parse(transport.mock.calls[3][1].body).connection_id).toBe('original-connection')
    // Refreshing the control does not cast a vote. This person clicks again.
    await player.run(() => player.safety('resume'))
    expect(JSON.parse(transport.mock.calls[4][1].body).expected_control_revision).toBe('5')
    expect(player.state.pending).toBe(original)
    expect(player.state.control).toEqual(resumed)
    await player.retry()
    expect(player.state.pending).toBeUndefined()
    expect(player.state.notice).toBe('已找回原行动结果。')
    expect(transport.mock.calls[5][1].body).toBe(transport.mock.calls[0][1].body)
    expect(transport.mock.calls[5][1].headers).toEqual(transport.mock.calls[0][1].headers)
    expect(transport).toHaveBeenCalledTimes(6)
  })
  it('removes the pending safety controls when current pause authorization is revoked', async () => {
    const transport = vi.fn()
      .mockResolvedValueOnce(new Response('{', { headers: { 'Content-Type': 'application/json' } }))
      .mockResolvedValueOnce(refusal('DENIED', 403))
    const player = await unknownCommand(transport)
    await player.run(() => player.safety('pause'))
    expect(player.state.pending).toBeUndefined()
    expect(player.state.room).toBeUndefined()
    expect(player.state.control).toBeUndefined()
    const markup = renderToStaticMarkup(<App />)
    expect(markup).not.toContain('立即安全暂停')
    expect(markup).not.toContain('查询原操作结果')
    expect(markup).not.toContain('Private room')
  })
})
