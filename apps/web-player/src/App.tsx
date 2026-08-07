// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

function Divider() {
  return (
    <div className="divider" aria-hidden="true">
      <span />
      <i />
      <span />
    </div>
  )
}

export function App() {
  return (
    <main className="baseline-shell" aria-labelledby="baseline-title">
      <div className="contour contour-top" aria-hidden="true" />
      <div className="contour contour-bottom" aria-hidden="true" />

      <header className="brand">TRPG Platform</header>

      <section className="baseline-status">
        <Divider />
        <h1 id="baseline-title">V1 restart baseline</h1>
        <p className="status-primary">No playable functionality</p>
        <div className="status-rule" aria-hidden="true" />
        <p className="status-secondary">M0 engineering shell only</p>
      </section>

      <div className="footer-rule" aria-hidden="true" />
    </main>
  )
}
