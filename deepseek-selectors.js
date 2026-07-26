// ============================================
// 在 chat.deepseek.com 按 F12 → 控制台 → 粘贴运行
// ============================================

// 1. 输入框
console.log("=== 输入框 ===");
const input = document.querySelector('textarea') || document.querySelector('[contenteditable="true"]');
console.log("标签:", input?.tagName);
console.log("选择器:", 'textarea' || '[contenteditable="true"]');
console.log("占位符:", input?.placeholder);

// 2. 发送按钮
console.log("\n=== 发送按钮 ===");
const btns = document.querySelectorAll('button');
for (const b of btns) {
  const html = b.outerHTML.substring(0, 300);
  if (html.includes('发送') || html.includes('send') || html.includes('arrow') || html.includes('Submit')) {
    console.log(html);
    console.log('---');
  }
}

// 3. 回复区 - 检测 data-streaming
console.log("\n=== 回复区 data-streaming 检测 ===");
const resp = document.querySelector('[class*="ds-markdown"]');
if (resp) {
  const container = resp.closest('[data-streaming]');
  if (container) {
    console.log("找到 data-streaming 容器:", container.outerHTML.substring(0, 300));
    console.log("当前值:", container.getAttribute('data-streaming'));
  } else {
    console.log("无 data-streaming 属性");
  }
}

// 4. 停止按钮
console.log("\n=== 停止按钮 ===");
for (const b of btns) {
  if (b.textContent.includes('停止') || b.textContent.includes('Stop')) {
    console.log(b.outerHTML.substring(0, 300));
  }
}

// 5. 完整选择器建议
console.log("\n=== 建议的选择器 ===");
console.log("输入框 selector:", input ? 'textarea' || '[contenteditable="true"]' : '未找到');
console.log("输入框 method:", input?.tagName === 'TEXTAREA' ? 'react-setter' : 'exec-command');
