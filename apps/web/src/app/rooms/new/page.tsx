"use client";

import Link from "next/link";
import { FormEvent, useState } from "react";
import {
  RoomPrivacyMode,
  RoomResponse,
  apiErrorMessage,
  createTrpgApiClient,
  newIdempotencyKey
} from "../../../lib/backend";

export default function NewRoomPage() {
  const [title, setTitle] = useState("Friday Game");
  const [systemName, setSystemName] = useState("generic_percentile");
  const [privacyMode, setPrivacyMode] = useState<RoomPrivacyMode>("private_hybrid");
  const [idempotencyKey, setIdempotencyKey] = useState(() => newIdempotencyKey("create-room"));
  const [created, setCreated] = useState<RoomResponse | null>(null);
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
      const response = await createTrpgApiClient().createRoom({
        title,
        system_name: systemName,
        privacy_mode: privacyMode,
        idempotency_key: idempotencyKey
      });
      setCreated(response);
      setIdempotencyKey(newIdempotencyKey("create-room"));
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <main className="page page-narrow">
      <section className="panel">
        <p className="eyebrow">New room</p>
        <h1>创建房间</h1>
        <form className="stack" onSubmit={submit}>
          <label className="field">
            <span>房间名</span>
            <input value={title} onChange={(event) => setTitle(event.target.value)} required />
          </label>
          <label className="field">
            <span>规则系统</span>
            <input
              value={systemName}
              onChange={(event) => setSystemName(event.target.value)}
              required
            />
          </label>
          <label className="field">
            <span>隐私模式</span>
            <select
              value={privacyMode}
              onChange={(event) => setPrivacyMode(event.target.value as RoomPrivacyMode)}
            >
              <option value="standard">standard</option>
              <option value="private_hybrid">private_hybrid</option>
              <option value="local_only">local_only</option>
            </select>
          </label>
          <button className="button button-primary" disabled={submitting} type="submit">
            {submitting ? "创建中..." : "创建"}
          </button>
        </form>

        {created ? (
          <p className="status">
            已创建 {created.room.title}。<Link href="/rooms">返回房间列表</Link>
          </p>
        ) : null}
        {error ? <p className="error">{error}</p> : null}
      </section>
    </main>
  );
}
