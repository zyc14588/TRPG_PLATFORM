import { spawn } from "node:child_process";
import { writeFile } from "node:fs/promises";
import path from "node:path";

export class BrowserPage {
  static async open(debugOrigin, url) {
    const target = await fetch(`${debugOrigin}/json/new?${encodeURIComponent(url)}`, { method: "PUT" }).then((response) => response.json());
    const page = new BrowserPage(target.webSocketDebuggerUrl);
    await page.ready;
    await Promise.all([
      page.send("Page.enable"),
      page.send("Runtime.enable"),
      page.send("Network.enable"),
      page.send("Log.enable"),
    ]);
    page.on("Runtime.exceptionThrown", (event) => page.errors.push(event.exceptionDetails?.text || "runtime exception"));
    page.on("Log.entryAdded", (event) => {
      if (["error", "warning"].includes(event.entry.level)) page.errors.push(event.entry.text);
    });
    page.on("Network.responseReceived", (event) => page.responses.set(event.requestId, event.response.url));
    page.on("Network.requestWillBeSent", (event) => {
      page.requests.push(`${event.request.method} ${new URL(event.request.url).pathname}`);
    });
    page.on("Network.loadingFailed", (event) => page.networkFailures.push(`${event.errorText}:${event.blockedReason || ""}`));
    page.on("Network.loadingFinished", async (event) => {
      if (!page.responses.has(event.requestId)) return;
      try {
        const body = await page.send("Network.getResponseBody", { requestId: event.requestId });
        page.responseBodies.push(body.body || "");
      } catch {}
    });
    page.on("Network.webSocketFrameReceived", (event) => page.websocketFrames.push(event.response.payloadData || ""));
    await page.waitForText("进入调查台");
    return page;
  }

  constructor(websocketUrl) {
    this.socket = new WebSocket(websocketUrl);
    this.pending = new Map();
    this.listeners = new Map();
    this.nextId = 0;
    this.errors = [];
    this.requests = [];
    this.responses = new Map();
    this.responseBodies = [];
    this.networkFailures = [];
    this.websocketFrames = [];
    this.ready = new Promise((resolve, reject) => {
      this.socket.addEventListener("open", resolve, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (message.id) {
        const pending = this.pending.get(message.id);
        this.pending.delete(message.id);
        if (message.error) pending?.reject(new Error(message.error.message));
        else pending?.resolve(message.result || {});
      } else {
        for (const listener of this.listeners.get(message.method) || []) listener(message.params || {});
      }
    });
  }

  send(method, params = {}) {
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  on(method, listener) {
    const listeners = this.listeners.get(method) || [];
    listeners.push(listener);
    this.listeners.set(method, listeners);
  }

  async evaluate(expression) {
    const result = await this.send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails) {
      throw new Error(
        result.exceptionDetails.exception?.description || result.exceptionDetails.text,
      );
    }
    return result.result?.value;
  }

  text() {
    return this.evaluate("document.body.innerText");
  }

  async hasText(text) {
    return (await this.text()).includes(text);
  }

  async waitForText(text, timeout = 8_000) {
    await waitUntil(() => this.hasText(text), timeout, `text not found: ${text}`);
  }

  click(selector) {
    return this.evaluate(`(() => { const element = document.querySelector(${JSON.stringify(selector)}); if (!element) throw new Error('missing selector'); element.click(); })()`);
  }

  submit(selector, values = {}) {
    return this.evaluate(`(() => {
      const form = document.querySelector(${JSON.stringify(selector)});
      if (!form) throw new Error('missing form ${selector}');
      const values = ${JSON.stringify(values)};
      for (const [name, value] of Object.entries(values)) {
        const field = form.elements.namedItem(name);
        if (!field) throw new Error('missing field ' + name);
        field.value = value;
        field.dispatchEvent(new Event('input', { bubbles: true }));
        field.dispatchEvent(new Event('change', { bubbles: true }));
      }
      form.requestSubmit();
    })()`);
  }

  setViewport(width, height) {
    return this.send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: width < 600 });
  }

  async navigate(url) {
    const loaded = new Promise((resolve) => {
      const handler = () => resolve();
      this.on("Page.loadEventFired", handler);
    });
    await this.send("Page.navigate", { url });
    await Promise.race([loaded, timeout(8_000, "navigation timeout")]);
  }

  async screenshot(destination) {
    const result = await this.send("Page.captureScreenshot", { format: "png", fromSurface: true });
    await writeFile(destination, Buffer.from(result.data, "base64"));
  }

  async pressTab() {
    await this.send("Input.dispatchKeyEvent", { type: "keyDown", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
    await this.send("Input.dispatchKeyEvent", { type: "keyUp", key: "Tab", code: "Tab", windowsVirtualKeyCode: 9 });
  }
}

export async function launchChrome(outputRoot) {
  const profile = path.join(outputRoot, "chrome-profile");
  const executable = process.env.CHROME_BIN || "/usr/bin/google-chrome";
  const certificatePin = process.env.AR11_CERTIFICATE_SPKI;
  const child = spawn(executable, [
    "--headless=new",
    "--disable-gpu",
    "--disable-dev-shm-usage",
    "--no-first-run",
    "--no-default-browser-check",
    ...(certificatePin ? [`--ignore-certificate-errors-spki-list=${certificatePin}`] : []),
    `--user-data-dir=${profile}`,
    "--remote-debugging-port=0",
    "about:blank",
  ], { stdio: ["ignore", "ignore", "pipe"] });
  let stderr = "";
  const debugUrl = await Promise.race([
    new Promise((resolve, reject) => {
      child.stderr.setEncoding("utf8");
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
        const match = stderr.match(/DevTools listening on (ws:\/\/[^\s]+)/);
        if (match) resolve(match[1]);
      });
      child.once("exit", (code) => reject(new Error(`Chrome exited ${code}: ${stderr}`)));
    }),
    timeout(10_000, "Chrome DevTools endpoint timeout"),
  ]);
  const parsed = new URL(debugUrl);
  return {
    child,
    debugOrigin: `http://${parsed.host}`,
    async close() {
      child.kill("SIGTERM");
      await Promise.race([new Promise((resolve) => child.once("exit", resolve)), new Promise((resolve) => setTimeout(resolve, 2_000))]);
      if (child.exitCode === null) {
        child.kill("SIGKILL");
        await Promise.race([new Promise((resolve) => child.once("exit", resolve)), new Promise((resolve) => setTimeout(resolve, 2_000))]);
      }
    },
  };
}

export async function waitUntil(check, waitMs = 8_000, message = "condition timeout") {
  const deadline = Date.now() + waitMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      if (await check()) return;
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 80));
  }
  throw lastError || new Error(message);
}

function timeout(waitMs, message) {
  return new Promise((_, reject) => setTimeout(() => reject(new Error(message)), waitMs));
}
