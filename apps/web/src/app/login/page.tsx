"use client";

import Link from "next/link";
import { FormEvent, useState } from "react";
import { apiErrorMessage, createTrpgApiClient } from "../../lib/backend";

export default function LoginPage() {
  const [email, setEmail] = useState("owner@example.test");
  const [magicLink, setMagicLink] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (loading) {
      return;
    }
    setLoading(true);
    setError(null);
    setMessage(null);
    setMagicLink(null);

    try {
      const redirectUri = `${window.location.origin}/auth/callback`;
      const response = await createTrpgApiClient().requestMagicLink(email, redirectUri);
      setMessage("Magic Link 已发送。开发模式会在这里显示可点击链接。");
      setMagicLink(response.development_magic_link);
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <main className="page page-narrow">
      <section className="panel">
        <p className="eyebrow">登录</p>
        <h1>Magic Link</h1>
        <form className="stack" onSubmit={submit}>
          <label className="field">
            <span>Email</span>
            <input
              type="email"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
              required
            />
          </label>
          <button className="button button-primary" disabled={loading} type="submit">
            {loading ? "发送中..." : "发送 Magic Link"}
          </button>
        </form>

        {message ? <p className="status">{message}</p> : null}
        {magicLink ? (
          <p>
            <Link className="button" href={magicLink}>
              使用开发 Magic Link 登录
            </Link>
          </p>
        ) : null}
        {error ? <p className="error">{error}</p> : null}
      </section>
    </main>
  );
}
