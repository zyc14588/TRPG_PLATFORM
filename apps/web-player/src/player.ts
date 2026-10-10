// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

import { API, APIError, attempt, segment, type Attempt } from './api'
import type { Admission, Connection, Consent, Context, Control, Export, Game, Invitation, Lobby, Presentation, RecoveryPoint, Result, Room, Slot, Snapshot, Value } from './types'

export type PlayerState = {
  context?: Context; workspace: string; games: Game[]; room?: Room; lobby?: Lobby;
  presentation?: Presentation; selected?: Game; admission?: Admission; invitations: Invitation[];
  queue: Admission[]; connection?: Connection; snapshot?: Snapshot; control?: Control;
  point?: RecoveryPoint; exported?: Export; busy: boolean; error: string; notice: string;
  exportLimit?: number;
  pending?: { attempt: Attempt; connection?: Connection; finish: (result: unknown) => Promise<void> };
}
export function presentationMatches(room: Room, lobby: Lobby, p: Presentation) {
  return p.workspace_id === room.workspace_id && p.game_id === room.game_id &&
    p.configuration_id === lobby.configuration_id && p.configuration_hash === lobby.configuration_hash && p.graph_hash === lobby.graph_hash && p.packages.length > 0
}
export function roomPath(w: string, r: string) { return `/api/v1/workspaces/${segment(w)}/rooms/${segment(r)}` }
function identity(context?: Context) {
  const p = context?.principal
  return p?.kind === 'account' ? `account/${p.account_id}` : p?.kind === 'guest' ? `guest/${p.participation_id}/${p.scope.workspace_id}/${p.scope.room_id}/${p.scope.game_id}` : 'anonymous'
}

