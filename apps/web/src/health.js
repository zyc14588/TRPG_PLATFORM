export function validateHealthPayload(payload, kind, processName) {
  if (!payload || typeof payload !== "object" || Array.isArray(payload)) return false;
  if (payload.service !== processName || typeof payload.version !== "string" || !payload.version.trim()) return false;
  if (!Array.isArray(payload.checks) || payload.checks.length === 0) return false;

  const checksAreValid = payload.checks.every((check) => (
    check
      && typeof check === "object"
      && typeof check.name === "string"
      && check.name.length > 0
      && (check.status === "pass" || check.status === "fail")
      && typeof check.detail === "string"
  ));
  if (!checksAreValid) return false;

  if (kind === "live") {
    return payload.status === "live"
      && payload.checks.some((check) => check.name === "listener" && check.status === "pass");
  }
  if (kind === "ready") {
    return payload.status === "ready" && payload.checks.every((check) => check.status === "pass");
  }
  return false;
}
