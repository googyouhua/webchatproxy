let ws = null;
let reconnectTimer = null;
const RECONNECT_INTERVAL = 5000;

async function getSettings() {
  const defaults = { serverUrl: "ws://localhost:9530", token: "", enabled: true };
  return await chrome.storage.local.get(defaults);
}

let connecting = false;

async function connect() {
  if (connecting) return;
  connecting = true;
  updateConnectionState("connecting", "正在连接...");
  try {
    const settings = await getSettings();
    if (!settings.enabled) { connecting = false; updateConnectionState("disconnected", "已禁用"); return; }
    if (ws && ws.readyState === WebSocket.OPEN) { connecting = false; return; }

    const url = `${settings.serverUrl}?token=${encodeURIComponent(settings.token)}`;
    ws = new WebSocket(url);

    ws.onopen = () => { connecting = false; updateConnectionState("connected", "已连接到 webai-proxy"); };

    ws.onmessage = async (event) => {
      try {
        const text = event.data instanceof Blob ? await event.data.text() : event.data;
        await handleServerMessage(JSON.parse(text));
      } catch (e) {
        console.error("[webai-proxy] parse error:", e);
      }
    };

    ws.onclose = () => {
      connecting = false; ws = null; scheduleReconnect();
      updateConnectionState("disconnected", "连接断开");
    };

    ws.onerror = () => { connecting = false; updateConnectionState("disconnected", "连接错误"); };
  } catch (e) {
    connecting = false; scheduleReconnect();
  }
}

function scheduleReconnect() {
  if (reconnectTimer) clearTimeout(reconnectTimer);
  const nextRetry = Date.now() + RECONNECT_INTERVAL;
  updateConnectionState("retrying", "连接断开，正在重试", nextRetry);
  reconnectTimer = setTimeout(connect, RECONNECT_INTERVAL);
}

const PLATFORM_URLS = {
  deepseek: { pattern: "https://chat.deepseek.com/*", openUrl: "https://chat.deepseek.com/" },
};

async function handleServerMessage(msg) {
  const { requestId, action, payload } = msg;
  wsLog("msg: " + action + " " + (payload?.platform || ""));

  try {
    const platform = payload?.platform || "deepseek";
    const platformConfig = PLATFORM_URLS[platform];
    if (!platformConfig) return sendResponse(requestId, false, null, "unsupported");

    let tab;
    if (action === "send_message" && payload?.sessionUrl) {
      const allTabs = await chrome.tabs.query({});
      tab = allTabs.find(t => t.url && t.url.startsWith(payload.sessionUrl));
      if (!tab) tab = await openChatTab(payload.sessionUrl);
    } else {
      tab = await findChatTab(platformConfig.pattern);
      if (!tab) tab = await openChatTab(platformConfig.openUrl);
    }

    if (!tab) return sendResponse(requestId, false, null, "no tab");

    await chrome.tabs.update(tab.id, { active: true });

    const csReady = await waitForContentScript(tab.id);
    if (!csReady) return sendResponse(requestId, false, null, "cs not ready");

    const response = await chrome.tabs.sendMessage(tab.id, { requestId, action, payload });
    wsLog("CS result: " + (response?.status || "unknown") + " " + (response?.response?.length || 0) + " chars");

    if (response && response.status === "done") {
      sendResponse(requestId, true, { response: response.response, sessionUrl: response.sessionUrl });
    } else if (response && response.error) {
      sendResponse(requestId, false, null, response.error);
    } else {
      sendResponse(requestId, true, response);
    }
  } catch (e) {
    wsLog("error: " + e.message);
    sendResponse(requestId, false, null, e.message);
  }
}

async function findChatTab(pattern) {
  const tabs = await chrome.tabs.query({ url: pattern });
  return tabs.length > 0 ? tabs[0] : null;
}

