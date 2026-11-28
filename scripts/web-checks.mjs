// Checks patches 0034, 0035 and 0043 in a browser: hello's IME mirror (attributes, placement, the password element), its
// lifecycle phases and its keyboard inset (docs/web-text-input.md, docs/web-lifecycle.md). Needs hello served
// (docs/examples-run.md). Two drivers:
// - chromium: a Chromium started as record-traces.sh's web_setup does (--headless=new --remote-debugging-port=<port>
//   --window-size=412,915 --force-device-scale-factor=1 --touch-events=enabled), over the DevTools Protocol with
//   Node 22's built-in WebSocket, no deps. Only it can compose, hide, freeze and resume the page.
// - firefox, webkit: Playwright's builds, loaded from PLAYWRIGHT_DIR (a directory whose node_modules holds playwright;
//   its browsers come from PLAYWRIGHT_BROWSERS_PATH). Checks needing what Playwright can't drive print "skip".
// Usage: node scripts/web-checks.mjs <hello url> desktop|mobile <chromium debugging port | firefox | webkit>
// Prints each check; exits 1 if any fails.
import { createRequire } from "node:module";

const [url, mode, browser] = process.argv.slice(2);
if (!["desktop", "mobile"].includes(mode)) throw new Error("web-checks: mode is desktop or mobile");
// An Android phone: touch input, and the user agent that makes gpui_web treat the browser as mobile.
const ANDROID = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const logs = [];

