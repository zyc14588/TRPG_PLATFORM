// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { App } from './App'

describe('Web Player M0 baseline', () => {
  it('states that no playable functionality exists', () => {
    const markup = renderToStaticMarkup(<App />)

    expect(markup).toContain('V1 restart baseline')
    expect(markup).toContain('No playable functionality')
    expect(markup).toContain('M0 engineering shell only')
    expect(markup).not.toContain('<button')
    expect(markup).not.toContain('<form')
  })
})