async function openChatTab(url) {
  const tab = await chrome.tabs.create({ url, active: true });
  await new Promise((resolve) => {
    const listener = (tabId, info) => {
      if (tabId === tab.id && info.status === "complete") {
        chrome.tabs.onUpdated.removeListener(listener);
        resolve();
      }
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(() => { chrome.tabs.onUpdated.removeListener(listener); resolve(); }, 20000);
  });
  await new Promise(r => setTimeout(r, 2000));
  await waitForContentScript(tab.id);
  return await chrome.tabs.get(tab.id);
}

async function waitForContentScriptPing(tabId, timeoutMs) {
  return Promise.race([
    chrome.tabs.sendMessage(tabId, { action: "ping" }),
    delay(timeoutMs).then(() => { throw new Error("ping timeout"); }),
  ]);
}

async function waitForContentScript(tabId) {
  for (let i = 0; i < 3; i++) {
    try { await waitForContentScriptPing(tabId, 2000); return true; } catch { await delay(1000); }
  }
  try {
    await chrome.tabs.reload(tabId);
    await delay(3000);
    for (let i = 0; i < 3; i++) {
      try { await waitForContentScriptPing(tabId, 2000); return true; } catch { await delay(1000); }
    }
  } catch {}
  return false;
}

async function sendResponse(requestId, success, data, error) {
  for (let i = 0; i < 30; i++) {
    if (ws && ws.readyState === WebSocket.OPEN) break;
    if (i === 0) connect();
    await delay(1000);
  }
  try {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ requestId, success, data, error }));
    }
  } catch {}
}

function delay(ms) {
  return new Promise(r => setTimeout(r, ms));
}

function wsLog(msg) {
  console.log("[webai-proxy]", msg);
  try {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "log", message: msg }));
    }
  } catch {}
}

// Keepalive alarm
const KEEPALIVE_ALARM = "webai-proxy-keepalive";
function startKeepAlive() { chrome.alarms.create(KEEPALIVE_ALARM, { periodInMinutes: 0.17 }); }
function stopKeepAlive() { chrome.alarms.clear(KEEPALIVE_ALARM); }

chrome.alarms.onAlarm.addListener(async (alarm) => {
  if (alarm.name === KEEPALIVE_ALARM) {
    if (!ws || ws.readyState !== WebSocket.OPEN) await connect();
  }
});

chrome.runtime.onInstalled.addListener(async () => {
  const { enabled } = await getSettings();
  if (enabled) { connect(); startKeepAlive(); }
});

chrome.runtime.onStartup.addListener(async () => {
  const { enabled } = await getSettings();
  if (enabled) { connect(); startKeepAlive(); }
});

(async () => {
  const { enabled } = await getSettings();
  if (enabled) { connect(); startKeepAlive(); }
})();

// ==================== Popup Communication & Badge ====================

function updateBadge(text, color) {
  chrome.action.setBadgeText({ text });
  if (color) {
    chrome.action.setBadgeBackgroundColor({ color });
  }
}

function updateConnectionState(state, detail, nextRetry) {
  chrome.storage.local.set({
    connectionState: { state, detail, nextRetry, updatedAt: Date.now() }
  });
  try { chrome.runtime.sendMessage({ type: "connectionState", state, detail }); } catch {}
  // Badge
  switch (state) {
    case "connected": updateBadge("ON", "#4CAF50"); break;
    case "connecting": updateBadge("...", "#FF9800"); break;
    case "retrying": updateBadge("...", "#FF9800"); break;
    default: updateBadge("", "");
  }
}

chrome.runtime.onMessage.addListener((msg, sender, sendResponse) => {
  if (msg.type === "getStatus") {
    chrome.storage.local.get("connectionState", (data) => {
      sendResponse(data.connectionState || { state: "disconnected", detail: "未连接" });
    });
    return true;
  }
  if (msg.type === "reconnect") {
    connect();
    sendResponse({ success: true });
  }
  if (msg.type === "disconnect") {
    if (reconnectTimer) clearTimeout(reconnectTimer);
    reconnectTimer = null;
    connecting = false;
    if (ws) { ws.onclose = null; ws.close(); ws = null; }
    updateConnectionState("disconnected", "已断开连接");
    sendResponse({ success: true });
  }
});
