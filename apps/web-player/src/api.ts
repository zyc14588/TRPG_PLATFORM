// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

const messages: Record<string, string> = {
  INVALID_REQUEST: '请检查填写的内容。',
  UNAUTHENTICATED: '登录已失效，请重新登录或使用邀请入场。',
  DENIED: '当前身份没有这项权限。请刷新房间或联系房主。',
  CONFLICT: '房间或游戏已发生变化，请刷新后重新确认。',
  CLAIM_REQUIRED: '请先认领当前访客身份。',
  RATE_LIMITED: '操作过于频繁，请稍后再试。',
  UNAVAILABLE: '服务暂时不可用，请稍后重试。',
  OUTCOME_UNKNOWN: '提交结果尚未确认。请查询原操作结果，避免重复行动。',
  PLAYER_PAUSED: '游戏已暂停，请等待参与者确认继续。',
  PLAYER_CONNECTION_EXPIRED: '连接已失效，请重新连接并确认继续。',
  NETWORK: '连接中断。请重新连接以恢复服务器保存的进度。',
}

export class APIError extends Error {
  constructor(readonly code: string) { super(messages[code] ?? messages.UNAVAILABLE) }
}

export type Attempt = Readonly<{ path: string; body: string; key: string; method: 'POST' | 'PUT' | 'DELETE' }>
export const identifier = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,127}$/
export function segment(value: string) {
  if (!identifier.test(value)) throw new APIError('INVALID_REQUEST')
  return value
}
export function attempt(path: string, fields: object = {}, method: Attempt['method'] = 'POST'): Attempt {
  const body = JSON.stringify({ schema_version: 1, ...fields })
  if (new TextEncoder().encode(body).length > 16384) throw new APIError('INVALID_REQUEST')
  return Object.freeze({ path, body, key: crypto.randomUUID(), method })
}

// Cookies remain HttpOnly. Neither credentials nor connection identifiers are
// stored in URLs, Web Storage, diagnostic output, or a service worker.
export class API {
  csrf = ''
  constructor(private readonly transport: typeof fetch = fetch) {}
  async get<T>(path: string): Promise<T> { return this.request<T>(path) }
  async send<T>(operation: Attempt): Promise<T> { return this.request<T>(operation.path, operation) }
  private async request<T>(path: string, operation?: Attempt): Promise<T> {
    if (!/^\/api\/v1\/[A-Za-z0-9/_.-]+$/.test(path)) throw new APIError('INVALID_REQUEST')
    const controller = new AbortController()
    const timer = setTimeout(() => controller.abort(), 8000)
    try {
      const response = await (0, this.transport)(path, {
        method: operation?.method ?? 'GET', credentials: 'same-origin',
        cache: 'no-store', redirect: 'error', referrerPolicy: 'no-referrer',
        signal: controller.signal,
        headers: operation ? { 'Content-Type': 'application/json', 'X-CSRF-Token': this.csrf, 'Idempotency-Key': operation.key } : undefined,
        body: operation?.body,
      })
      if (!response.headers.get('Content-Type')?.includes('application/json') || !response.body) throw new APIError(operation ? 'OUTCOME_UNKNOWN' : 'UNAVAILABLE')
      const reader = response.body.getReader()
      const chunks: Uint8Array[] = []
      let size = 0
      for (;;) {
        const next = await reader.read()
        if (next.done) break
        size += next.value.byteLength
        if (size > 256 * 1024) { await reader.cancel(); throw new APIError(operation ? 'OUTCOME_UNKNOWN' : 'UNAVAILABLE') }
        chunks.push(next.value)
      }
      const bytes = new Uint8Array(size)
      let offset = 0
      for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length }
      let envelope: { schema_version?: number; data?: T; error?: { code?: string } }
      try { envelope = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)) } catch { throw new APIError(operation ? 'OUTCOME_UNKNOWN' : 'UNAVAILABLE') }
      if (envelope.schema_version !== 1) throw new APIError(operation ? 'OUTCOME_UNKNOWN' : 'UNAVAILABLE')
      if (!response.ok || envelope.error) throw new APIError(envelope.error?.code ?? 'UNAVAILABLE')
      if (!('data' in envelope)) throw new APIError(operation ? 'OUTCOME_UNKNOWN' : 'UNAVAILABLE')
      return envelope.data as T
    } catch (error) {
      if (error instanceof APIError) throw error
      throw new APIError(operation ? 'OUTCOME_UNKNOWN' : 'NETWORK')
    } finally { clearTimeout(timer) }
  }
}
