export const roomRoles = [
  "owner",
  "kp",
  "assistant_kp",
  "pl",
  "observer",
  "public_screen"
] as const;

export const roomPrivacyModes = ["standard", "private_hybrid", "local_only"] as const;

export type RoomRole = (typeof roomRoles)[number];
export type InvitationRole = Exclude<RoomRole, "owner">;
export type RoomPrivacyMode = (typeof roomPrivacyModes)[number];

export interface UserDto {
  id: string;
  email: string;
  display_name: string;
}

export interface MagicLinkRequestResponse {
  status: "sent";
  challenge_id: string;
  expires_at_unix: number;
  development_magic_link: string | null;
}

export interface AuthSessionResponse {
  access_token: string;
  token_type: "Bearer";
  access_token_expires_at_unix: number;
  csrf_token: string;
  user: UserDto;
}

export interface RoomDto {
  id: string;
  title: string;
  system_name: string;
  privacy_mode: RoomPrivacyMode;
  version: number;
  my_role: RoomRole;
}

export interface RoomResponse {
  room: RoomDto;
}

export interface ListRoomsResponse {
  rooms: RoomDto[];
}

export interface CreateInvitationResponse {
  room_id: string;
  invited_email: string;
  role: RoomRole;
  expires_at_unix: number;
  token: string;
  invitation_url: string;
}

export interface ListRoomMembersResponse {
  members: Array<{ user_id: string; role: RoomRole }>;
}

export interface TrpgSession {
  accessToken: string;
  accessTokenExpiresAtUnix: number;
  csrfToken: string;
  user: UserDto;
}

export interface SessionStore {
  load(): TrpgSession | null;
  save(session: TrpgSession): void;
  clear(): void;
}

export type Fetcher = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;

const SESSION_KEY = "trpg.session:v1";
const CSRF_HEADER = "x-csrf-token";
const KP_ONLY_FIELDS = ["kp_only_notes", "private_kp_notes", "kp_secret", "kp_only"] as const;

export class TrpgApiError extends Error {
  constructor(
    readonly status: number,
    message: string
  ) {
    super(message);
    this.name = "TrpgApiError";
  }
}

export class TrpgApiClient {
  private readonly baseUrl: string;
  private readonly fetcher: Fetcher;
  private readonly store: SessionStore;

  constructor(options: { baseUrl?: string; fetcher?: Fetcher; sessionStore?: SessionStore } = {}) {
    this.baseUrl = (options.baseUrl ?? defaultApiBaseUrl()).replace(/\/$/, "");
    this.fetcher = options.fetcher ?? fetch.bind(globalThis);
    this.store = options.sessionStore ?? browserSessionStore();
  }

  currentSession(): TrpgSession | null {
    return this.store.load();
  }

  async requestMagicLink(
    email: string,
    redirectUri: string
  ): Promise<MagicLinkRequestResponse> {
    return this.request(
      "/api/auth/magic-link/request",
      {
        method: "POST",
        body: JSON.stringify({ email, redirect_uri: redirectUri })
      },
      parseMagicLinkRequestResponse
    );
  }

  async verifyMagicLink(token: string): Promise<AuthSessionResponse> {
    const session = await this.request(
      "/api/auth/magic-link/verify",
      {
        method: "POST",
        body: JSON.stringify({ token })
      },
      parseAuthSessionResponse
    );
    this.store.save(sessionFromResponse(session));
    return session;
  }

  async refreshSession(): Promise<AuthSessionResponse> {
    const session = this.requireSession();
    const refreshed = await this.request(
      "/api/auth/refresh",
      {
        method: "POST",
        headers: { [CSRF_HEADER]: session.csrfToken }
      },
      parseAuthSessionResponse
    );
    this.store.save(sessionFromResponse(refreshed));
    return refreshed;
  }

  async logout(): Promise<void> {
    const session = this.store.load();
    if (!session) {
      return;
    }
    try {
      await this.request(
        "/api/auth/logout",
        {
          method: "POST",
          headers: { [CSRF_HEADER]: session.csrfToken }
        },
        () => undefined
      );
    } finally {
      this.store.clear();
    }
  }

  async me(): Promise<UserDto> {
    const response = await this.authorized(
      "/api/me",
      { method: "GET" },
      (value) => parseUserDto(record(value, "me").user)
    );
    return response;
  }

  async listRooms(): Promise<ListRoomsResponse> {
    return this.authorized("/api/rooms", { method: "GET" }, parseListRoomsResponse);
  }

  async getRoom(roomId: string): Promise<RoomResponse> {
    return this.authorized(`/api/rooms/${roomId}`, { method: "GET" }, parseRoomResponse);
  }

