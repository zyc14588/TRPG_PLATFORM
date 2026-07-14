import assert from "node:assert/strict";
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import path from "node:path";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const config = JSON.parse(await readFile(path.join(root, "dist/config.json"), "utf8"));
const { inspectServiceHealth } = await import(
  new URL("../dist/src/health.js", import.meta.url)
);

assert.equal(config.services.length, 5);
assert.equal(new Set(config.services.map(({ url }) => url)).size, 5);

const services = await Promise.all(
  config.services.map(({ name }, index) => startService(name, `0.1.${index}`, true)),
);
try {
  const health = await Promise.all(
    services.map(({ name, url }) => inspectServiceHealth({ name, url })),
  );
  assert.equal(health.filter(({ healthy }) => healthy).length, 5);
  assert.deepEqual(
    health.map(({ state }) => state),
    ["ready", "ready", "ready", "ready", "ready"],
  );

  services[0].ready = false;
  assert.deepEqual(await inspectServiceHealth(services[0]), {
    healthy: false,
    state: "degraded",
    version: "0.1.0",
  });
} finally {
  await Promise.all(services.map(({ server }) => close(server)));
}

assert.deepEqual(await inspectServiceHealth(services[0]), {
  healthy: false,
  state: "unavailable",
  version: "-",
});

console.log("web shell behavior: 5 ready, degraded, and unavailable paths passed");

async function startService(name, version, ready) {
  const service = { name, version, ready, server: undefined, url: undefined };
  service.server = createServer((request, response) => {
    const isLive = request.url === "/health/live";
    const isReady = request.url === "/health/ready";
    if (!isLive && !isReady) {
      response.writeHead(404).end();
      return;
    }
    const healthy = isLive || service.ready;
    response.writeHead(healthy ? 200 : 503, { "Content-Type": "application/json" });
    response.end(
      JSON.stringify({
        service: name,
        version,
        status: isLive ? "live" : service.ready ? "ready" : "not_ready",
      }),
    );
  });
  await new Promise((resolve, reject) => {
    service.server.once("error", reject);
    service.server.listen(0, "127.0.0.1", resolve);
  });
  const address = service.server.address();
  service.url = `http://127.0.0.1:${address.port}`;
  return service;
}

function close(server) {
  return new Promise((resolve, reject) => {
    server.close((error) => (error ? reject(error) : resolve()));
  });
}
