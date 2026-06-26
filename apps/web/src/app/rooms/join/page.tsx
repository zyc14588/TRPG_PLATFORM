"use client";

import Link from "next/link";
import { useSearchParams } from "next/navigation";
import { FormEvent, Suspense, useState } from "react";
import {
  RoomResponse,
  apiErrorMessage,
  createTrpgApiClient,
  newIdempotencyKey
} from "../../../lib/backend";

export default function JoinRoomPage() {
  return (
    <Suspense fallback={<JoinShell initialToken="" />}>
      <JoinRoomInner />
    </Suspense>
  );
}

function JoinRoomInner() {
  const searchParams = useSearchParams();
  return <JoinShell initialToken={searchParams.get("token") ?? ""} />;
}

function JoinShell({ initialToken }: { initialToken: string }) {
  const [token, setToken] = useState(initialToken);
  const [key, setKey] = useState(() => newIdempotencyKey("accept-invite"));
  const [joined, setJoined] = useState<RoomResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitting) {
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const response = await createTrpgApiClient().acceptInvitation(token, key);
      setJoined(response);
      setKey(newIdempotencyKey("accept-invite"));
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="page page-narrow">
      <section className="panel">
        <p className="eyebrow">Join</p>
        <h1>加入房间</h1>
        <form className="stack" onSubmit={submit}>
          <label className="field">
            <span>邀请 token</span>
            <input value={token} onChange={(event) => setToken(event.target.value)} required />
          </label>
          <button className="button button-primary" disabled={submitting} type="submit">
            {submitting ? "加入中..." : "加入"}
          </button>
        </form>

        {joined ? (
          <p className="status">
            已加入 {joined.room.title}，角色为 {joined.room.my_role}。{" "}
            <Link href="/rooms">查看房间</Link>
          </p>
        ) : null}
        {error ? <p className="error">{error}</p> : null}
      </section>
    </main>
  );
}
