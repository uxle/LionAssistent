// AI Assistant Remote -- mobile companion client (Development_plan
// Phase 28). Deliberately plain JavaScript, no build step, no bundler,
// no framework: this file *is* what runs in the browser, unmodified.
// See lib.rs for why this is a PWA rather than a native Android app.

const TOKEN_KEY = "ai-assistant-remote-token";
const LABEL_KEY = "ai-assistant-remote-label";

const pairView = document.getElementById("pair-view");
const mainView = document.getElementById("main-view");
const connectionDot = document.getElementById("connection-dot");

function storedToken() {
  return localStorage.getItem(TOKEN_KEY);
}

function apiFetch(path, options = {}) {
  const token = storedToken();
  const headers = Object.assign({}, options.headers || {});
  if (token) headers["Authorization"] = "Bearer " + token;
  return fetch(path, Object.assign({}, options, { headers }));
}

// --- Pairing ---

document.getElementById("pair-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const label = document.getElementById("device-label").value.trim();
  const code = document.getElementById("pair-code").value.trim();
  const errorEl = document.getElementById("pair-error");
  errorEl.textContent = "";

  try {
    const res = await fetch("/pair/complete", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code: code, device_label: label }),
    });
    const body = await res.json();
    if (!res.ok) {
      errorEl.textContent = body.error || "Pairing failed.";
      return;
    }
    localStorage.setItem(TOKEN_KEY, body.token);
    localStorage.setItem(LABEL_KEY, label);
    enterMainView();
  } catch (err) {
    errorEl.textContent = "Could not reach the assistant: " + err;
  }
});

document.getElementById("forget-device").addEventListener("click", () => {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(LABEL_KEY);
  location.reload();
});

// --- Main view ---

async function enterMainView() {
  pairView.hidden = true;
  mainView.hidden = false;
  document.getElementById("paired-label").textContent = localStorage.getItem(LABEL_KEY) || "this device";

  await loadStatus();
  await loadTools();
  connectNotifications();
}

async function loadStatus() {
  const res = await apiFetch("/status");
  if (res.status === 401) return signOutAndShowPairing();
  const body = await res.json();
  document.getElementById("paired-label").textContent = body.paired_as;
  setConnected(true);
}

async function loadTools() {
  const res = await apiFetch("/tools");
  if (res.status === 401) return signOutAndShowPairing();
  const body = await res.json();

  const listEl = document.getElementById("tools-list");
  const selectEl = document.getElementById("tool-select");
  listEl.innerHTML = "";
  selectEl.innerHTML = "";

  for (const tool of body.tools) {
    const item = document.createElement("div");
    item.className = "tool-item";
    item.innerHTML = `<strong>${escapeHtml(tool.name)}</strong>
      <span class="perm perm-${escapeHtml(tool.permission_level)}">${escapeHtml(tool.permission_level)}</span>
      <p>${escapeHtml(tool.description)}</p>`;
    listEl.appendChild(item);

    const option = document.createElement("option");
    option.value = tool.name;
    option.textContent = tool.name;
    selectEl.appendChild(option);
  }
}

document.getElementById("execute-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const tool = document.getElementById("tool-select").value;
  const resultEl = document.getElementById("execute-result");
  let args;
  try {
    args = JSON.parse(document.getElementById("tool-args").value || "{}");
  } catch (err) {
    resultEl.textContent = "Arguments must be valid JSON: " + err;
    return;
  }

  resultEl.textContent = "Running...";
  try {
    const res = await apiFetch("/tools/execute", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ tool: tool, args: args }),
    });
    if (res.status === 401) return signOutAndShowPairing();
    const body = await res.json();
    resultEl.textContent = JSON.stringify(body, null, 2);
  } catch (err) {
    resultEl.textContent = "Request failed: " + err;
  }
});

// --- Live notifications (SSE) ---

let eventSource = null;

function connectNotifications() {
  const token = storedToken();
  if (!token) return;
  // EventSource cannot set an Authorization header directly, so the
  // token travels as a query parameter for this one connection only.
  // It is the same bearer token already stored in localStorage (i.e.
  // no new secret is created), and this endpoint is otherwise
  // unreachable without it -- this is a documented, deliberate
  // narrowing for the one browser API that has no header hook, not an
  // auth bypass.
  eventSource = new EventSource("/events?token=" + encodeURIComponent(token));
  eventSource.onopen = () => setConnected(true);
  eventSource.onerror = () => setConnected(false);
  eventSource.onmessage = (event) => {
    const data = JSON.parse(event.data);
    addNotification(data);
  };
}

function addNotification(data) {
  const list = document.getElementById("notifications");
  const item = document.createElement("li");
  const time = new Date().toLocaleTimeString();
  item.textContent = `[${time}] ${data.type}` + (data.tool ? `: ${data.tool}` : "");
  list.prepend(item);
  while (list.children.length > 50) list.removeChild(list.lastChild);
}

function setConnected(isConnected) {
  connectionDot.classList.toggle("connected", isConnected);
}

function signOutAndShowPairing() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(LABEL_KEY);
  if (eventSource) eventSource.close();
  mainView.hidden = true;
  pairView.hidden = false;
}

function escapeHtml(s) {
  const div = document.createElement("div");
  div.textContent = String(s);
  return div.innerHTML;
}

// --- Boot ---

if (storedToken()) {
  enterMainView();
} else {
  pairView.hidden = false;
}
