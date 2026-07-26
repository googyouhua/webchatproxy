// ============================================
// background.js
// Chrome 插件 Service Worker
// 职责：管理 WebSocket 连接，在 MCP Server 和 Content Script 之间路由消息
// ============================================

let ws = null;
let reconnectTimer = null;
const RECONNECT_INTERVAL = 5000;

// ---------- 设置管理 ----------

async function getSettings() {
  const defaults = {
    serverUrl: "ws://localhost:9527",
    token: "",
    enabled: true,
  };
  const result = await chrome.storage.local.get(defaults);
  return result;
}

// ---------- WebSocket 连接 ----------

let connectAttempts = 0;
let isManualDisconnect = false;

function updateConnectionState(state, detail, nextRetry) {
  chrome.storage.local.set({ connectionState: { state, detail, time: Date.now(), nextRetry: nextRetry || null } });
}

let connecting = false;

async function connect() {
  if (connecting) return;
  connecting = true;
  try {
    const settings = await getSettings();

    if (!settings.enabled) {
      updateConnectionState("disabled", "连接未启用");
      return;
    }

    if (ws && ws.readyState === WebSocket.OPEN) {
      return;
    }

    connectAttempts++;
    const url = `${settings.serverUrl}?token=${encodeURIComponent(settings.token)}`;
    updateBadge("...", "#FF9800");
    updateConnectionState("connecting", `第 ${connectAttempts} 次尝试连接...`);

    try {
      ws = new WebSocket(url);

      ws.onopen = () => {
        connectAttempts = 0;
        updateBadge("ON", "#4CAF50");
        updateConnectionState("connected", "已连接到 MCP Server");
      };

      ws.onmessage = async (event) => {
        try {
          const text = event.data instanceof Blob ? await event.data.text() : event.data;
          const msg = JSON.parse(text);
          await handleServerMessage(msg);
        } catch (e) {
          console.error("[ai-bridge] 解析消息失败:", e);
        }
      };

      ws.onclose = (event) => {
        ws = null;
        if (isManualDisconnect) {
          isManualDisconnect = false;
          return;
        }
        updateBadge("...", "#FF9800");
        const nextRetry = Date.now() + 10000;
        updateConnectionState("retrying", `连接断开，等待重连...`, nextRetry);
        scheduleReconnect();
      };

      ws.onerror = (error) => {
        if (isManualDisconnect) return;
        const nextRetry = Date.now() + 10000;
        updateConnectionState("retrying", `连接失败（第 ${connectAttempts} 次），等待重连...`, nextRetry);
      };
    } catch (e) {
      updateConnectionState("error", `连接异常: ${e.message}`);
      scheduleReconnect();
    }
  } finally {
    connecting = false;
  }
}

function disconnect() {
  isManualDisconnect = true;
  clearReconnectTimer();
  if (ws) {
    ws.close();
    ws = null;
  }
  updateBadge("", "");
  updateConnectionState("disabled", "连接未启用");
}

function scheduleReconnect() {
  // 不用 setTimeout（Service Worker 被杀后 timer 会丢），依赖 keepalive alarm 重连
  console.log("[ai-bridge] 等待 keepalive alarm 重连...");
}

function clearReconnectTimer() {
  // 保留接口兼容，实际不需要操作
}

function updateBadge(text, color) {
  chrome.action.setBadgeText({ text });
  if (color) {
    chrome.action.setBadgeBackgroundColor({ color });
  }
}

// ---------- 聊天页面 URL 配置 ----------

const PLATFORM_URLS = {
  doubao:   { pattern: "https://www.doubao.com/*",     openUrl: "https://www.doubao.com/chat/" },
  chatgpt:  { pattern: "https://chatgpt.com/*",        openUrl: "https://chatgpt.com/" },
  deepseek: { pattern: "https://chat.deepseek.com/*",  openUrl: "https://chat.deepseek.com/" },
};

// ---------- 处理来自 MCP Server 的消息 ----------

