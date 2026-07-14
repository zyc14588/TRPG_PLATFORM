export async function inspectServiceHealth(service, fetchImplementation = fetch) {
  try {
    const [liveResponse, readyResponse] = await Promise.all([
      fetchImplementation(`${service.url}/health/live`, { cache: "no-store" }),
      fetchImplementation(`${service.url}/health/ready`, { cache: "no-store" }),
    ]);
    const live = await liveResponse.json();
    const ready = await readyResponse.json();
    const healthy =
      liveResponse.ok &&
      readyResponse.ok &&
      live.status === "live" &&
      ready.status === "ready" &&
      live.service === ready.service;
    return {
      healthy,
      state: healthy ? "ready" : "degraded",
      version: ready.version ?? live.version ?? "-",
    };
  } catch {
    return { healthy: false, state: "unavailable", version: "-" };
  }
}
