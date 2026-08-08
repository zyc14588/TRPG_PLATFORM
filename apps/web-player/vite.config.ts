// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  build: {
    sourcemap: true,
  },
})
