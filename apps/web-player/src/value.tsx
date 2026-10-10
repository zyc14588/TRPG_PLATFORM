// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

import type { Value } from './types'

// Render only the seat-filtered DTO. Strings are React text, never HTML or URLs.
export function GameValue({ value, depth = 0 }: { value: Value; depth?: number }) {
  if (depth > 32) return <span>内容过深，无法显示。</span>
  switch (value.kind) {
    case 'nil': return <span>—</span>
    case 'string': return <span>{value.string ?? ''}</span>
    case 'integer': case 'float': return <span>{value.number}</span>
    case 'boolean': return <span>{value.boolean ? '是' : '否'}</span>
    case 'array': return <ol>{(value.array ?? []).map((v, i) => <li key={i}><GameValue value={v} depth={depth + 1} /></li>)}</ol>
    case 'table': return <dl className="game-values">{Object.entries(value.table ?? {}).map(([k, v]) => <div key={k}><dt>{k}</dt><dd><GameValue value={v} depth={depth + 1} /></dd></div>)}</dl>
  }
}

// Generic structured actions preserve integer precision. Game rules remain on
// the server; this encoder only represents a player's chosen fields.
export function encodeValue(value: unknown, depth = 0): Value {
  if (depth > 30) throw new Error('行动内容层级过多。')
  if (value === null) return { kind: 'nil' }
  if (typeof value === 'string' && !value.startsWith('cap:')) return { kind: 'string', string: value }
  if (typeof value === 'boolean') return { kind: 'boolean', boolean: value }
  if (typeof value === 'number' && Number.isFinite(value)) {
    if (Number.isInteger(value) && !Number.isSafeInteger(value)) throw new Error('整数超出安全范围。')
    return { kind: Number.isInteger(value) ? 'integer' : 'float', number: String(value) }
  }
  if (Array.isArray(value)) return { kind: 'array', array: value.map(v => encodeValue(v, depth + 1)) }
  if (value && typeof value === 'object') {
    const entries = Object.entries(value)
    if (entries.some(([k]) => k.startsWith('cap:'))) throw new Error('行动字段无效。')
    return { kind: 'table', table: Object.fromEntries(entries.map(([k, v]) => [k, encodeValue(v, depth + 1)])) }
  }
  throw new Error('行动内容无效。')
}
