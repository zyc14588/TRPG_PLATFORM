"use client";

import Link from "next/link";
import { FormEvent, useEffect, useState } from "react";
import {
  CreateInvitationResponse,
  ListRoomMembersResponse,
  RoomDto,
  apiErrorMessage,
  createTrpgApiClient,
  newIdempotencyKey
} from "../../lib/backend";

export default function RoomsPage() {
  const [rooms, setRooms] = useState<RoomDto[]>([]);
  const [status, setStatus] = useState("loading");
  const [error, setError] = useState<string | null>(null);

  async function load() {
    setStatus("loading");
    setError(null);
    try {
      const response = await createTrpgApiClient().listRooms();
      setRooms(response.rooms);
      setStatus(response.rooms.length === 0 ? "empty" : "ready");
    } catch (err) {
      setStatus("unauthorized");
      setError(apiErrorMessage(err));
    }
  }

  useEffect(() => {
    void load();
  }, []);

  async function logout() {
    await createTrpgApiClient().logout();
    setRooms([]);
    setStatus("unauthorized");
  }

  return (
    <main className="page">
      <header className="topbar">
        <div>
          <p className="eyebrow">Rooms</p>
          <h1>我的房间</h1>
        </div>
        <nav className="actions">
          <Link className="button button-primary" href="/rooms/new">
            创建房间
          </Link>
          <Link className="button" href="/rooms/join">
            加入房间
          </Link>
          <button className="button" onClick={logout} type="button">
            登出
          </button>
        </nav>
      </header>

      {status === "loading" ? <p className="status">正在载入房间...</p> : null}
      {status === "empty" ? <p className="status">还没有房间。创建一个就能开始邀请玩家。</p> : null}
      {status === "unauthorized" ? (
        <section className="panel">
          <h2>需要登录</h2>
          <p className="error">{error ?? "请先登录。"}</p>
          <Link className="button button-primary" href="/login">
            去登录
          </Link>
        </section>
      ) : null}

      <section className="room-grid">
        {rooms.map((room) => (
          <article className="panel" key={room.id}>
            <div className="room-card-heading">
              <div>
                <h2>{room.title}</h2>
                <p className="muted">
                  {room.system_name} / {room.privacy_mode} / {room.my_role}
                </p>
              </div>
              <span className="badge">v{room.version}</span>
            </div>
            <MembersPanel roomId={room.id} />
            {room.my_role === "owner" ? <InviteForm roomId={room.id} /> : null}
          </article>
        ))}
      </section>
    </main>
  );
}

function MembersPanel({ roomId }: { roomId: string }) {
  const [members, setMembers] = useState<ListRoomMembersResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function loadMembers() {
    setLoading(true);
    setError(null);
    try {
      setMembers(await createTrpgApiClient().listMembers(roomId));
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  return (
    <div className="stack">
      <button className="button" disabled={loading} onClick={loadMembers} type="button">
        {loading ? "读取成员..." : "查看成员"}
      </button>
      {members ? (
        <ul className="compact-list">
          {members.members.map((member) => (
            <li key={member.user_id}>
              <span>{member.role}</span>
              <code>{member.user_id.slice(0, 8)}</code>
            </li>
          ))}
        </ul>
      ) : null}
      {error ? <p className="error">{error}</p> : null}
    </div>
  );
}

function InviteForm({ roomId }: { roomId: string }) {
  const [email, setEmail] = useState("player@example.test");
  const [key, setKey] = useState(() => newIdempotencyKey("invite"));
  const [invite, setInvite] = useState<CreateInvitationResponse | null>(null);
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
      const response = await createTrpgApiClient().createInvitation(roomId, {
        email,
        role: "pl",
        idempotency_key: key
      });
      setInvite(response);
      setKey(newIdempotencyKey("invite"));
    } catch (err) {
      setError(apiErrorMessage(err));
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form className="stack" onSubmit={submit}>
      <label className="field">
        <span>邀请邮箱</span>
        <input
          type="email"
          value={email}
          onChange={(event) => setEmail(event.target.value)}
          required
        />
      </label>
      <button className="button" disabled={submitting} type="submit">
        {submitting ? "生成中..." : "生成邀请"}
      </button>
      {invite ? (
        <p className="status">
          邀请已生成：
          <Link href={invite.invitation_url}>{invite.invitation_url}</Link>
        </p>
      ) : null}
      {error ? <p className="error">{error}</p> : null}
    </form>
  );
}
