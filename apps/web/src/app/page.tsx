import Link from "next/link";

export default function HomePage() {
  return (
    <main className="page page-narrow">
      <section className="panel">
        <p className="eyebrow">Phase 1B</p>
        <h1>TRPG Platform</h1>
        <p className="muted">
          最小 Auth 与 Room 纵向流程已接入。正式主题、地图、音频、线索图和动画留给后续阶段。
        </p>
        <div className="actions">
          <Link className="button button-primary" href="/login">
            登录
          </Link>
          <Link className="button" href="/rooms">
            我的房间
          </Link>
        </div>
      </section>
    </main>
  );
}
