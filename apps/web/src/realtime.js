export const REALTIME_PROTOCOL = "trpg.realtime.v1";
const AUTH_PROTOCOL_PREFIX = "trpg.auth.";

export class RealtimeClient {
  constructor({ baseUrl, WebSocketClass = globalThis.WebSocket, onStatus, onEvent }) {
    this.baseUrl = baseUrl;
    this.WebSocketClass = WebSocketClass;
    this.onStatus = onStatus;
    this.onEvent = onEvent;
    this.socket = null;
    this.reconnectTimer = null;
    this.reconnectAttempt = 0;
    this.cursor = 0;
    this.resumeToken = null;
    this.resumeCursor = 0;
    this.manualClose = false;
    this.binding = null;
  }

  connect({ token, campaignId, roomId = campaignId, kind = "campaign" }) {
    this.disconnect(true);
    if (!/^[A-Za-z0-9._~-]{1,2048}$/.test(token)) {
      throw new Error("REALTIME_AUTH_TOKEN_INVALID");
    }
    this.connection = { token, campaignId, roomId, kind };
    this.manualClose = false;
    this.#open();
  }

  reconnect() {
    if (!this.connection) return;
    clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    const previous = this.socket;
    this.socket = null;
    if (previous && previous.readyState < 2) previous.close(1000, "client_reconnect");
    this.manualClose = false;
    this.#open();
  }

  disconnect(clearResume = true) {
    this.manualClose = true;
    clearTimeout(this.reconnectTimer);
    this.reconnectTimer = null;
    if (this.socket && this.socket.readyState < 2) this.socket.close(1000, "client_navigation");
    this.socket = null;
    if (clearResume) {
      this.cursor = 0;
      this.resumeToken = null;
      this.resumeCursor = 0;
      this.connection = null;
    }
  }

  #open() {
    clearTimeout(this.reconnectTimer);
    if (!this.connection) return;
    const { token, campaignId, roomId, kind } = this.connection;
    const url = websocketUrl(this.baseUrl, campaignId, roomId);
    this.onStatus?.({ state: this.reconnectAttempt ? "reconnecting" : "connecting" });
    const socket = new this.WebSocketClass(url, [
      REALTIME_PROTOCOL,
      `${AUTH_PROTOCOL_PREFIX}${token}`,
    ]);
    this.socket = socket;
    socket.addEventListener("open", () => {
      this.reconnectAttempt = 0;
      socket.send(
        JSON.stringify({
          version: REALTIME_PROTOCOL,
          request_id: requestId("subscribe"),
          type: "subscribe",
          subscription: { kind, room_id: roomId },
          cursor: this.resumeToken ? this.resumeCursor : 0,
          resume_token: this.resumeToken,
        }),
      );
    });
    socket.addEventListener("message", (message) => this.#receive(message.data));
    socket.addEventListener("error", () => this.onStatus?.({ state: "unavailable" }));
    socket.addEventListener("close", (event) => this.#closed(socket, event));
  }

  #receive(raw) {
    let envelope;
    try {
      envelope = JSON.parse(raw);
    } catch {
      this.onStatus?.({ state: "error", code: "REALTIME_ENVELOPE_MALFORMED" });
      this.socket?.close(1002, "malformed_envelope");
      return;
    }
    if (envelope.version !== REALTIME_PROTOCOL) {
      this.onStatus?.({ state: "error", code: "REALTIME_PROTOCOL_VERSION_UNSUPPORTED" });
      return;
    }
    if (envelope.type === "connected") {
      this.binding = envelope.binding;
      this.onStatus?.({ state: "connected", binding: envelope.binding });
    } else if (envelope.type === "subscribed") {
      this.cursor = Number(envelope.cursor) || 0;
      this.resumeToken = envelope.resume_token || null;
      this.resumeCursor = this.resumeToken ? this.cursor : 0;
      this.onStatus?.({ state: "synced", cursor: this.cursor });
    } else if (envelope.type === "event") {
      this.cursor = Math.max(this.cursor, Number(envelope.cursor) || 0);
      this.onEvent?.(envelope.event);
      this.socket?.send(
        JSON.stringify({
          version: REALTIME_PROTOCOL,
          request_id: requestId("ack"),
          type: "ack",
          cursor: this.cursor,
        }),
      );
    } else if (envelope.type === "checkpoint" || envelope.type === "acked") {
      const checkpointCursor = Number(envelope.cursor) || 0;
      this.cursor = Math.max(this.cursor, checkpointCursor);
      if (envelope.resume_token && checkpointCursor >= this.resumeCursor) {
        this.resumeToken = envelope.resume_token;
        this.resumeCursor = checkpointCursor;
      }
      this.onStatus?.({ state: "synced", cursor: this.cursor });
    } else if (envelope.type === "heartbeat") {
      this.socket?.send(
        JSON.stringify({
          version: REALTIME_PROTOCOL,
          request_id: requestId("pong"),
          type: "pong",
          nonce: envelope.nonce,
        }),
      );
    } else if (envelope.type === "resync_required") {
      this.cursor = Math.max(0, Number(envelope.earliest_cursor) - 1);
      this.resumeToken = null;
      this.resumeCursor = 0;
      this.onStatus?.({ state: "resync", code: envelope.reason });
      this.socket?.close(4009, "resync_required");
    } else if (envelope.type === "error") {
      this.onStatus?.({ state: "error", code: envelope.code });
    }
  }

  #closed(socket, event) {
    if (this.socket !== socket) return;
    this.socket = null;
    if (this.manualClose) {
      this.onStatus?.({ state: "disconnected" });
      return;
    }
    if ([4001, 4003, 4011].includes(event.code)) {
      const state = event.code === 4001 ? "unauthenticated" : "forbidden";
      this.onStatus?.({ state, code: event.code });
      return;
    }
    this.reconnectAttempt += 1;
    const delay = Math.min(8_000, 500 * 2 ** Math.min(this.reconnectAttempt, 4));
    this.onStatus?.({ state: "reconnecting", delay });
    this.reconnectTimer = setTimeout(() => this.#open(), delay);
  }
}

export function websocketUrl(baseUrl, campaignId, roomId) {
  if (
    typeof baseUrl !== "string"
    || !baseUrl.startsWith("/")
    || baseUrl.startsWith("//")
    || baseUrl.includes("\\")
  ) {
    throw new Error("REALTIME_BASE_NOT_SAME_ORIGIN");
  }
  const base = new URL(baseUrl, globalThis.location?.href || "http://localhost/");
  base.protocol = base.protocol === "https:" ? "wss:" : "ws:";
  base.pathname = `${base.pathname.replace(/\/$/, "")}/ws/v1/campaigns/${encodeURIComponent(
    campaignId,
  )}/rooms/${encodeURIComponent(roomId)}`;
  base.search = "";
  base.hash = "";
  return base.toString();
}

function requestId(prefix) {
  return `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}
