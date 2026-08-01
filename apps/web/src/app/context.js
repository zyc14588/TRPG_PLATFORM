export const root = document.querySelector("#app");
export const announcer = document.querySelector("#announcer");

export const state = {
  config: null,
  screen: "login",
  workspaceTab: "play",
  session: null,
  campaigns: [],
  campaign: null,
  authority: null,
  membership: null,
  events: [],
  realtime: { state: "disconnected", cursor: 0 },
  versions: new Map(),
  recent: {},
  invite: null,
  export: null,
  adminEvidence: null,
  adminReady: false,
  adminVersion: null,
  error: "",
  notice: "",
  busy: false,
};

export let api;
export let realtime;

export function setApi(value) {
  api = value;
}

export function setRealtime(value) {
  realtime = value;
}