  async createRoom(input: {
    title: string;
    system_name: string;
    privacy_mode: RoomPrivacyMode;
    idempotency_key: string;
  }): Promise<RoomResponse> {
    return this.authorized(
      "/api/rooms",
      {
        method: "POST",
        body: JSON.stringify(input)
      },
      parseRoomResponse
    );
  }

  async createInvitation(
    roomId: string,
    input: { email: string; role: InvitationRole; idempotency_key: string }
  ): Promise<CreateInvitationResponse> {
    return this.authorized(
      `/api/rooms/${roomId}/invitations`,
      {
        method: "POST",
        body: JSON.stringify(input)
      },
      parseCreateInvitationResponse
    );
  }

  async acceptInvitation(token: string, idempotencyKey: string): Promise<RoomResponse> {
    return this.authorized(
      `/api/room-invitations/${encodeURIComponent(token)}/accept`,
      {
        method: "POST",
        body: JSON.stringify({ idempotency_key: idempotencyKey })
      },
      parseRoomResponse
    );
  }

  async listMembers(roomId: string): Promise<ListRoomMembersResponse> {
    return this.authorized(
      `/api/rooms/${roomId}/members`,
      { method: "GET" },
      parseListRoomMembersResponse
    );
  }

  private async authorized<T>(
    path: string,
    init: RequestInit,
    parse: (value: unknown) => T,
    retry = true
  ): Promise<T> {
    const session = this.requireSession();
    try {
      return await this.request(
        path,
        {
          ...init,
          headers: {
            ...(headersObject(init.headers)),
            Authorization: `Bearer ${session.accessToken}`
          }
        },
        parse
      );
    } catch (error) {
      if (retry && error instanceof TrpgApiError && error.status === 401) {
        await this.refreshSession();
        return this.authorized(path, init, parse, false);
      }
      throw error;
    }
  }

  private async request<T>(
    path: string,
    init: RequestInit,
    parse: (value: unknown) => T
  ): Promise<T> {
    let response: Response;
    try {
      response = await this.fetcher(this.url(path), {
        ...init,
        credentials: "include",
        headers: {
          "content-type": "application/json",
          ...headersObject(init.headers)
        }
      });
    } catch {
      throw new TrpgApiError(0, "网络错误：无法连接 API，请稍后重试。");
    }

    const value = await readJson(response);
    if (!response.ok) {
      const body = isRecord(value) ? value : {};
      const message = typeof body.message === "string" ? body.message : response.statusText;
      throw new TrpgApiError(response.status, message);
    }
    return parse(value);
  }

  private url(path: string): string {
    return `${this.baseUrl}${path}`;
  }

  private requireSession(): TrpgSession {
    const session = this.store.load();
    if (!session) {
      throw new TrpgApiError(401, "请先登录。");
    }
    return session;
  }
}

export function createTrpgApiClient(options?: {
  baseUrl?: string;
  fetcher?: Fetcher;
  sessionStore?: SessionStore;
}): TrpgApiClient {
  return new TrpgApiClient(options);
}

export function browserSessionStore(): SessionStore {
  return {
    load() {
      if (typeof window === "undefined") {
        return null;
      }
      try {
        const raw = window.sessionStorage.getItem(SESSION_KEY);
        return raw ? parseSession(JSON.parse(raw)) : null;
      } catch {
        return null;
      }
    },
    save(session) {
      if (typeof window === "undefined") {
        return;
      }
      try {
        window.sessionStorage.setItem(SESSION_KEY, JSON.stringify(session));
      } catch {
        // Session loss is recoverable through the HttpOnly refresh cookie.
      }
    },
    clear() {
      if (typeof window === "undefined") {
        return;
      }
      try {
        window.sessionStorage.removeItem(SESSION_KEY);
      } catch {
        // Best effort.
      }
    }
  };
}

export function createMemorySessionStore(initial?: TrpgSession): SessionStore {
  let value = initial ?? null;
  return {
    load: () => value,
    save: (session) => {
      value = session;
    },
    clear: () => {
      value = null;
    }
  };
}

