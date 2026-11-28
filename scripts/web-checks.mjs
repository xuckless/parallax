// Checks patches 0034, 0035 and 0046 in a browser: hello's IME mirror (attributes, placement, the password element), its
// lifecycle phases and its keyboard inset (docs/web-text-input.md, docs/web-lifecycle.md). Needs hello served
// (docs/examples-run.md). Two drivers:
// - chromium: a Chromium started as record-traces.sh's web_setup does (--headless=new --remote-debugging-port=<port>
//   --window-size=412,915 --force-device-scale-factor=1 --touch-events=enabled), over the DevTools Protocol with
//   Node 22's built-in WebSocket, no deps. Only it can compose, hide, freeze and resume the page.
// - firefox, webkit: Playwright's builds, loaded from PLAYWRIGHT_DIR (a directory whose node_modules holds playwright;
//   its browsers come from PLAYWRIGHT_BROWSERS_PATH). Checks needing what Playwright can't drive print "skip".
// Usage: node scripts/web-checks.mjs <hello url> desktop|mobile <chromium debugging port | firefox | webkit>
// With a fourth argument `native-view` and native-view-web's url, it checks patches 0066-0069's native views instead
// (docs/native-view-web-checks.md): input into the views' elements and the divs over them, focus between them and
// GPUI's field, page content around an offset canvas, 0 GPUI frames while only the elements change, the alpha mode,
// each screenshot's content (holes cut, views, divs and GPUI drew, no gap; still and animating) at the page's scale,
// and, on Chromium, the toggle's and the views' costs (on a phone profile in mobile mode). A fifth argument is the page's scale (default
// 1), emulated: Emulation.setDeviceMetricsOverride on Chromium, the context's deviceScaleFactor in Playwright.
// Prints each check; exits 1 if any fails.
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";

