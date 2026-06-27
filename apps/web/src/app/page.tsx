import Link from "next/link";

export default function HomePage() {
  return (
    <main className="page page-narrow">
      <section className="panel">
        <p className="eyebrow">Rules & RAG foundation</p>
        <h1>TRPG Platform</h1>
        <p className="muted">
          Rooms, rules, and secure retrieval foundations for online TRPG play.
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
