# cargo parallax: web

`cargo parallax run web [--release] [--port <n>] [--host <address>] [--open]` builds the app crate in the current
directory for the browser and serves it; `cargo parallax build web [--release]` only writes the site. Source:
[web.rs](../crates/cli/src/web.rs), the `[web]` table in [config.rs](../crates/cli/src/config.rs),
[templates/web/](../crates/cli/templates/web/). Shared keys and install: [cli.md](cli.md).

## parallax.toml `[web]`

Defaults are one table, `WEB_DEFAULTS` in [config.rs](../crates/cli/src/config.rs). Unknown keys are errors.

| Key | Default | Meaning |
|---|---|---|
| `web.title` | `app.name` | `<title>`, `apple-mobile-web-app-title` and the manifest's `name` |
| `web.background` | `#ffffff` | `#rgb` or `#rrggbb`: the page behind GPUI's canvas until its first frame, `theme-color`, the manifest's colours |
| `web.index` | the template | The app's own host page, relative to `parallax.toml`, in place of [index.html](../crates/cli/templates/web/index.html). Its `{{title}}`, `{{background}}`, `{{lib}}`, `{{icon_links}}` are filled too; it must import `./pkg/<lib>.js` |
| `web.assets` | `[]` | Files copied next to `index.html` under their file name (a name cargo parallax writes is an error) |

`app.icon` (an SVG) is copied as `favicon.svg`, linked as the favicon and listed as the manifest's icon (`sizes:
any`). [examples/native-view](../examples/native-view/parallax.toml) uses `index` and `assets`; hello the template.

## What `build web` does

1. `cargo metadata`: the app package, its `cdylib` name, the target directory, and the `wasm-bindgen` crate's
   version in `Cargo.lock`. Several versions there is an error.
2. `wasm-bindgen --version` must print that version, else it stops before the build with the fix:
   `cargo install wasm-bindgen-cli --version <X>`. Nothing is installed for you.
3. Reads `web.index`, `web.assets` and `app.icon` (missing files fail here, before the build).
4. `cargo build --lib --target wasm32-unknown-unknown [--release]` (`rustup target add wasm32-unknown-unknown`
   if missing; the repo's toolchain lists it).
5. Deletes and rewrites `<target dir>/parallax/web/<package>/`, the deployable site: `wasm-bindgen --target web
   --no-typescript` into `pkg/` (`--debug` without `--release`, as wasm-pack's dev profile), then `index.html`,
   `manifest.json`, `favicon.svg` and the assets.
6. `--release` only: `wasm-opt -O` with rustc's default wasm features (`WASM_OPT_FEATURES`) when binaryen's
   `wasm-opt` is on PATH; without it the build says so and stays unoptimised.

Upload that directory to any static host; it needs `application/wasm` for `.wasm` (most hosts do it).

## The host page

[index.html](../crates/cli/templates/web/index.html) is ported from Flutter's
`packages/flutter_tools/templates/app/web/index.html.tmpl` @ 3.50.1 and [manifest.json](../crates/cli/templates/web/manifest.json)
from its `manifest.json.tmpl` (ledger: [PORTED.md](../third_party/flutter/PORTED.md)). Kept: the iOS meta tags,
`theme-color`, the manifest. Changed: `viewport-fit=cover` is on (Flutter leaves it commented out), since gpui_web
reads `env(safe-area-inset-*)` into the window's safe area; `user-scalable=0` sends pinches to the app. The loader is a
module script importing wasm-bindgen's `pkg/<lib>.js`; gpui_web appends its own full-page canvas to `<body>`.

## What `run web` adds

After `build web`, a small built-in server ([web.rs](../crates/cli/src/web.rs) `serve`, std only, one thread per
connection) serves the site at `http://127.0.0.1:8080/` and prints the URL and each request. `--port` picks the port;
`--host 0.0.0.0` (or a LAN address) exposes it to phones on the network; `--open` opens the system browser
(`OPENERS`: `xdg-open`, `open`, `cmd /C start`). Rules: `/` and `…/` are `index.html`, the query is ignored, `..`,
`.`, empty segments and backslashes are refused (404), only `GET` and `HEAD`; types come from `MIME_TYPES`;
`Cache-Control: no-cache`, so a rebuild shows on reload. It is a development server: no TLS, no compression. Ctrl-C
stops it.

## Checks

Table tests in web.rs (flags, routes, MIME types, URLs, site files and templates, wasm-bindgen versions) and
config.rs (`[web]` rows). In a browser: [web-checks.mjs](../scripts/web-checks.mjs) against the served page
([examples-run.md](examples-run.md#web), [native-view-web-checks.md](native-view-web-checks.md)).