export function newIdempotencyKey(scope: string): string {
  const cryptoApi = globalThis.crypto;
  const id =
    cryptoApi && "randomUUID" in cryptoApi
      ? cryptoApi.randomUUID()
      : `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  return `${scope}:${id}`;
}

export function apiErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "请求失败，请稍后重试。";
}

export function sessionFromResponse(response: AuthSessionResponse): TrpgSession {
  return {
    accessToken: response.access_token,
    accessTokenExpiresAtUnix: response.access_token_expires_at_unix,
    csrfToken: response.csrf_token,
    user: response.user
  };
}

function defaultApiBaseUrl(): string {
  return process.env.NEXT_PUBLIC_API_BASE_URL ?? "";
}

function headersObject(headers: HeadersInit | undefined): Record<string, string> {
  if (!headers) {
    return {};
  }
  if (headers instanceof Headers) {
    return Object.fromEntries(headers.entries());
  }
  if (Array.isArray(headers)) {
    return Object.fromEntries(headers);
  }
  return headers;
}

async function readJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }
  try {
    return JSON.parse(text) as unknown;
  } catch {
    throw new TrpgApiError(response.status, "API 返回了无法解析的 JSON。");
  }
}

function parseMagicLinkRequestResponse(value: unknown): MagicLinkRequestResponse {
  const source = record(value, "magic link response");
  return {
    status: literal(source.status, "sent", "status"),
    challenge_id: stringField(source, "challenge_id"),
    expires_at_unix: numberField(source, "expires_at_unix"),
    development_magic_link: nullableStringField(source, "development_magic_link")
  };
}

function parseAuthSessionResponse(value: unknown): AuthSessionResponse {
  const source = record(value, "auth session");
  return {
    access_token: stringField(source, "access_token"),
    token_type: literal(source.token_type, "Bearer", "token_type"),
    access_token_expires_at_unix: numberField(source, "access_token_expires_at_unix"),
    csrf_token: stringField(source, "csrf_token"),
    user: parseUserDto(source.user)
  };
}

function parseSession(value: unknown): TrpgSession {
  const source = record(value, "stored session");
  return {
    accessToken: stringField(source, "accessToken"),
    accessTokenExpiresAtUnix: numberField(source, "accessTokenExpiresAtUnix"),
    csrfToken: stringField(source, "csrfToken"),
    user: parseUserDto(source.user)
  };
}

function parseUserDto(value: unknown): UserDto {
  const source = record(value, "user");
  return {
    id: stringField(source, "id"),
    email: stringField(source, "email"),
    display_name: stringField(source, "display_name")
  };
}

function parseListRoomsResponse(value: unknown): ListRoomsResponse {
  const source = record(value, "rooms response");
  const rooms = arrayField(source, "rooms").map(parseRoomDto);
  return { rooms };
}

function parseRoomResponse(value: unknown): RoomResponse {
  const source = record(value, "room response");
  return { room: parseRoomDto(source.room) };
}

function parseRoomDto(value: unknown): RoomDto {
  const source = record(value, "room");
  for (const field of KP_ONLY_FIELDS) {
    if (field in source) {
      throw new Error(`room DTO contains forbidden KP-only field: ${field}`);
    }
  }
  return {
    id: stringField(source, "id"),
    title: stringField(source, "title"),
    system_name: stringField(source, "system_name"),
    privacy_mode: enumField(source.privacy_mode, roomPrivacyModes, "privacy_mode"),
    version: numberField(source, "version"),
    my_role: enumField(source.my_role, roomRoles, "my_role")
  };
}

function parseCreateInvitationResponse(value: unknown): CreateInvitationResponse {
  const source = record(value, "invitation response");
  return {
    room_id: stringField(source, "room_id"),
    invited_email: stringField(source, "invited_email"),
    role: enumField(source.role, roomRoles, "role"),
    expires_at_unix: numberField(source, "expires_at_unix"),
    token: stringField(source, "token"),
    invitation_url: stringField(source, "invitation_url")
  };
}

function parseListRoomMembersResponse(value: unknown): ListRoomMembersResponse {
  const source = record(value, "members response");
  return {
    members: arrayField(source, "members").map((member) => {
      const item = record(member, "room member");
      return {
        user_id: stringField(item, "user_id"),
        role: enumField(item.role, roomRoles, "role")
      };
    })
  };
}

function record(value: unknown, name: string): Record<string, unknown> {
  if (!isRecord(value)) {
    throw new Error(`${name} 格式不正确。`);
  }
  return value;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringField(source: Record<string, unknown>, key: string): string {
  const value = source[key];
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`${key} 必须是字符串。`);
  }
  return value;
}

function nullableStringField(source: Record<string, unknown>, key: string): string | null {
  const value = source[key];
  if (value === null) {
    return null;
  }
  if (typeof value !== "string") {
    throw new Error(`${key} 必须是字符串或 null。`);
  }
  return value;
}

function numberField(source: Record<string, unknown>, key: string): number {
  const value = source[key];
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`${key} 必须是数字。`);
  }
  return value;
}

function arrayField(source: Record<string, unknown>, key: string): unknown[] {
  const value = source[key];
  if (!Array.isArray(value)) {
    throw new Error(`${key} 必须是数组。`);
  }
  return value;
}

function enumField<T extends string>(value: unknown, allowed: readonly T[], key: string): T {
  if (typeof value !== "string" || !allowed.includes(value as T)) {
    throw new Error(`${key} 不在允许范围内。`);
  }
  return value as T;
}

function literal<T extends string>(value: unknown, expected: T, key: string): T {
  if (value !== expected) {
    throw new Error(`${key} 必须是 ${expected}。`);
  }
  return expected;
}
