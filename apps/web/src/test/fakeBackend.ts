import type {
  Fetcher,
  InvitationRole,
  RoomDto,
  RoomPrivacyMode,
  RoomRole
} from "../lib/backend";

interface FakeUser {
  id: string;
  email: string;
  display_name: string;
}

interface FakeRoom {
  id: string;
  owner_id: string;
  title: string;
  system_name: string;
  privacy_mode: RoomPrivacyMode;
  version: number;
}

interface FakeInvite {
  token: string;
  room_id: string;
  email: string;
  role: InvitationRole;
  accepted: boolean;
}

interface IdempotentResponse {
  hash: string;
  body: unknown;
}

export class FakeBackend {
  readonly fetch: Fetcher = async (input, init) => {
    const url = new URL(input.toString(), "http://ui.test");
    const method = init?.method ?? "GET";
    const body = parseBody(init?.body);

    if (method === "POST" && url.pathname === "/api/auth/magic-link/request") {
      const email = stringBodyField(body, "email").toLowerCase();
      const token = this.id("ml");
      this.magicLinks.set(token, email);
      return json(200, {
        status: "sent",
        challenge_id: this.id("challenge"),
        expires_at_unix: 4_102_444_800,
        development_magic_link: `http://localhost/auth/callback?token=${token}`
      });
    }

    if (method === "POST" && url.pathname === "/api/auth/magic-link/verify") {
      const token = stringBodyField(body, "token");
      const email = this.magicLinks.get(token);
      if (!email || this.usedMagicLinks.has(token)) {
        return json(401, { error: "unauthorized", message: "invalid magic link" });
      }
      this.usedMagicLinks.add(token);
      const user = this.user(email);
      const accessToken = this.id(`access_${user.id}`);
      this.sessions.set(accessToken, user.id);
      return json(200, authSession(accessToken, user));
    }

    const user = this.authenticatedUser(init);
    if (!user) {
      return json(401, { error: "unauthorized", message: "access token is required" });
    }

    if (method === "GET" && url.pathname === "/api/me") {
      return json(200, { user });
    }

    if (method === "GET" && url.pathname === "/api/rooms") {
      const rooms = [...this.members.entries()]
        .filter(([, members]) => user.id in members)
        .map(([roomId, members]) => roomDto(this.rooms.get(roomId), members[user.id]))
        .filter((room): room is RoomDto => room !== null);
      return json(200, { rooms });
    }

    if (method === "POST" && url.pathname === "/api/rooms") {
      const key = stringBodyField(body, "idempotency_key");
      const replay = this.idempotent(`create:${user.id}:${key}`, body);
      if (replay) {
        return json(200, replay);
      }

      const room: FakeRoom = {
        id: this.id("room"),
        owner_id: user.id,
        title: stringBodyField(body, "title"),
        system_name: stringBodyField(body, "system_name"),
        privacy_mode: privacyModeField(body),
        version: 0
      };
      this.rooms.set(room.id, room);
      this.members.set(room.id, { [user.id]: "owner" });
      const response = { room: roomDto(room, "owner") };
      this.rememberIdempotent(`create:${user.id}:${key}`, body, response);
      return json(200, response);
    }

    const roomMatch = url.pathname.match(/^\/api\/rooms\/([^/]+)$/);
    if (method === "GET" && roomMatch) {
      const roomId = roomMatch[1];
      const role = this.role(roomId, user.id);
      const dto = role ? roomDto(this.rooms.get(roomId), role) : null;
      return dto ? json(200, { room: dto }) : json(404, { error: "not_found", message: "room not found" });
    }

    const inviteMatch = url.pathname.match(/^\/api\/rooms\/([^/]+)\/invitations$/);
    if (method === "POST" && inviteMatch) {
      const roomId = inviteMatch[1];
      if (this.role(roomId, user.id) !== "owner") {
        return json(403, { error: "forbidden", message: "only room owner can invite" });
      }
      const role = invitationRoleField(body);
      const token = this.id("invite");
      const invite: FakeInvite = {
        token,
        room_id: roomId,
        email: stringBodyField(body, "email").toLowerCase(),
        role,
        accepted: false
      };
      this.invites.set(token, invite);
      return json(200, {
        room_id: roomId,
        invited_email: invite.email,
        role,
        expires_at_unix: 4_102_444_800,
        token,
        invitation_url: `/rooms/join?token=${token}`
      });
    }

    const acceptMatch = url.pathname.match(/^\/api\/room-invitations\/([^/]+)\/accept$/);
    if (method === "POST" && acceptMatch) {
      const token = decodeURIComponent(acceptMatch[1]);
      const invite = this.invites.get(token);
      if (!invite) {
        return json(404, { error: "not_found", message: "invitation not found" });
      }
      if (invite.email !== user.email) {
        return json(403, { error: "forbidden", message: "invitation is not for this user" });
      }
      invite.accepted = true;
      const members = this.members.get(invite.room_id);
      if (members) {
        members[user.id] = invite.role;
      }
      return json(200, { room: roomDto(this.rooms.get(invite.room_id), invite.role) });
    }

    const membersMatch = url.pathname.match(/^\/api\/rooms\/([^/]+)\/members$/);
    if (method === "GET" && membersMatch) {
      const roomId = membersMatch[1];
      if (!this.role(roomId, user.id)) {
        return json(404, { error: "not_found", message: "room not found" });
      }
      const roomMembers = this.members.get(roomId) ?? {};
      return json(200, {
        members: Object.entries(roomMembers).map(([user_id, role]) => ({ user_id, role }))
      });
    }

    return json(404, { error: "not_found", message: "not found" });
  };

