import assert from "node:assert/strict";
import test from "node:test";
import { validateHealthPayload } from "../src/health.js";

const live = {
  service: "api-server",
  version: "0.1.0",
  status: "live",
  checks: [{ name: "listener", status: "pass", detail: "request accepted" }],
};

test("accepts the matching live process contract", () => {
  assert.equal(validateHealthPayload(live, "live", "api-server"), true);
});

test("rejects fake 200 payloads and mismatched processes", () => {
  assert.equal(validateHealthPayload("OK", "live", "api-server"), false);
  assert.equal(validateHealthPayload({ ...live, service: "realtime-server" }, "live", "api-server"), false);
  assert.equal(validateHealthPayload({ ...live, checks: [] }, "live", "api-server"), false);
});

test("requires every readiness check to pass", () => {
  const ready = {
    service: "api-server",
    version: "0.1.0",
    status: "ready",
    checks: [{ name: "config", status: "pass", detail: "loaded" }],
  };
  assert.equal(validateHealthPayload(ready, "ready", "api-server"), true);
  assert.equal(
    validateHealthPayload({ ...ready, checks: [{ ...ready.checks[0], status: "fail" }] }, "ready", "api-server"),
    false,
  );
  assert.equal(validateHealthPayload({ ...ready, status: "not_ready" }, "ready", "api-server"), false);
});
