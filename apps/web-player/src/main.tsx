// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

import { App } from './App'
import './styles.css'

const root = document.getElementById('root')

if (root === null) {
  throw new Error('Web Player root element is missing')
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