  private counter = 0;
  private readonly magicLinks = new Map<string, string>();
  private readonly usedMagicLinks = new Set<string>();
  private readonly sessions = new Map<string, string>();
  private readonly usersByEmail = new Map<string, FakeUser>();
  private readonly usersById = new Map<string, FakeUser>();
  private readonly rooms = new Map<string, FakeRoom>();
  private readonly members = new Map<string, Record<string, RoomRole>>();
  private readonly invites = new Map<string, FakeInvite>();
  private readonly idempotency = new Map<string, IdempotentResponse>();

  private id(prefix: string): string {
    this.counter += 1;
    return `${prefix}_${this.counter}`;
  }

  private user(email: string): FakeUser {
    const existing = this.usersByEmail.get(email);
    if (existing) {
      return existing;
    }
    const user = { id: this.id("user"), email, display_name: "TRPG Player" };
    this.usersByEmail.set(email, user);
    this.usersById.set(user.id, user);
    return user;
  }

  private authenticatedUser(init: RequestInit | undefined): FakeUser | null {
    const authorization = headerValue(init?.headers, "authorization");
    const token = authorization?.replace(/^Bearer /, "");
    const userId = token ? this.sessions.get(token) : undefined;
    return userId ? (this.usersById.get(userId) ?? null) : null;
  }

  private role(roomId: string, userId: string): RoomRole | null {
    return this.members.get(roomId)?.[userId] ?? null;
  }

  private idempotent(scope: string, body: unknown): unknown | null {
    const existing = this.idempotency.get(scope);
    if (!existing) {
      return null;
    }
    if (existing.hash !== JSON.stringify(body)) {
      return { error: "conflict", message: "idempotency key conflict" };
    }
    return existing.body;
  }

  private rememberIdempotent(scope: string, body: unknown, response: unknown): void {
    this.idempotency.set(scope, {
      hash: JSON.stringify(body),
      body: response
    });
  }
}

function authSession(accessToken: string, user: FakeUser) {
  return {
    access_token: accessToken,
    token_type: "Bearer",
    access_token_expires_at_unix: 4_102_444_800,
    csrf_token: "csrf_test",
    user
  };
}

function roomDto(room: FakeRoom | undefined, role: RoomRole): RoomDto | null {
  if (!room) {
    return null;
  }
  return {
    id: room.id,
    title: room.title,
    system_name: room.system_name,
    privacy_mode: room.privacy_mode,
    version: room.version,
    my_role: role
  };
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" }
  });
}

function parseBody(body: BodyInit | null | undefined): unknown {
  if (typeof body !== "string") {
    return {};
  }
  return JSON.parse(body) as unknown;
}

function stringBodyField(body: unknown, key: string): string {
  if (typeof body !== "object" || body === null || Array.isArray(body)) {
    throw new Error("body must be object");
  }
  const value = (body as Record<string, unknown>)[key];
  if (typeof value !== "string") {
    throw new Error(`${key} must be string`);
  }
  return value;
}

function privacyModeField(body: unknown): RoomPrivacyMode {
  const value = stringBodyField(body, "privacy_mode");
  if (value === "standard" || value === "private_hybrid" || value === "local_only") {
    return value;
  }
  throw new Error("invalid privacy mode");
}

function invitationRoleField(body: unknown): InvitationRole {
  const value = stringBodyField(body, "role");
  if (
    value === "kp" ||
    value === "assistant_kp" ||
    value === "pl" ||
    value === "observer" ||
    value === "public_screen"
  ) {
    return value;
  }
  throw new Error("invalid invitation role");
}

function headerValue(headers: HeadersInit | undefined, name: string): string | null {
  if (!headers) {
    return null;
  }
  const lowerName = name.toLowerCase();
  if (headers instanceof Headers) {
    return headers.get(name);
  }
  if (Array.isArray(headers)) {
    return headers.find(([key]) => key.toLowerCase() === lowerName)?.[1] ?? null;
  }
  const match = Object.entries(headers).find(([key]) => key.toLowerCase() === lowerName);
  return match?.[1] ?? null;
}
