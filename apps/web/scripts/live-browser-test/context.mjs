import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

export const origin = requiredEnvironment("AR11_LIVE_ORIGIN");
export const credentials = parseEnvironmentFile(
  await readFile(requiredEnvironment("AR11_CREDENTIALS_FILE"), "utf8"),
);
export const tutorial = parseEnvironmentFile(
  await readFile(requiredEnvironment("AR11_TUTORIAL_FILE"), "utf8"),
);
export const privateCanary = requiredEnvironment("AR11_PRIVATE_CANARY");
export const runId = process.env.AR11_BROWSER_RUN_ID || "primary";
export const evidenceRoot = process.env.AR11_EVIDENCE_DIRECTORY
  ? path.resolve(process.env.AR11_EVIDENCE_DIRECTORY)
  : await mkdtemp(path.join(tmpdir(), "ar11-live-browser-evidence-"));
await mkdir(evidenceRoot, { recursive: true, mode: 0o700 });

export const humanCampaignId = tutorial.TUTORIAL_CAMPAIGN_ID;
export const aiCampaignId = `${humanCampaignId}_ai_${runId}`;
export const humanSessionId = `session_${humanCampaignId}_${runId}`;
export const characterId = `investigator_${humanCampaignId}_${runId}`;
export const accounts = {
  owner: {
    userId: credentials.ADMIN_USER_ID,
    login: credentials.ADMIN_LOGIN,
    password: credentials.ADMIN_PASSWORD,
  },
  keeper: {
    userId: credentials.BUSINESS_USER_ID,
    login: credentials.BUSINESS_LOGIN,
    password: credentials.BUSINESS_PASSWORD,
  },
  playerA: generatedAccount("ar11_player_a"),
  playerB: generatedAccount("ar11_player_b"),
  spectator: generatedAccount("ar11_spectator"),
};

export const result = {
  browser: "Google Chrome headless via DevTools Protocol",
  composition: "production Docker Compose APIs, policy, event store, realtime, and agent worker",
  url: origin,
  viewports: ["1600x1000", "390x844"],
  screenshots: [],
  exports: [],
  checks: [],
};


function generatedAccount(userId) {
  return {
    userId,
    login: `${userId.replaceAll("_", "-")}@example.invalid`,
    password: createHash("sha256")
      .update(`ar11-browser:${humanCampaignId}:${userId}`)
      .digest("hex"),
  };
}

function parseEnvironmentFile(contents) {
  return Object.fromEntries(contents.trim().split("\n").map((line) => {
    const separator = line.indexOf("=");
    if (separator <= 0) throw new Error("invalid credentials file");
    const key = line.slice(0, separator);
    let value = line.slice(separator + 1);
    if ((value.startsWith("'") && value.endsWith("'"))
      || (value.startsWith('"') && value.endsWith('"'))) {
      value = value.slice(1, -1);
    }
    return [key, value];
  }));
}

function requiredEnvironment(name) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is required`);
  return value;
}


export let chrome;

export function setChrome(value) {
  chrome = value;
}
