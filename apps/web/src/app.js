import { ProductApi } from "/src/api.js";
import { RealtimeClient } from "/src/realtime.js";
import { root, setApi, setRealtime, state } from "/src/app/context.js";
import { appendEvent } from "/src/app/governance-operations.js";
import { handleAction, handleForm } from "/src/app/handlers.js";
import { messageFor } from "/src/app/support.js";
import { render } from "/src/app/views.js";
import { fatalView, updateConnectionStatus } from "/src/app/view-support.js";

start().catch((error) => {
  state.error = messageFor(error);
  root.innerHTML = fatalView();
  root.setAttribute("aria-busy", "false");
});

root.addEventListener("click", (event) => {
  const target = event.target.closest("[data-action]");
  if (!target || state.busy) return;
  void handleAction(target.dataset.action, target.dataset);
});

root.addEventListener("submit", (event) => {
  const form = event.target.closest("form[data-form]");
  if (!form) return;
  event.preventDefault();
  if (!state.busy) void handleForm(form);
});

async function start() {
  const response = await fetch("/config.json", { cache: "no-store" });
  if (!response.ok) throw new Error(`CONFIGURATION_${response.status}`);
  state.config = await response.json();
  setApi(new ProductApi(state.config));
  setRealtime(new RealtimeClient({
    baseUrl: state.config.realtimeBase,
    onStatus: (status) => {
      state.realtime = { ...state.realtime, ...status };
      if (["forbidden", "unauthenticated", "resync", "error"].includes(status.state)) {
        render();
      } else {
        updateConnectionStatus();
      }
    },
    onEvent: (event) => {
      appendEvent(event);
      render();
    },
  }));
  render();
}
