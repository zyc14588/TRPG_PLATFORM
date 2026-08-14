/* SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0 */

import type { CreatorServiceBinding } from './bridge'

declare global {
  interface Window {
    go?: {
      creator?: {
        Service?: CreatorServiceBinding
      }
    }
  }
}

export {}