// This controller owns only transient UI projections. Every permission, seat,
// preparation revision, event and command result comes from the HTTPS service.
export class Player {
  state: PlayerState = { workspace: '', games: [], queue: [], invitations: [], busy: false, error: '', notice: '' }
  private listeners = new Set<() => void>()
  private epoch = 0
  private polling = false
  private pollTask?: Promise<void>
  private lastSnapshotAt = 0
  private cursor = '0'
  constructor(readonly api = new API()) {}
  subscribe = (fn: () => void) => { this.listeners.add(fn); return () => { this.listeners.delete(fn) } }
  snapshot = () => this.state
  private set(patch: Partial<PlayerState>) { this.state = { ...this.state, ...patch }; this.listeners.forEach(f => f()) }
  private clearPrivate() {
    this.epoch++
    this.cursor = '0'
    this.set({ games: [], room: undefined, lobby: undefined, presentation: undefined, selected: undefined, admission: undefined, queue: [], invitations: [], connection: undefined, snapshot: undefined, control: undefined, exported: undefined, point: undefined })
  }
  private failure(error: unknown) {
    const e = error instanceof APIError ? error : new APIError('UNAVAILABLE')
    if (e.code === 'DENIED' || e.code === 'UNAUTHENTICATED') {
      this.clearPrivate()
      this.set({ pending: undefined })
      if (e.code === 'UNAUTHENTICATED') this.set({ context: undefined })
    } else if (['NETWORK', 'OUTCOME_UNKNOWN', 'PLAYER_CONNECTION_EXPIRED'].includes(e.code)) {
      this.set({ connection: undefined, snapshot: undefined, exported: undefined })
    }
    this.set({ error: e.message })
  }
  async run(fn: () => Promise<void>) {
    if (this.state.busy) return
    this.set({ busy: true, error: '', notice: '' })
    try { await this.pollTask; await fn() } catch (e) { this.failure(e) } finally { this.set({ busy: false }) }
  }
  async context() {
    const context = await this.api.get<Context>('/api/v1/auth/context')
    if (identity(context) !== identity(this.state.context) || (this.state.context?.state === 'authenticated' && this.state.context.csrf_token !== context.csrf_token)) {
      this.clearPrivate()
      this.set({ pending: undefined, workspace: '' })
    }
    this.api.csrf = context.csrf_token
    this.set({ context })
    return context
  }
  async initialize() { await this.run(async () => { await this.context() }) }
  private async write<T>(path: string, body: object, finish: (result: T) => Promise<void>, method: Attempt['method'] = 'POST') {
    if (this.state.pending) throw new APIError('OUTCOME_UNKNOWN')
    const operation = attempt(path, body, method)
    const pending = { attempt: operation, connection: this.state.connection, finish: finish as (result: unknown) => Promise<void> }
    let data: T
    try {
      data = await this.api.send<T>(operation)
    } catch (e) {
      if (e instanceof APIError && e.code === 'OUTCOME_UNKNOWN') this.set({ pending })
      throw e
    }
    await finish(data)
  }
  async retry() {
    const p = this.state.pending
    if (!p) return
    await this.run(async () => {
      try {
        const data = await this.api.send(p.attempt)
        this.set({ pending: undefined })
        await p.finish(data)
      } catch (e) {
        if (e instanceof APIError && !['OUTCOME_UNKNOWN', 'NETWORK', 'UNAVAILABLE', 'RATE_LIMITED', 'PLAYER_PAUSED'].includes(e.code)) this.set({ pending: undefined })
        throw e
      }
    })
  }
  async authenticate(path: string, fields: object) {
    await this.run(async () => {
      await this.context()
      await this.write<Context | { context: Context }>(path, fields, async data => {
        const context = 'context' in data ? data.context : data
        const target = this.state.room
        this.clearPrivate()
        this.api.csrf = context.csrf_token
        this.set({ context, pending: undefined, admission: undefined })
        if (target && path.endsWith('/claim')) await this.loadRoom(target.workspace_id, target.room_id)
      })
    })
  }
  async catalog(w: string) {
    this.set({ presentation: undefined, selected: undefined, games: [] })
    const catalog = await this.api.get<{ games: Game[] }>(`/api/v1/workspaces/${segment(w)}/games`)
    this.set({ workspace: w, games: catalog.games })
  }
  async select(game: Game) {
    this.set({ selected: game, presentation: undefined })
    const presentation = await this.api.get<Presentation>(`/api/v1/workspaces/${segment(this.state.workspace)}/games/${segment(game.configuration_id)}/presentation`)
    if (presentation.workspace_id !== this.state.workspace || presentation.configuration_id !== game.configuration_id || presentation.game_id !== game.game_id || !presentation.packages.length) throw new APIError('CONFLICT')
    this.set({ presentation })
  }
  async loadRoom(w: string, r: string) {
    if (this.state.room && (this.state.room.workspace_id !== w || this.state.room.room_id !== r)) this.clearPrivate()
    this.set({ lobby: undefined, presentation: undefined, selected: undefined, games: [], exported: undefined, invitations: [], queue: [] })
    const path = roomPath(w, r)
    const room = await this.api.get<Room>(path)
    if (room.workspace_id !== w || room.room_id !== r || room.state === 'closed') throw new APIError('DENIED')
    this.set({ room, workspace: w })
    if (typeof window !== 'undefined') history.replaceState(null, '', `#room/${segment(w)}/${segment(r)}`)
    const lobby = await this.api.get<Lobby>(path + '/preparation')
    this.set({ lobby })
    // A participant's room admission authorizes this room, independently of
    // workspace catalog membership. Only management needs catalog selection.
    if (lobby.can_manage && room.state === 'lobby') await this.catalog(w)
    if (lobby.configuration_id) await this.roomPresentation(room, lobby)
    if (lobby.can_manage && room.state === 'lobby') {
      const q = await this.api.get<{ admissions: Admission[] }>(path + '/admissions')
      this.set({ queue: q.admissions })
    }
  }
  private async roomPresentation(room: Room, lobby: Lobby) {
    this.set({ presentation: undefined })
    const presentation = await this.api.get<Presentation>(roomPath(room.workspace_id, room.room_id) + '/presentation')
    if (!presentationMatches(room, lobby, presentation)) throw new APIError('CONFLICT')
    this.set({ presentation, selected: this.state.games.find(g => g.configuration_id === lobby.configuration_id) })
    return presentation
  }
  path() {
    const r = this.state.room
    if (!r) throw new APIError('INVALID_REQUEST')
    return roomPath(r.workspace_id, r.room_id)
  }
  async refresh() {
    const r = this.state.room
    if (r) await this.loadRoom(r.workspace_id, r.room_id)
  }
  async createRoom(name: string) {
    const game = this.state.selected
    if (!game || !this.state.presentation) throw new APIError('CONFLICT')
    await this.write<Room>(`/api/v1/workspaces/${segment(this.state.workspace)}/rooms`, { name, game_id: game.game_id }, async room => {
      await this.loadRoom(room.workspace_id, room.room_id)
      await this.select(game)
      this.set({ notice: '房间已创建。先通过邀请加入，再安排参与者席位。' })
    })
  }
  async invite(approval: boolean, seconds: number, uses: number) {
    await this.write<Invitation>(this.path() + '/invitations', { approval_required: approval, expires_in_seconds: seconds, max_uses: uses }, async invitation => { this.set({ invitations: [...this.state.invitations, invitation] }) })
  }
  dismissInvitations() { this.set({ invitations: [] }) }
  async join(secret: string, name: string) {
    const mode = this.state.context?.principal?.kind === 'account' ? 'account' : 'guest'
    const credential = /^[0123456789ABCDEFGHJKMNPQRSTVWXYZ]{16}$/.test(secret) ? { room_code: secret } : { invite_token: secret }
    await this.write<Admission>('/api/v1/room-admissions', { mode, ...credential, ...(mode === 'guest' ? { display_name: name } : {}) }, async admission => { this.set({ admission }); await this.enter(admission) })
  }
  private async enter(admission: Admission) {
    if (admission.status !== 'approved') return
    if (admission.mode === 'guest') {
      await this.write<{ admission_token: string }>(`/api/v1/room-admissions/${segment(admission.admission_id)}/guest-token`, {}, async token => {
        await this.write<Context>('/api/v1/auth/guest/exchange', { admission_token: token.admission_token }, async context => {
          this.clearPrivate(); this.api.csrf = context.csrf_token; this.set({ context, admission: undefined })
          await this.loadRoom(admission.workspace_id, admission.room_id)
        })
      })
    } else { this.set({ admission: undefined }); await this.loadRoom(admission.workspace_id, admission.room_id) }
  }
  async admissionStatus() {
    const id = this.state.admission?.admission_id
    if (!id) return
    const admission = await this.api.get<Admission>(`/api/v1/room-admissions/${segment(id)}`)
    this.set({ admission }); await this.enter(admission)
  }
  async decision(id: string, decision: 'approve' | 'reject') {
    await this.write(this.path() + `/admissions/${segment(id)}/decision`, { decision }, async () => this.refresh())
  }
  async role(id: string, role: 'host' | 'administrator', enabled: boolean) {
    await this.write(this.path() + `/participants/${segment(id)}/role`, { role, enabled }, async () => this.refresh(), 'PUT')
  }
  async configure(configuration: string, slots: Slot[]) {
    await this.write<Lobby>(this.path() + '/preparation', { configuration_id: configuration, slots }, async lobby => {
      // Configure already returns the authoritative new preparation. Avoid
      // repeating the room/catalog/preparation requests after the same write.
      this.set({ lobby, presentation: undefined })
      if (!this.state.room) throw new APIError('CONFLICT')
      await this.roomPresentation(this.state.room, lobby)
    })
  }
  async consent(own: Omit<Consent, 'revision'>) {
    const { room, lobby, presentation } = this.state
    if (!room || !lobby || !presentation || !presentationMatches(room, lobby, presentation)) throw new APIError('CONFLICT')
    const rev = lobby.revision
    // Recheck both presentation digests before approval. The consent mutation
    // atomically rejects an outdated preparation revision on the server.
    const p = await this.api.get<Presentation>(this.path() + '/presentation')
    if (!presentationMatches(room, lobby, p)) {
      this.set({ presentation: undefined }); throw new APIError('CONFLICT')
    }
    await this.write(this.path() + '/consent', { revision: rev, ...own }, async () => {
      const lobby = await this.api.get<Lobby>(this.path() + '/preparation')
      this.set({ lobby })
      if (!presentationMatches(room, lobby, p)) { this.set({ presentation: undefined }); throw new APIError('CONFLICT') }
      this.set({ presentation: p })
    })
  }
  async launch() {
    const revision = this.state.lobby?.revision
    if (!revision) throw new APIError('CONFLICT')
    try {
      await this.write(this.path() + '/launch', { revision }, async () => { await this.refresh(); this.set({ notice: '游戏已开始。入座的参与者可以连接游戏。' }) })
    } catch (error) {
      if (!(error instanceof APIError) || !['DENIED', 'CONFLICT'].includes(error.code)) throw error
      // A failed launch can mean incomplete readiness. Reauthorize the lobby
      // before retaining its projection, then show the current preparation.
      await this.refresh()
      this.set({ error: '准备检查未通过。请根据当前房间状态重新确认席位、包与个人准备。' })
    }
  }
  async connect() {
    const after = this.cursor
    await this.context()
    const connection = await this.api.send<Connection>(attempt(this.path() + '/session/connect', { after_cursor: after }))
    this.set({ connection, control: connection.control, snapshot: undefined, exported: undefined })
    await this.readSnapshot()
  }
  private async readSnapshot() {
    const active = this.state.connection
    const pending = this.state.pending
    const connection = active ?? pending?.connection
    if (!connection) return
    const epoch = this.epoch
    const cursor = this.cursor
    this.lastSnapshotAt = Date.now()
    const snapshot = await this.api.send<Snapshot>(attempt(this.path() + '/session/snapshot', { connection_id: connection.connection_id, after_cursor: cursor, limit: 128 }))
    if (epoch !== this.epoch || (active ? active !== this.state.connection : pending !== this.state.pending)) return
    if (snapshot.session_id !== connection.session_id || snapshot.seat_id !== connection.seat_id) throw new APIError('DENIED')
    if (!active) {
      // Keep the unknown command's original lease for safety revision reads.
      // Neither restore its removed private view nor consume unseen events.
      this.set({ control: snapshot.control })
      return
    }
    this.cursor = snapshot.event_cursor
    this.set({ snapshot, control: snapshot.control })
  }
  async poll() {
    // The production admission budget is shared with user operations. Keep
    // idle refreshes well below ten snapshots per minute and renew the lease
    // within its thirty-second lifetime; explicit action reads reset this gap.
    if (!this.state.connection || this.state.busy || this.polling || this.state.pending || Date.now() - this.lastSnapshotAt < 10000) return
    this.polling = true
    this.pollTask = (async () => {
      try { await this.readSnapshot() } catch (e) { this.failure(e) } finally { this.polling = false }
    })()
    await this.pollTask
  }
  async command(type: string, payload: Value) {
    const { connection, snapshot, control } = this.state
    if (!connection || !snapshot || control?.paused || snapshot.more || snapshot.ended) throw new APIError('PLAYER_PAUSED')
    await this.write<Result>(this.path() + '/session/commands', { connection_id: connection.connection_id, command_id: crypto.randomUUID(), expected_state_version: snapshot.state_version, type: segment(type), payload, correlation_id: crypto.randomUUID() }, async result => {
      this.set({ control: result.control, notice: result.replayed ? '已找回原行动结果。' : '行动已由服务器保存。' })
      await this.readSnapshot()
    })
  }
  async safety(action: 'pause' | 'resume') {
    const control = this.state.control
    if (!control) throw new APIError('CONFLICT')
    if (action === 'pause') {
      // Safety remains available while an unrelated command result is unknown.
      const paused = await this.api.send<Control>(attempt(this.path() + '/session/pause', { expected_control_revision: control.revision }))
      this.set({ control: paused, exported: undefined })
    } else {
      try {
        const path = this.path() + '/session/resume'
        const fields = { expected_control_revision: control.revision }
        const finish = async (control: Control) => { this.set({ control, exported: undefined }) }
        if (this.state.pending) {
          // An explicit safety confirmation must not replace the unknown game
          // command. The server still requires every current human's vote.
          await finish(await this.api.send<Control>(attempt(path, fields)))
        } else await this.write<Control>(path, fields, finish)
      } catch (error) {
        if (error instanceof APIError && ['CONFLICT', 'RATE_LIMITED'].includes(error.code)) {
          // A different participant may be confirming concurrently. Read the
          // current revision, retain the visible refusal, and let this person
          // explicitly confirm again; never manufacture a successful vote.
          await this.readSnapshot()
        }
        throw error
      }
    }
  }
  async disconnect() {
    const connection = this.state.connection
    if (!connection) return
    await this.write<Control>(this.path() + '/session/disconnect', { connection_id: connection.connection_id }, async control => {
      this.epoch++; this.set({ connection: undefined, snapshot: undefined, exported: undefined, control, notice: '已离开游戏连接。重新连接后请确认继续。' })
    })
  }
  async recovery(create: boolean) {
    const path = this.path() + '/session/recovery-point'
    if (create) await this.write<RecoveryPoint>(path, {}, async point => { this.set({ point }) })
    else this.set({ point: await this.api.get<RecoveryPoint>(path) })
  }
  async exportPage(kind: Export['kind'], next = false, limit = this.state.exportLimit ?? 128) {
    if (!Number.isInteger(limit) || limit < 1 || limit > 128) throw new APIError('INVALID_REQUEST')
    const cursor = next && this.state.exported?.kind === kind ? this.state.exported.next_cursor : '0'
    this.set({ exported: undefined, exportLimit: limit })
    const exported = await this.api.send<Export>(attempt(this.path() + '/session/export', { kind, after_cursor: cursor, limit }))
    this.set({ exported })
  }
}