// The page, over the DevTools Protocol. Optional steps (compose, hide, show, freeze, resume) exist only here.
const chromium = async (port) => {
  const page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page");
  const socket = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
  let nextId = 0;
  const pending = new Map();
  socket.onmessage = (message) => {
    const reply = JSON.parse(message.data);
    if (reply.method === "Runtime.consoleAPICalled") logs.push(reply.params.args.map((a) => a.value ?? "").join(" "));
    if (!pending.has(reply.id)) return;
    const { resolve, reject } = pending.get(reply.id);
    pending.delete(reply.id);
    reply.error ? reject(new Error(reply.error.message)) : resolve(reply.result);
  };
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    const id = ++nextId;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
  await send("Runtime.enable");
  await send("Emulation.setFocusEmulationEnabled", { enabled: true });
  if (mode === "mobile") {
    await send("Emulation.setUserAgentOverride", { userAgent: ANDROID });
    await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
  }
  let front;
  return {
    navigate: (url) => send("Page.navigate", { url }),
    js: async (expression) => (await send("Runtime.evaluate", { expression, returnByValue: true })).result.value,
    tap: async (x, y) => {
      if (mode === "mobile") {
        await send("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [{ x, y }] });
        await sleep(60);
        await send("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
      } else {
        for (const type of ["mousePressed", "mouseReleased"]) {
          await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 });
        }
      }
    },
    insertText: (text) => send("Input.insertText", { text }),
    pressEnter: async () => {
      for (const type of ["keyDown", "keyUp"]) {
        await send("Input.dispatchKeyEvent", { type, key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
      }
    },
    compose: (text) => send("Input.imeSetComposition", { text, selectionStart: text.length, selectionEnd: text.length }),
    // A tab opened in front hides this one (blur, then visibilitychange), as in a desktop browser. Turning focus
    // emulation off alone hides nothing: a fresh headless Chromium's only tab is visible and focused, and stays so.
    hide: async () => {
      await send("Emulation.setFocusEmulationEnabled", { enabled: false });
      front = (await send("Target.createTarget", { url: "about:blank" })).targetId;
    },
    show: async () => {
      await send("Target.activateTarget", { targetId: page.id });
      await send("Target.closeTarget", { targetId: front });
      await send("Emulation.setFocusEmulationEnabled", { enabled: true });
    },
    freeze: () => send("Page.setWebLifecycleState", { state: "frozen" }),
    resume: () => send("Page.setWebLifecycleState", { state: "active" }),
    close: () => socket.close(),
  };
};

// The page in Playwright's Firefox or WebKit: their pages stay visible and focused whatever else opens.
const playwright = async (engine) => {
  const launcher = createRequire(`${process.env.PLAYWRIGHT_DIR}/`)("playwright")[engine];
  const instance = await launcher.launch();
  const touch = mode === "mobile" ? { hasTouch: true, userAgent: ANDROID } : {};
  const context = await instance.newContext({ viewport: { width: 412, height: 915 }, deviceScaleFactor: 1, ...touch });
  const page = await context.newPage();
  page.on("console", (message) => logs.push(message.text()));
  return {
    navigate: (url) => page.goto(url),
    js: (expression) => page.evaluate(expression),
    tap: (x, y) => (mode === "mobile" ? page.touchscreen.tap(x, y) : page.mouse.click(x, y)),
    insertText: (text) => page.keyboard.insertText(text),
    pressEnter: () => page.keyboard.press("Enter"),
    close: () => instance.close(),
  };
};

const driver = ["firefox", "webkit"].includes(browser) ? await playwright(browser) : await chromium(browser);
const { js } = driver;
let failed = 0;
const check = (name, actual, expected) => {
  const ok = JSON.stringify(actual) === JSON.stringify(expected);
  if (!ok) failed++;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}: ${JSON.stringify(actual)}${ok ? "" : ` (want ${JSON.stringify(expected)})`}`);
};
const skip = (name) => console.log(`skip ${name}: ${browser} can't drive it`);
// hello's log lines since the last call that match `pattern`, each line's last word.
let seen = 0;
const heard = (pattern) => {
  const lines = logs.slice(seen).filter((line) => pattern.test(line)).map((line) => line.split(" ").pop());
  seen = logs.length;
  return lines;
};
// The mirror's style properties and attributes by name; `autocapitalize` as its IDL value, since Firefox rewrites the
// attribute "off" as its synonym "none".
const mirror = (names) => js(`(() => { const t = document.querySelector('textarea, input');
  const read = (n) => n === 'autocapitalize' ? t.autocapitalize : n in t.style ? t.style[n] : t.getAttribute(n);
  return Object.fromEntries(${JSON.stringify(names)}.map((n) => [n, read(n)])); })()`);
const tap = async (x, y) => {
  await driver.tap(x, y);
  await sleep(500);
};

await driver.navigate(url);
for (let i = 0; i < 150 && !logs.some((line) => line.includes("window open")); i++) await sleep(200);
// "window open" comes before the first frame; a tap before it hits nothing.
await sleep(3000);

// hello's fields: prose at y 44, email at y 110, password at y 176 (CSS px, 412 px wide).
const attributes = ["autocomplete", "autocorrect", "autocapitalize", "enterkeyhint", "spellcheck", "inputmode"];
await tap(200, 44);
check("prose field attributes", await mirror(attributes), {
  autocomplete: "off", autocorrect: "on", autocapitalize: "sentences", enterkeyhint: mode === "mobile" ? "done" : null,
  spellcheck: "true", inputmode: mode === "mobile" ? "text" : null,
});
await tap(200, 110);
check("email field attributes", await mirror(attributes), {
  autocomplete: "email", autocorrect: "off", autocapitalize: "none", enterkeyhint: mode === "mobile" ? "done" : null,
  spellcheck: "false", inputmode: mode === "mobile" ? "email" : null,
});
if (mode === "desktop") {
  check("mirror on the email caret", await mirror(["left", "top"]), { left: "32px", top: "104px" });
  // The mirror sits over the canvas there; `pointer-events: none` (patch 0043) leaves the pixel's clicks to the canvas.
  check("the caret's pixel is the canvas's", await js(`document.elementFromPoint(32, 104).localName`), "canvas");
  if (driver.compose) {
    await driver.compose("ni");
    await sleep(300);
    check("mirror stays put mid-composition", await mirror(["left", "top"]), { left: "32px", top: "104px" });
  } else {
    skip("mirror stays put mid-composition");
  }
  await driver.insertText("你");
  await sleep(400);
  const { top, left } = await mirror(["top", "left"]);
  check("mirror follows the caret after the commit", [top, left !== "32px"], ["104px", true]);
  // A selection the browser moves (here the page) reaches the field through the document's `selectionchange`.
  await js(`document.querySelector('textarea, input').setSelectionRange(0, 0)`);
  await sleep(400);
  check("selection import moves the caret", await mirror(["left", "top"]), { left: "32px", top: "104px" });
} else {
  check("mirror stays at the top on touch", await mirror(["left", "top"]), { left: "0px", top: "0px" });
  heard(/keyboard inset/);
  // A 300 px keyboard shrinks the visual viewport, as Android Chrome's does; then it closes.
  await js(`(() => { const height = Object.getOwnPropertyDescriptor(VisualViewport.prototype, 'height');
    const full = height.get.call(visualViewport);
    window.__height = height;
    Object.defineProperty(VisualViewport.prototype, 'height', { configurable: true, get: () => full - 300 });
    visualViewport.dispatchEvent(new Event('resize')); })()`);
  await sleep(600);
  check("keyboard open", heard(/keyboard inset/), ["300"]);
  await js(`(() => { Object.defineProperty(VisualViewport.prototype, 'height', window.__height);
    visualViewport.dispatchEvent(new Event('resize')); })()`);
  await sleep(600);
  check("keyboard closed", heard(/keyboard inset/), ["0"]);
}

// The mirror is an <input type=password> while the password field is focused, a textarea after.
const element = () => js(`(() => { const t = document.querySelector('textarea, input');
  return [t.localName, t.getAttribute('type'), t.getAttribute('autocomplete'), t === document.activeElement]; })()`);
await tap(200, 176);
check("password field element", await element(), ["input", "password", "current-password", true]);
check("the password element lets clicks through", await mirror(["pointerEvents"]), { pointerEvents: "none" });
await driver.insertText("hunter2");
await driver.pressEnter();
await sleep(400);
check("password typed into the input", logs.filter((line) => line.includes("submitted: 7 bytes")).length, 1);
await tap(200, 110);
check("email field element again", await element(), ["textarea", null, "email", true]);

// The phases hello logs, waiting up to 3 s for `want` of them, then 300 ms for any extra.
const phases = async (want) => {
  let got = heard(/ lifecycle /);
  for (let i = 0; i < 30 && got.length < want; i++) {
    await sleep(100);
    got = got.concat(heard(/ lifecycle /));
  }
  await sleep(300);
  return got.concat(heard(/ lifecycle /));
};
heard(/ lifecycle /);
if (driver.hide) {
  await driver.hide();
  check("page hidden", await phases(2), ["Inactive", "Background"]);
  await driver.freeze();
  await sleep(300);
  await driver.resume();
  await sleep(400);
  check("frozen and resumed while hidden", await phases(0), []);
  await driver.show();
  check("page shown and focused", await phases(2), ["Foreground", "Active"]);
} else {
  for (const name of ["page hidden", "frozen and resumed while hidden", "page shown and focused"]) skip(name);
}
await js(`window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: true }))`);
await sleep(200);
check("into the back/forward cache", await phases(2), ["Inactive", "Background"]);
await js(`window.dispatchEvent(new PageTransitionEvent('pageshow', { persisted: true }))`);
await sleep(300);
check("back from the back/forward cache", await phases(2), ["Foreground", "Active"]);
await js(`window.dispatchEvent(new PageTransitionEvent('pagehide', { persisted: false }))`);
await sleep(300);
check("unloading", await phases(3), ["Inactive", "Background", "Disconnected"]);

await driver.close();
process.exit(failed ? 1 : 0);