const [url, mode, browser, suite, scale = "1"] = process.argv.slice(2);
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
    move: (x, y) => send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y }),
    click: async (x, y) => {
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      for (const type of ["mousePressed", "mouseReleased"]) {
        await send("Input.dispatchMouseEvent", { type, x, y, button: "left", clickCount: 1 });
      }
    },
    drag: async ([x0, y0], [x1, y1]) => {
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x: x0, y: y0 });
      await send("Input.dispatchMouseEvent", { type: "mousePressed", x: x0, y: y0, button: "left", clickCount: 1 });
      for (let step = 1; step <= 10; step++) {
        const at = { x: x0 + ((x1 - x0) * step) / 10, y: y0 + ((y1 - y0) * step) / 10 };
        await send("Input.dispatchMouseEvent", { type: "mouseMoved", ...at, button: "left", buttons: 1 });
      }
      await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: x1, y: y1, button: "left", clickCount: 1 });
    },
    wheel: async (x, y, deltaY) => {
      await send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
      await send("Input.dispatchMouseEvent", { type: "mouseWheel", x, y, deltaX: 0, deltaY });
    },
    screenshot: async (path) => {
      const { data } = await send("Page.captureScreenshot", { format: "png" });
      (await import("node:fs")).writeFileSync(path, Buffer.from(data, "base64"));
    },
    // Takes effect on the next navigation's first layout too; `mobile` also turns on the mobile viewport.
    viewport: (width, height) =>
      send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: Number(scale), mobile: mode === "mobile" }),
    throttle: (rate) => send("Emulation.setCPUThrottlingRate", { rate }),
    taskSeconds: async () => {
      await send("Performance.enable");
      return (await send("Performance.getMetrics")).metrics.find((m) => m.name === "TaskDuration").value;
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
  let page;
  // A new page in a new context: Playwright sets the scale per context.
  const open = async (width, height, deviceScaleFactor) => {
    await page?.context().close();
    const context = await instance.newContext({ viewport: { width, height }, deviceScaleFactor, ...touch });
    page = await context.newPage();
    page.on("console", (message) => logs.push(message.text()));
  };
  await open(412, 915, Number(scale));
  return {
    navigate: (url) => page.goto(url),
    js: (expression) => page.evaluate(expression),
    tap: (x, y) => (mode === "mobile" ? page.touchscreen.tap(x, y) : page.mouse.click(x, y)),
    move: (x, y) => page.mouse.move(x, y),
    click: (x, y) => page.mouse.click(x, y),
    drag: async ([x0, y0], [x1, y1]) => {
      await page.mouse.move(x0, y0);
      await page.mouse.down();
      await page.mouse.move(x1, y1, { steps: 10 });
      await page.mouse.up();
    },
    wheel: async (x, y, deltaY) => {
      await page.mouse.move(x, y);
      await page.mouse.wheel(0, deltaY);
    },
    screenshot: (path) => page.screenshot({ path }),
    viewport: (width, height) => open(width, height, Number(scale)),
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

if (suite === "native-view") {
  await nativeViewChecks();
  await driver.close();
  process.exit(failed ? 1 : 0);
}

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
  // The mirror sits over the canvas there; `pointer-events: none` (patch 0046) leaves the pixel's clicks to the canvas.
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

// native-view-web (examples/native-view-web, src/lib.rs's VIEWS), in CSS px of a 900 x 700 page: each view is
// 400 x 260 at its corner of the canvas, with a GPUI div at +20..+180, +20..+80 over it. `?offset` puts the canvas
// at (8, 48), under a header of page content.
async function nativeViewChecks() {
  const views = { same: [20, 60], cross: [460, 60], canvas: [20, 360], form: [460, 360] };
  const names = Object.keys(views);
  const [VIEW_W, VIEW_H, OVER_W, OVER_H] = [400, 260, 160, 60];
  let origin = [0, 0];
  const at = (name, [x, y]) => [origin[0] + views[name][0] + x, origin[1] + views[name][1] + y];
  const uncovered = (name) => at(name, [280, 190]);
  const covered = (name) => at(name, [80, 50]);
  const shots = mkdtempSync(`${process.env.SHOTS_DIR ?? tmpdir()}/native-view-web-`);
  const pixels = (...args) => execFileSync("python3", ["-I", new URL("pixels.py", import.meta.url).pathname, ...args]).toString().trim();
  const count = (path, colour) => Number(pixels(path, "count", colour));
  const s = Number(scale);
  const rgb = (path, [x, y]) => pixels(path, "at", String(Math.round(x * s)), String(Math.round(y * s))).split(" ").map(Number);
  // Each view's content, looked for at points clear of its div, its text and (±12 px) the animation's shift: the
  // iframes' frame.html gradient (blue, not the slot's white), the canvas's hsl(h 70% 60%) (two points, one clear of
  // its moving black bar) and the form's orange. Add a row per view.
  const hsl = ([r, g, b]) => {
    const [max, min] = [Math.max(r, g, b) / 255, Math.min(r, g, b) / 255];
    const l = (max + min) / 2;
    return [max === min ? 0 : (max - min) / (1 - Math.abs(2 * l - 1)), l];
  };
  const LOOKS = {
    same: [[[280, 200]], ([r, , b]) => b > 200 && r > 120 && r < 230],
    cross: [[[280, 200]], ([r, , b]) => b > 200 && r > 120 && r < 230],
    canvas: [[[100, 130], [140, 130]], (pixel) => { const [sat, l] = hsl(pixel); return sat > 0.55 && sat < 0.85 && l > 0.5 && l < 0.7; }],
    form: [[[200, 130]], ([r, g, b]) => Math.abs(r - 0xe0) < 24 && Math.abs(g - 0xa0) < 24 && Math.abs(b - 0x30) < 24],
  };
  // The page's gap colour (green: a hole with no element under it), GPUI's view boxes (magenta: a view not cut out),
  // the divs over the views, GPUI's background and each view's content, in one screenshot.
  const content = async (name, shown = names) => {
    const path = `${shots}/${name}.png`;
    await driver.screenshot(path);
    const divs = count(path, "2050d0") / (shown.length * OVER_W * OVER_H * s * s);
    const canvasArea = (900 - 2 * origin[0]) * (700 - origin[1] - origin[0]);
    const background = count(path, "202020") / ((canvasArea - shown.length * VIEW_W * VIEW_H) * s * s);
    return {
      gap: count(path, "00ff00"),
      uncut: count(path, "c000c0"),
      divs: divs > 0.9 && divs < 1.1 ? "drew" : divs.toFixed(2),
      background: background > 0.5 ? "drew" : background.toFixed(2),
      views: Object.fromEntries(shown.map((view) => {
        const [points, looks] = LOOKS[view];
        const seen = points.map((point) => rgb(path, at(view, point)));
        return [view, seen.some(looks) ? "drew" : seen.map((pixel) => pixel.join(" ")).join(", ")];
      })),
    };
  };
  const drew = (shown = names) => ({ gap: 0, uncut: 0, divs: "drew", background: "drew", views: Object.fromEntries(shown.map((v) => [v, "drew"])) });
  const open = async (query) => {
    await driver.viewport(900, 700);
    logs.length = 0;
    await driver.navigate(`${url}${query}`);
    for (let i = 0; i < 150 && !logs.some((line) => line.includes("window open")); i++) await sleep(200);
    await sleep(2500);
  };
  const press = (x, y) => (mode === "mobile" ? driver.tap(x, y) : driver.click(x, y));
  const events = async (frame, kind) =>
    (await js("window.frameEvents.splice(0)")).filter((e) => e.frame === frame && e.kind === kind);
  const said = (pattern) => logs.some((line) => pattern.test(line));
  const saidSince = (mark, text) => logs.slice(mark).some((line) => line.includes(text));
  const command = async (text) => {
    await js(`window.nativeViewCommand(${JSON.stringify(text)})`);
    await sleep(600);
  };
  const active = () => js("document.activeElement.localName");
  // A real click (or tap) into each element at view-relative (280, 190), and the GPUI div over each.
  const pressChecks = async (label) => {
    for (const name of names) {
      await js("window.frameEvents.length = 0");
      await press(...uncovered(name));
      await sleep(500);
      check(`${label}${name} element takes a real ${mode === "mobile" ? "tap" : "click"} at view-relative (280, 190)`,
        (await events(name, "click")).map(({ x, y, trusted }) => [Math.round(x), Math.round(y), trusted]), [[280, 190, true]]);
      const mark = logs.length;
      await press(...covered(name));
      await sleep(500);
      check(`${label}the div over ${name} takes its own press`,
        [saidSince(mark, `GPUI div over ${name}: down`), (await events(name, "click")).length], [true, 0]);
    }
  };

  await open("");
  check(`page scale ${scale}`, await js("devicePixelRatio"), Number(scale));
  check("alpha mode logged", said(/native views: (window surface alpha mode PreMultiplied|WebGL2 canvas, premultiplied)/), true);
  const [gpui0, element0] = await js("[window.gpuiFrames, window.elementFrames]");
  await sleep(2000);
  const [gpui1, element1] = await js("[window.gpuiFrames, window.elementFrames]");
  check("GPUI frames while only the elements change", gpui1 - gpui0, 0);
  check("the canvas element drew frames meanwhile", element1 - element0 > 30, true);
  check("still: holes cut, views, divs and GPUI drew, no gap", await content("still"), drew());

  await pressChecks("");
  // A press into the form lands in its <input>: GPUI focusing the view (from its field) must leave it there.
  await command("field");
  await press(...uncovered("form"));
  await sleep(500);
  await driver.insertText("hi");
  await sleep(300);
  check("focus inside an element (its <input>) stays when GPUI focuses the view",
    await js("[document.activeElement.localName, document.querySelector('gpui-native-view input').value]"), ["input", "hi"]);

  if (mode === "desktop") {
    await driver.wheel(...uncovered("same"), 200);
    await sleep(600);
    check("the same-origin iframe scrolls by a real wheel", (await events("same", "scroll")).some((e) => e.top > 0), true);
    const mark = logs.length;
    await driver.wheel(...covered("same"), 200);
    await sleep(600);
    check("a wheel over the div is GPUI's", [saidSince(mark, "GPUI div over same: scroll"),
      (await events("same", "wheel")).length], [true, 0]);
    await js("window.frameEvents.length = 0");
    await driver.drag([views.cross[0] + 5, views.cross[1] + 130], [views.cross[0] + 390, views.cross[1] + 130]);
    await sleep(500);
    check("text selects in the cross-origin iframe", (await events("cross", "selection")).some((e) => e.text.startsWith("A page")), true);
    const pointerEvents = () => js("document.body.querySelector(':scope > canvas').style.pointerEvents");
    await driver.move(...uncovered("canvas"));
    await sleep(100);
    const overView = await pointerEvents();
    await driver.move(880, 680);
    await sleep(100);
    check("the canvas lets the pointer through only over a view", [overView, await pointerEvents()], ["none", "auto"]);
  }

  // A shield left over a spot where GPUI's content no longer is (moved there by hand): the pointer there must reach
  // the element under it, and the shield go back where GPUI's frame has its occluder.
  const shield = "document.querySelector('gpui-native-views > div:nth-child(2) > div')";
  const placed = await js(`${shield}.style.transform`);
  await js(`${shield}.style.transform = 'translate(${views.canvas[0] + 260}px, ${views.canvas[1] + 170}px)'`);
  await js("window.frameEvents.length = 0");
  await press(...uncovered("canvas"));
  await sleep(500);
  check("a stale shield gives its press to the element under it",
    [await js(`${shield}.style.transform`) === placed,
      (await events("canvas", "pointerdown")).some(({ x, y }) => Math.round(x) === 280 && Math.round(y) === 190)], [true, true]);

  // Focus: GPUI's field must get the keys back from a view's element, also when that view goes.
  await open("");
  const enter = async () => {
    const mark = logs.length;
    await driver.pressEnter();
    await sleep(400);
    return saidSince(mark, "field key enter");
  };
  await press(...uncovered("same"));
  await sleep(500);
  const inFrame = await active();
  await command("field");
  check("GPUI field focused from inside a view: typing reaches it", [inFrame, await enter()], ["iframe", true]);
  await press(...uncovered("same"));
  await sleep(500);
  await command("drop same");
  await command("field");
  check("focused view dropped then GPUI field focused: typing reaches it", await enter(), true);
  for (const name of names.slice(1)) await command(`drop ${name}`);
  check("the last view gone, the canvas has its own styles back and no layer",
    await js("(() => { const c = document.body.querySelector(':scope > canvas'); return [c.style.position, c.style.zIndex, c.style.pointerEvents, !!document.querySelector('gpui-native-views')]; })()"),
    ["", "", "", false]);

  // The canvas away from the page's origin, under page content.
  origin = [8, 48];
  await open("?offset");
  check("offset: holes cut, views, divs and GPUI drew, no gap", await content("offset"), drew());
  await pressChecks("offset: ");
  await js("window.frameEvents.length = 0");
  await press(100, 28);
  await sleep(400);
  check("page content outside the canvas takes a real press while views show",
    (await events("header", "click")).map(({ trusted }) => trusted), [true]);
  if (mode === "desktop") {
    await driver.wheel(100, 28, 100);
    await sleep(600);
    check("page content outside the canvas scrolls while views show", (await events("header", "scroll")).some((e) => e.top > 0), true);
  }
  origin = [0, 0];

  await open("?animate");
  const moving = [];
  for (let i = 0; i < 8; i++) moving.push(await content(`animate-${i}`));
  check("while moving and resizing: holes cut, views, divs and GPUI drew, no gap", moving, Array(8).fill(drew()));

  if (!driver.throttle) {
    skip("the toggle's and the views' costs");
    return;
  }
  // On a phone profile (mobile, scale 2.625) the CPU runs 4x slower. Moves cross
  // the views and GPUI's content; the cost is the time from the page's first capture listener to its last bubbling
  // one, with views and without.
  await driver.throttle(mode === "mobile" ? 4 : 1);
  const path = Array.from({ length: 200 }, (_, i) => [20 + ((i * 37) % 840), 60 + ((i * 53) % 560)]);
  const perMove = async (query) => {
    await open(query);
    await js("window.moveTimes.length = 0");
    for (const [x, y] of path) await driver.move(x, y);
    const times = await js("window.moveTimes");
    return times.reduce((sum, time) => sum + time, 0) / times.length;
  };
  const withViews = await perMove("");
  const without = await perMove("?noview");
  console.log(`measure pointer move handling: ${withViews.toFixed(3)} ms with views, ${without.toFixed(3)} ms without`);
  const perFrame = async (query) => {
    await open(query);
    const [frames0, task0] = [await js("window.gpuiFrames"), await driver.taskSeconds()];
    await sleep(3000);
    const [frames1, task1] = [await js("window.gpuiFrames"), await driver.taskSeconds()];
    return [(frames1 - frames0) / 3, ((task1 - task0) * 1000) / (frames1 - frames0)];
  };
  const [fpsViews, taskViews] = await perFrame("?animate");
  const [fpsNone, taskNone] = await perFrame("?animate&noview");
  console.log(`measure animating: ${fpsViews.toFixed(1)} GPUI frames/s, ${taskViews.toFixed(2)} ms main-thread task per frame with views; ` +
    `${fpsNone.toFixed(1)} frames/s, ${taskNone.toFixed(2)} ms without`);
  await driver.throttle(1);
}