async function handleServerMessage(msg) {
  const { requestId, action, payload } = msg;
  wsLog(`收到指令: ${action} platform: ${payload?.platform}`);

  try {
    const platform = payload?.platform || "doubao";
    const platformConfig = PLATFORM_URLS[platform];
    if (!platformConfig) {
      sendResponse(requestId, false, null, `不支持的平台: ${platform}`);
      return;
    }

    if (action !== "new_session" && action !== "send_message" && action !== "chat") {
      sendResponse(requestId, false, null, `未知的 action: ${action}`);
      return;
    }

    let tab;

    if ((action === "send_message" || action === "chat") && payload?.sessionUrl) {
      const allTabs = await chrome.tabs.query({});
      tab = allTabs.find(t => t.url && t.url.startsWith(payload.sessionUrl));
      if (!tab) {
        wsLog(`会话页面已关闭，重新打开: ${payload.sessionUrl}`);
        tab = await openChatTab(payload.sessionUrl);
      }
    } else {
      tab = await findChatTab(platformConfig.pattern);
      if (!tab) {
        wsLog(`未找到 ${platform} 页面，自动打开...`);
        tab = await openChatTab(platformConfig.openUrl);
      }
    }

    if (!tab) {
      sendResponse(requestId, false, null, "无法打开 AI 聊天页面");
      return;
    }

    await chrome.tabs.update(tab.id, { active: true });

    // DeepSeek new_session 跳过 CS 路径：后台标签页 SPA 点击事件不可靠
    if (platform === "deepseek" && action === "new_session") {
      wsLog("DeepSeek 跳过 CS 路径，使用 CDP 直接操作 DOM");
      await sendMessageViaCDP(tab.id, platform, payload.message);
      await startDebuggerPoll(tab.id, requestId);
      return;
    }

    // 先尝试 content script 路径
    const csReady = await waitForContentScript(tab.id);
    wsLog(`CS ready=${csReady}`);

    if (csReady) {
      try {
        const response = await chrome.tabs.sendMessage(tab.id, {
          requestId, action, payload,
        });
        wsLog(`CS 返回: ${JSON.stringify(response).substring(0,100)}`);

        if (response && response.status === "sent") {
          // CS 成功发送消息，但可能被 CSP 阻止注入 __bridgeCheck
          // 通过 CDP 注入确保 poll 可用
          const cfg = PLATFORM_CDP[platform];
          if (cfg) {
            try {
              await chrome.debugger.attach({ tabId: tab.id }, "1.3");
              wsLog("debugger 已附加（CS 路径）");
            } catch {}
            await injectBridgeCheckViaCDP(tab.id, cfg);
          }
          await startDebuggerPoll(tab.id, requestId);
          return;
        }
        if (response && response.error) {
          wsLog(`CS 返回错误: ${response.error}，尝试 CDP`);
        } else if (response) {
          sendResponse(requestId, true, response);
          return;
        }
      } catch (e) {
        wsLog(`CS 通信异常: ${e.message}，尝试 CDP`);
      }
    } else {
      wsLog("Content script 未就绪，使用 CDP 直接注入");
    }

    // CDP 路径：绕过 content script，通过 chrome.debugger 直接操作 DOM
    if (action === "new_session") {
      await sendMessageViaCDP(tab.id, platform, payload.message);
    } else {
      await sendMessageOnlyViaCDP(tab.id, platform, payload.message);
    }

    await startDebuggerPoll(tab.id, requestId);
  } catch (e) {
    wsLog(`handleServerMessage 异常: ${e.message}`);
    sendResponse(requestId, false, null, e.message);
  }
}

async function findChatTab(pattern) {
  const tabs = await chrome.tabs.query({ url: pattern });
  return tabs.length > 0 ? tabs[0] : null;
}

async function openChatTab(url) {
  const tab = await chrome.tabs.create({ url, active: true });
  wsLog(`已打开新标签: ${url}, tabId=${tab.id}`);

  // 等待页面加载完成
  await new Promise((resolve) => {
    const listener = (tabId, info) => {
      if (tabId === tab.id && info.status === "complete") {
        chrome.tabs.onUpdated.removeListener(listener);
        resolve();
      }
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(() => {
      chrome.tabs.onUpdated.removeListener(listener);
      resolve();
    }, 20000);
  });

  wsLog("页面加载完成，等待 content script 注入...");

  // 页面加载完后额外等一下，让 content script 有时间注入
  await new Promise((r) => setTimeout(r, 2000));

  // 确保 content script 就绪
  await waitForContentScript(tab.id);
  wsLog("content script 已就绪");

  // 重新获取 tab 信息（URL 可能变了）
  const updatedTab = await chrome.tabs.get(tab.id);
  return updatedTab;
}

async function waitForContentScriptPing(tabId, timeoutMs) {
  return Promise.race([
    chrome.tabs.sendMessage(tabId, { action: "ping" }),
    delay(timeoutMs).then(() => { throw new Error("ping timeout"); }),
  ]);
}

async function waitForContentScript(tabId) {
  for (let i = 0; i < 3; i++) {
    try {
      await waitForContentScriptPing(tabId, 2000);
      return true;
    } catch {
      await delay(1000);
    }
  }

  // content script 没就绪，刷新页面（unfreeze tab）
  wsLog("Content script 未就绪，刷新页面重试...");
  try {
    await chrome.tabs.reload(tabId);
    await delay(3000);
    for (let i = 0; i < 3; i++) {
      try {
        await waitForContentScriptPing(tabId, 2000);
        wsLog("刷新后 content script 已就绪");
        return true;
      } catch {
        await delay(1000);
      }
    }
  } catch (e) {
    wsLog(`刷新页面失败: ${e.message}`);
  }

  return false;
}

// 通过 WebSocket 发送日志给 MCP Server
function wsLog(msg) {
  console.log("[ai-bridge]", msg);
  try {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: "log", message: msg }));
    }
  } catch {}
}

