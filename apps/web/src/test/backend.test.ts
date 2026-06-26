import { describe, expect, it } from "vitest";
import {
  TrpgApiError,
  createMemorySessionStore,
  createTrpgApiClient,
  newIdempotencyKey
} from "../lib/backend";
import { FakeBackend } from "./fakeBackend";

async function login(email: string, backend = new FakeBackend()) {
  const store = createMemorySessionStore();
  const client = createTrpgApiClient({ fetcher: backend.fetch, sessionStore: store });
  const requested = await client.requestMagicLink(email, "http://localhost/auth/callback");
  const token = new URL(requested.development_magic_link ?? "").searchParams.get("token");
  if (!token) {
    throw new Error("missing token");
  }
  await client.verifyMagicLink(token);
  return { backend, client, store };
}

describe("TRPG API client", () => {
  it("runs the frontend login flow without storing refresh tokens", async () => {
    const { client, store } = await login("owner@example.test");

    expect(store.load()?.user.email).toBe("owner@example.test");
    expect(JSON.stringify(store.load())).not.toContain("refresh");
    await expect(client.me()).resolves.toMatchObject({ email: "owner@example.test" });
  });

  it("runs room creation and duplicate idempotency-key replay", async () => {
    const { client } = await login("owner@example.test");
    const key = newIdempotencyKey("create-room");
    const input = {
      title: "Friday Game",
      system_name: "generic_percentile",
      privacy_mode: "private_hybrid" as const,
      idempotency_key: key
    };

    const first = await client.createRoom(input);
    const second = await client.createRoom(input);
    const list = await client.listRooms();

    expect(second).toEqual(first);
    expect(list.rooms).toHaveLength(1);
    expect(list.rooms[0]).toMatchObject({
      title: "Friday Game",
      my_role: "owner"
    });
  });

  it("reports network errors clearly", async () => {
    const client = createTrpgApiClient({
      fetcher: async () => {
        throw new Error("offline");
      },
      sessionStore: createMemorySessionStore()
    });

    await expect(client.requestMagicLink("owner@example.test", "http://localhost")).rejects.toThrow(
      "网络错误"
    );
  });

  it("rejects room DTOs that contain KP-only fields", async () => {
    const store = createMemorySessionStore({
      accessToken: "access_test",
      accessTokenExpiresAtUnix: 4_102_444_800,
      csrfToken: "csrf",
      user: { id: "user_1", email: "owner@example.test", display_name: "Owner" }
    });
    const client = createTrpgApiClient({
      sessionStore: store,
      fetcher: async () =>
        new Response(
          JSON.stringify({
            rooms: [
              {
                id: "room_1",
                title: "Unsafe",
                system_name: "generic_percentile",
                privacy_mode: "standard",
                version: 0,
                my_role: "pl",
                kp_only_notes: "should not cross the wire"
              }
            ]
          }),
          { status: 200, headers: { "content-type": "application/json" } }
        )
    });

    await expect(client.listRooms()).rejects.toThrow("forbidden KP-only field");
  });

  it("keeps API status on authorization failures", async () => {
    const { client } = await login("owner@example.test");
    const error = await client
      .getRoom("missing-room")
      .then(() => null)
      .catch((err: unknown) => err);

    expect(error).toBeInstanceOf(TrpgApiError);
    expect((error as TrpgApiError).status).toBe(404);
  });
});
