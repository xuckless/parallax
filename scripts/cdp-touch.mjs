// Drives a page in a running Chromium over the Chrome DevTools Protocol: the browser driver of record-traces.sh
// (docs/traces.md). One command per run, against the browser's first page; Node 22's built-in WebSocket, no deps.
//
// Usage: node scripts/cdp-touch.mjs <debugging port> <command> [args]   (positions in CSS px, times in ms)
//   open <url>                          load the page (touch emulation on, so it sees a touchscreen)
//   screenshot <file.png>               the viewport, at the browser's device scale factor
//   tap <x> <y> <ms>                    one finger down, held <ms>, up
//   taps <x> <y> <count> <pause> <ms>   <count> taps, <pause> ms apart
//   swipe <x1> <y1> <x2> <y2> <ms>      one finger moved in a line over <ms>, a move per 16 ms frame
//   pinch <x> <y> <radius> <scale> <ms> two fingers <radius> either side of x, spread to <radius>·<scale>
import { writeFileSync } from "node:fs";

const [port, command, ...args] = process.argv.slice(2);
const FRAME_MS = 16;
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((target) => target.type === "page");
if (!page) throw new Error("cdp-touch: no page in the browser");

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.onopen = resolve;
  socket.onerror = reject;
});
let nextId = 0;
const pending = new Map();
socket.onmessage = (message) => {
  const reply = JSON.parse(message.data);
  if (!pending.has(reply.id)) return;
  const { resolve, reject } = pending.get(reply.id);
  pending.delete(reply.id);
  reply.error ? reject(new Error(`${reply.error.message}`)) : resolve(reply.result);
};
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const id = ++nextId;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
// Each finger is { x, y, id }; an event lists the fingers still down (touchEnd: none).
const touch = (type, fingers) =>
  send("Input.dispatchTouchEvent", {
    type,
    touchPoints: fingers.map(({ x, y, id }) => ({ x, y, id, radiusX: 1, radiusY: 1, force: 1 })),
  });
// Moves every finger from `from` to `to` over `ms`, a touchMove at most every frame. Each move goes where the clock
// says, not one step further: a dispatch waits for the page's next frame (about 33 ms in headless Chromium), so
// counting steps would stretch the gesture and slow a fling.
const glide = async (from, to, ms) => {
  const start = performance.now();
  for (let t = 0; t < 1; ) {
    await sleep(FRAME_MS);
    t = Math.min(1, (performance.now() - start) / ms);
    await touch(
      "touchMove",
      from.map((finger, i) => ({ ...finger, x: finger.x + (to[i].x - finger.x) * t, y: finger.y + (to[i].y - finger.y) * t })),
    );
  }
};
const tap = async (x, y, ms) => {
  await touch("touchStart", [{ x, y, id: 0 }]);
  await sleep(ms);
  await touch("touchEnd", []);
};

const n = args.map(Number);
const commands = {
  open: async () => {
    await send("Emulation.setTouchEmulationEnabled", { enabled: true, maxTouchPoints: 5 });
    await send("Page.navigate", { url: args[0] });
  },
  screenshot: async () => {
    const { data } = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(args[0], Buffer.from(data, "base64"));
  },
  tap: () => tap(n[0], n[1], n[2]),
  taps: async () => {
    for (let i = 0; i < n[2]; i++) {
      if (i > 0) await sleep(n[3]);
      await tap(n[0], n[1], n[4]);
    }
  },
  swipe: async () => {
    const from = [{ x: n[0], y: n[1], id: 0 }];
    await touch("touchStart", from);
    await glide(from, [{ x: n[2], y: n[3] }], n[4]);
    await touch("touchEnd", []);
  },
  pinch: async () => {
    const [x, y, radius, scale, ms] = n;
    const from = [{ x: x - radius, y, id: 0 }, { x: x + radius, y, id: 1 }];
    await touch("touchStart", from);
    await glide(from, [{ x: x - radius * scale }, { x: x + radius * scale }].map((p) => ({ ...p, y })), ms);
    await touch("touchEnd", []);
  },
};
if (!commands[command]) throw new Error(`cdp-touch: unknown command ${command}`);
await commands[command]();
socket.close();