async function sendResponse(requestId, success, data, error) {
  if (error) console.log("[ai-bridge]", `错误: ${error}`);
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
  return new Promise((r) => setTimeout(r, ms));
}

// ---------- 平台配置（CDP 直投用） ----------

const PLATFORM_CDP = {
  "doubao": {
    newChatMethod: "shortcut",
    newChatShortcut: { key: "k", meta: true, shift: true },
    inputSelector: "#input-engine-container textarea",
    sendSelector: "#flow-end-msg-send",
    responseSelector: ".md-box-root",
    completion: "data-streaming",
  },
  "chatgpt": {
    newChatMethod: "click",
    newChatSelector: 'a[href="/"]',
    inputSelector: "#prompt-textarea",
    sendSelector: 'button[data-testid="send-button"]',
    responseSelector: '[data-message-author-role="assistant"] .markdown',
    completion: "aria-label",
    completionAriaSelector: "button.composer-submit-button-color",
    completionAriaDoneValue: "发送提示,启动语音功能",
  },
  "deepseek": {
    newChatMethod: "click",
    newChatSelector: 'a[href="/"]',
    inputSelector: "textarea",
    sendSelector: ".ds-button.ds-button--primary",
    responseSelector: ".ds-message",
    completion: "text-only",
  },
};

// ---------- CDP（chrome.debugger）直投函数 ----------
// 当 content script 无法响应时（frozen/background tab），用 CDP 直接操作 DOM

async function ensureDebugger(tabId) {
  try { await chrome.debugger.detach({ tabId }); } catch {}
  await chrome.debugger.attach({ tabId }, "1.3");
}

async function evaluateInPage(tabId, expression) {
  const result = await chrome.debugger.sendCommand(
    { tabId },
    "Runtime.evaluate",
    { expression, returnByValue: true, awaitPromise: false }
  );
  if (result?.exceptionDetails) {
    throw new Error(`CDP 执行错误: ${result.exceptionDetails.text}`);
  }
  return result?.result?.value;
}

