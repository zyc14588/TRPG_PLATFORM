import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import net from "node:net";
import path from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { createMemorySessionStore, createTrpgApiClient, newIdempotencyKey } from "../../lib/backend";

let server: ReturnType<typeof spawn> | null = null;
let baseUrl = "";
let serverOutput = "";

async function login(email: string) {
  const client = createTrpgApiClient({
    baseUrl,
    sessionStore: createMemorySessionStore()
  });
  const request = await client.requestMagicLink(email, "http://localhost/auth/callback");
  const token = new URL(request.development_magic_link ?? "").searchParams.get("token");
  if (!token) {
    throw new Error("missing token");
  }
  await client.verifyMagicLink(token);
  return client;
}

describe("frontend auth and room E2E flow", () => {
  beforeAll(async () => {
    const port = await freePort();
    baseUrl = `http://127.0.0.1:${port}`;
    const env = { ...process.env };
    delete env.DATABASE_URL;
    Object.assign(env, {
      TRPG_AUTH_MODE: "development",
      TRPG_AUTH_SECRET: "test-secret",
      TRPG_BIND_ADDR: `127.0.0.1:${port}`
    });

    const child = spawn("cargo", ["run", "-p", "server"], {
      cwd: repoRoot(),
      env,
      stdio: ["ignore", "pipe", "pipe"]
    });
    server = child;
    child.stdout?.on("data", (chunk) => {
      serverOutput += chunk.toString();
    });
    child.stderr?.on("data", (chunk) => {
      serverOutput += chunk.toString();
    });
    await waitForHealth();
  }, 60000);

  afterAll(() => {
    if (!server?.pid) {
      return;
    }
    if (process.platform === "win32") {
      spawnSync("taskkill", ["/pid", String(server.pid), "/T", "/F"]);
    } else {
      server.kill("SIGTERM");
    }
  });

  it("lets two users create, invite, join, and blocks a third party", async () => {
    const suffix = newIdempotencyKey("e2e").replace(/[^a-z0-9-]/gi, "-");
    const owner = await login(`owner-${suffix}@example.test`);
    const player = await login(`player-${suffix}@example.test`);
    const outsider = await login(`outsider-${suffix}@example.test`);

    const created = await owner.createRoom({
      title: "Invite Room",
      system_name: "generic_percentile",
      privacy_mode: "private_hybrid",
      idempotency_key: newIdempotencyKey("create-room")
    });
    const invite = await owner.createInvitation(created.room.id, {
      email: `player-${suffix}@example.test`,
      role: "pl",
      idempotency_key: newIdempotencyKey("invite")
    });
    const joined = await player.acceptInvitation(invite.token, newIdempotencyKey("accept"));
    const playerView = await player.getRoom(created.room.id);
    const members = await owner.listMembers(created.room.id);

    expect(joined.room).toMatchObject({ id: created.room.id, my_role: "pl" });
    expect(playerView.room).toMatchObject({ title: "Invite Room", my_role: "pl" });
    expect(members.members.map((member) => member.role).sort()).toEqual(["owner", "pl"]);
    await expect(outsider.getRoom(created.room.id)).rejects.toMatchObject({ status: 404 });
  }, 60000);
});

async function waitForHealth() {
  const started = Date.now();
  while (Date.now() - started < 55000) {
    if (server?.exitCode !== null) {
      throw new Error(`server exited before ready\n${serverOutput}`);
    }
    try {
      const response = await fetch(`${baseUrl}/healthz`);
      if (response.ok) {
        return;
      }
    } catch {
      // keep polling while cargo builds and the server starts
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`server did not become ready\n${serverOutput}`);
}

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = net.createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      probe.close(() => {
        if (address && typeof address === "object") {
          resolve(address.port);
        } else {
          reject(new Error("no free port"));
        }
      });
    });
  });
}

function repoRoot() {
  let dir = process.cwd();
  while (!existsSync(path.join(dir, "Cargo.toml"))) {
    const parent = path.dirname(dir);
    if (parent === dir) {
      throw new Error("repo root not found");
    }
    dir = parent;
  }
  return dir;
}