function escapeJS(str) {
  return str.replace(/\\/g, '\\\\').replace(/'/g, "\\'").replace(/\n/g, '\\n').replace(/\r/g, '\\r');
}

async function injectBridgeCheckViaCDP(tabId, cfg) {
  let checkExpr = "";

  if (cfg.completion === "data-streaming") {
    checkExpr = `
      var streaming = last.getAttribute('data-streaming');
      if (streaming !== 'false') return {ready: false};
    `;
  } else if (cfg.completion === "aria-label") {
    const ariaSel = escapeJS(cfg.completionAriaSelector || "");
    const ariaVals = escapeJS(cfg.completionAriaDoneValue || "");
    checkExpr = `
      var ariaBtn = document.querySelector('${ariaSel}');
      if (ariaBtn) {
        var doneValues = '${ariaVals}'.split(',').map(function(s){return s.trim();});
        if (!doneValues.includes(ariaBtn.getAttribute('aria-label'))) return {ready: false};
      }
    `;
  } else if (cfg.completion === "text-only") {
    checkExpr = `
      if (!window.__bridgeLastText) window.__bridgeLastText = '';
      var text = (last.innerText || last.textContent || '').trim();
      if (!text) return {ready: false, debug: 'empty-text'};
      if (text !== window.__bridgeLastText) {
        window.__bridgeLastText = text;
        return {ready: false, debug: 'text-changed', len: text.length};
      }
    `;
  }

  const sel = escapeJS(cfg.responseSelector);
  // 清理文本：去掉引用角标（-N- 格式，仅 DeepSeek）和图表 UI 文字
  const cleanCode = `
    function cleanText(t) {
      t = t.replace(/^图表[\\s\\n]*代码[\\s\\n]*下载[\\s\\n]*全屏[\\s\\n]*/gm,'')
            .replace(/[\\s\\n]*图表[\\s\\n]*代码[\\s\\n]*下载[\\s\\n]*全屏/gm,'')
            .replace(/复制[\\s\\n]*下载/gm,'')
            .replace(/[\\t ]+\\n/g,'\\n').replace(/\\n[\\t ]+/g,'\\n')
            .replace(/\\n{3,}/g,'\\n\\n');
      if (window.location.hostname === 'chat.deepseek.com') {
        t = t.replace(/\\s*-\\d+-\\s*/g,' ');
      }
      return t.trim();
    }
  `;
  const expr = `
    window.__bridgeCheck = function() {
      try {
        ${cleanCode}
        var els = document.querySelectorAll('${sel}');
        if (els.length === 0) return {ready: false, debug: 'no-els'};
        var last = els[els.length - 1];
        ${checkExpr}
        var raw = last.innerText || last.textContent || '';
        var text = cleanText(raw);
        return {ready: true, text: text, sessionUrl: window.location.href};
      } catch(e) { return {ready: false, error: e.message}; }
    };
  `;

  await evaluateInPage(tabId, expr);
}

async function sendMessageViaCDP(tabId, platform, message) {
  const cfg = PLATFORM_CDP[platform];
  if (!cfg) throw new Error(`不支持的平台: ${platform}`);

  wsLog(`CDP: debugger 模式发送消息到 ${platform} (tab=${tabId})`);

  // Step 1: 强制刷新页面
  wsLog("CDP newChat: 刷新页面新建会话");
  await chrome.tabs.reload(tabId);
  await new Promise((resolve) => {
    const listener = (id, info) => {
      if (id === tabId && info.status === "complete") {
        chrome.tabs.onUpdated.removeListener(listener);
        setTimeout(resolve, 5000);
      }
    };
    chrome.tabs.onUpdated.addListener(listener);
    setTimeout(() => {
      chrome.tabs.onUpdated.removeListener(listener);
      resolve();
    }, 30000);
  });

  // Step 2: 重新附加 debugger（页面已刷新）
  await ensureDebugger(tabId);

  // Step 3: 输入消息
  const escapedMsg = escapeJS(message);
  const typeResult = await evaluateInPage(tabId,
    `(function(){
      var input=document.querySelector('${cfg.inputSelector}');
      if(!input)return 'no-input';
      var s=Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype,'value')?.set
        ||Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')?.set;
      if(!s)return 'no-setter';
      s.call(input,'${escapedMsg}');
      input.dispatchEvent(new Event('input',{bubbles:true}));
      input.dispatchEvent(new Event('change',{bubbles:true}));
      return 'typed';
    })()`
  );
  wsLog(`CDP type: ${typeResult}`);
  await delay(500);

  // Step 4: 点击发送按钮
  const sendResult = await evaluateInPage(tabId,
    `(function(){
      var btn=document.querySelector('${cfg.sendSelector}');
      if(btn){btn.click();return 'clicked';}
      var inp=document.querySelector('${cfg.inputSelector}');
      if(inp){inp.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true}));return 'enter';}
      return 'no-btn';
    })()`
  );
  wsLog(`CDP send: ${sendResult}`);

  // Step 5: 注入 bridge check 函数
  await injectBridgeCheckViaCDP(tabId, cfg);
  wsLog(`CDP: 消息发送完成`);
}

async function sendMessageOnlyViaCDP(tabId, platform, message) {
  const cfg = PLATFORM_CDP[platform];
  if (!cfg) throw new Error(`不支持的平台: ${platform}`);

  wsLog(`CDP: debugger 模式发送后续消息到 ${platform}`);
  await ensureDebugger(tabId);

  const escapedMsg = escapeJS(message);
  await evaluateInPage(tabId,
    `(function(){
      var input=document.querySelector('${cfg.inputSelector}');
      if(!input)return 'no-input';
      var s=Object.getOwnPropertyDescriptor(window.HTMLTextAreaElement.prototype,'value')?.set
        ||Object.getOwnPropertyDescriptor(window.HTMLInputElement.prototype,'value')?.set;
      if(!s)return 'no-setter';
      s.call(input,'${escapedMsg}');
      input.dispatchEvent(new Event('input',{bubbles:true}));
      input.dispatchEvent(new Event('change',{bubbles:true}));
      return 'typed';
    })()`
  );
  await delay(500);

  const sendResult = await evaluateInPage(tabId,
    `(function(){
      var inp=document.querySelector('${cfg.inputSelector}');
      if(inp){
        inp.focus();
        inp.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',code:'Enter',bubbles:true,cancelable:true}));
        inp.dispatchEvent(new KeyboardEvent('keyup',{key:'Enter',code:'Enter',bubbles:true,cancelable:true}));
        return 'enter';
      }
      var btn=document.querySelector('${cfg.sendSelector}');
      if(btn){btn.click();return 'clicked';}
      return 'no-send';
    })()`
  );
  wsLog(`CDP send: ${sendResult}`);

  await injectBridgeCheckViaCDP(tabId, cfg);
  wsLog(`CDP: 后续消息发送完成`);
}

// ---------- 监听来自 popup 的消息 ----------

chrome.runtime.onMessage.addListener((msg, sender, sendReply) => {
  if (msg.type === "getStatus") {
    let status;
    if (ws && ws.readyState === WebSocket.OPEN) {
      status = "connected";
    } else if (ws && ws.readyState === WebSocket.CONNECTING) {
      status = "connecting";
    } else {
      status = "disconnected";
    }
    sendReply({ connected: status === "connected", status });
    return false;
  }

  if (msg.type === "reconnect") {
    disconnect();
    connect();
    sendReply({ ok: true });
    return false;
  }

  if (msg.type === "disconnect") {
    disconnect();
    sendReply({ ok: true });
    return false;
  }

  return false;
});

// ---------- 监听设置变化 ----------

chrome.storage.onChanged.addListener((changes) => {
  if (changes.enabled) {
    if (changes.enabled.newValue) {
      connect();
      startKeepAlive();
    } else {
      disconnect();
      stopKeepAlive();
    }
  }
});

// ---------- 保活机制 ----------
// Service Worker 会被浏览器在 ~30 秒后杀掉，用 alarm 定期唤醒并检查连接

const KEEPALIVE_ALARM = "ai-bridge-keepalive";

function startKeepAlive() {
  chrome.alarms.create(KEEPALIVE_ALARM, { periodInMinutes: 0.17 }); // 约 10 秒
}

function stopKeepAlive() {
  chrome.alarms.clear(KEEPALIVE_ALARM);
}

// ---------- 后台 Debugger 轮询（替代 content script 的 waitForResponse） ----------
// chrome.debugger 通过 CDP 协议直接评估 page 上下文中的 JS
// 不受 content script JS 被冻结的影响 — CDP 是浏览器进程级别的通信

const DEBUG_POLL_ALARM = "ai-bridge-debug-poll";

async function startDebuggerPoll(tabId, requestId) {
  await chrome.storage.local.set({
    debugger_tabId: tabId,
    debugger_requestId: requestId,
    debugger_started: Date.now(),
  });

  // 如果 CDP 路径已经 attach 了，不要重复 detach-attach
  try {
    await chrome.debugger.attach({ tabId }, "1.3");
    wsLog(`debugger 已附加到 tab ${tabId}`);
  } catch {
    // already attached by CDP send path
  }

  chrome.alarms.create(DEBUG_POLL_ALARM, { periodInMinutes: 0.1 }); // 每 6 秒
}

async function stopDebuggerPoll() {
  chrome.alarms.clear(DEBUG_POLL_ALARM);

  const { debugger_tabId } = await chrome.storage.local.get("debugger_tabId");
  if (debugger_tabId) {
    try { await chrome.debugger.detach({ tabId: debugger_tabId }); } catch {}
  }

  await chrome.storage.local.remove([
    "debugger_tabId",
    "debugger_requestId",
    "debugger_started",
  ]);
}

async function pollDebugger() {
  const { debugger_tabId, debugger_requestId, debugger_started } =
    await chrome.storage.local.get([
      "debugger_tabId",
      "debugger_requestId",
      "debugger_started",
    ]);

  if (!debugger_tabId || !debugger_requestId) return;

  // 10 分钟超时
  if (debugger_started && Date.now() - debugger_started > 600000) {
    sendResponse(debugger_requestId, false, null, "后台轮询超时");
    await stopDebuggerPoll();
    return;
  }

  // 检查标签页是否存在
  try {
    await chrome.tabs.get(debugger_tabId);
  } catch {
    wsLog(`debugger 轮询: 标签页已关闭 (${debugger_tabId})`);
    sendResponse(debugger_requestId, false, null, "标签页已关闭");
    await stopDebuggerPoll();
    return;
  }

  try {
    const result = await chrome.debugger.sendCommand(
      { tabId: debugger_tabId },
      "Runtime.evaluate",
      {
        expression:
          "typeof window.__bridgeCheck === 'function' ? window.__bridgeCheck() : null",
        returnByValue: true,
        awaitPromise: false,
      }
    );

    if (result?.exceptionDetails) {
      wsLog(
        `debugger 轮询: eval异常, text=${result.exceptionDetails.text || ""}`
      );
      return;
    }

    const data = result?.result?.value;

    if (data === null) {
      // __bridgeCheck 未定义 → debugger 重连后未注入，重新注入
      wsLog("debugger 轮询: 重连后重新注入 __bridgeCheck");
      const platformConfig = PLATFORM_CDP[await getPlatformForTab(debugger_tabId)];
      if (platformConfig) {
        try {
          await chrome.debugger.attach({ tabId: debugger_tabId }, "1.3");
        } catch {
          // already attached
        }
        await injectBridgeCheckViaCDP(debugger_tabId, platformConfig);
      }
      return;
    }

    if (data && data.ready) {
      wsLog(
        `debugger 检测到回复完成 (${data.text?.length || 0}字)`
      );
      sendResponse(debugger_requestId, true, {
        response: data.text || "",
        sessionUrl: data.sessionUrl || "",
      });
      await stopDebuggerPoll();
    } else {
      var msg = "ready=false";
      if (data && data.error) msg += ", error=" + data.error;
      if (data && data.debug) msg += ", debug=" + data.debug;
      if (data && data.dbgLen !== undefined) msg += ", len=" + data.dbgLen;
      wsLog("debugger 轮询: " + msg);
    }
  } catch (e) {
    const msg = e.message || "";
    if (
      msg.includes("Inspected target navigated or closed") ||
      msg.includes("target doesn't exist") ||
      msg.includes("No tab with id")
    ) {
      wsLog(`debugger 轮询停止: ${msg}`);
      sendResponse(
        debugger_requestId,
        false,
        null,
        `标签页异常: ${msg}`
      );
      await stopDebuggerPoll();
    } else if (
      msg.includes("Debugger is not attached") ||
      msg.includes("Session with given id not found") ||
      msg.includes("Protocol error") ||
      msg.includes("is not attached")
    ) {
      // SW 重启后 debugger session 丢失 → 重新 attach 并注入
      wsLog(`debugger 轮询: debugger 丢失 (${msg})，重新 attach`);
      try {
        await chrome.debugger.attach({ tabId: debugger_tabId }, "1.3");
      } catch (attachErr) {
        wsLog(`debugger 轮询: 重新 attach 失败: ${attachErr.message}`);
        return;
      }
      const platformConfig = PLATFORM_CDP[await getPlatformForTab(debugger_tabId)];
      if (platformConfig) {
        await injectBridgeCheckViaCDP(debugger_tabId, platformConfig);
      }
    } else {
      wsLog(`debugger 轮询: 未知错误: ${msg}`);
    }
  }
}

async function getPlatformForTab(tabId) {
  try {
    const tab = await chrome.tabs.get(tabId);
    const url = tab.url || "";
    if (url.includes("doubao.com")) return "doubao";
    if (url.includes("chatgpt.com")) return "chatgpt";
    if (url.includes("deepseek.com")) return "deepseek";
  } catch {}
  return "doubao";
}

// ---------- Alarm 统一处理 ----------

chrome.alarms.onAlarm.addListener(async (alarm) => {
  if (alarm.name === KEEPALIVE_ALARM) {
    if (!ws || ws.readyState !== WebSocket.OPEN) {
      updateBadge("...", "#FF9800");
      await connect();
    }
  }
  if (alarm.name === DEBUG_POLL_ALARM) {
    await pollDebugger();
  }
});

// ---------- 启动时自动连接 ----------

chrome.runtime.onInstalled.addListener(async () => {
  const { enabled } = await getSettings();
  if (enabled) {
    connect();
    startKeepAlive();
  }
});

chrome.runtime.onStartup.addListener(async () => {
  const { enabled } = await getSettings();
  if (enabled) {
    connect();
    startKeepAlive();
  }
});

(async () => {
  const { enabled } = await getSettings();
  if (enabled) {
    connect();
    startKeepAlive();
  }
})();
