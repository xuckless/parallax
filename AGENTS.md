# AGENTS.md

Rules for any agent (or human) working in this repo. Read this first, then `ROADMAP.md`.

## What Parallax is

Parallax takes GPUI to Android, touchscreen desktops and the web, Flutter-style: the same GPUI app runs on
desktop, iOS, Android and browsers. It ships as add-on crates under Apache 2.0, on top of an untouched GPUI.

Upstream GPUI already covers a lot. Parallax fills only the gaps:

| Area | Upstream GPUI has | Parallax adds |
|---|---|---|
| iOS | `gpui_ios` (UIKit, Metal, CoreText, text input, safe area) | Nothing new. Patches only for gaps found. |
| Touch + gestures | `TouchEvent`, `gestures.rs` (tap→mouse, long press, pan, pinch, drag), `ScrollPhysics` | Nothing new. Uses upstream. |
| Web | `gpui_web` (WebGPU/WebGL2, touch, IME mirror) | `parallax-web`: semantics DOM for screen readers, other gaps |
| Android | Nothing | `parallax-android`: whole backend, built on Flutter's ported Android embedding |
| Desktop touch | Nothing on Windows, Linux, macOS | Patches feeding `TouchEvent` from each OS |
| Accessibility | AccessKit tree in core, Windows adapter | Android adapter, web semantics DOM, gaps elsewhere |
| Tooling | Nothing for mobile builds | `cargo-parallax` CLI |

Re-check this table against upstream at every sync. Upstream moves fast; never assume a gap is still a gap.

## Flutter is the second source of truth

Flutter has spent a decade fixing touch, text input, IME and accessibility bugs on every platform. Parallax
does not reinvent that. It ports Flutter's work, credits it, and keeps tracking it.

| Flutter asset | How Parallax uses it |
|---|---|
| Android embedding, self-contained files (Java): `InputConnectionAdaptor`, `ListenableEditingState`, `TextEditingDelta`, `FlutterTextUtils`, `AndroidTouchProcessor`, `MotionEventTracker`, `KeyboardManager` (plus the input parts of `TextInputPlugin`) | **Ported as Java**, near-verbatim, in `crates/android/java/`. `MethodChannel` calls become JNI calls into Rust. |
| Android embedding, engine-bound files: `FlutterActivity`, `FlutterView`, `FlutterSurfaceView` | **Reference only.** Parallax writes its own small Activity and View, reading Flutter's for lifecycle, surface and inset handling. |
| iOS text input (`FlutterTextInputPlugin.mm` and its tests) | **Test oracle.** Its test cases become table-driven tests against upstream `gpui_ios`; failures get Flutter's fix as a patch, filed upstream. |
| Web text editing (`text_editing.dart`: per-browser strategies) | Test oracle for upstream `gpui_web`; fixes as patches or in `parallax-web`. |
| Framework algorithms (`velocity_tracker.dart`, `lsq_solver.dart`, `scroll_physics.dart`, `friction_simulation.dart`, `arena.dart`) | Test oracle for upstream `gestures.rs` and `ScrollPhysics`. Port a Flutter algorithm only where upstream fails its cases. |
| Platform behaviour (`TargetPlatform` defaults, selection handles, magnifier, text toolbar, haptics, touch-target minimums) | Ported as behaviour plus one target → behaviour table. No Material or Cupertino widgets. |
| Accessibility (`AccessibilityBridge.java`, `SemanticsObject.mm`) | Oracle only. Adapters come from AccessKit (`accesskit_android`, others). |

Rules for ported code:

1. **Upstream GPUI first, Flutter second.** Never build a parallel system next to upstream's. Flutter fixes go in as small patches to upstream and get filed with Zed.
2. **Port, don't paraphrase.** Java stays Java with Flutter's structure, names and comments, so later Flutter fixes can be diffed in. Code translated to Rust keeps Flutter's names, constants and comments (see rule 9).
3. **Header on every ported file:** `Ported from flutter/flutter <path> @ <commit>. Copyright 2014 The Flutter Authors. BSD-3-Clause, see third_party/flutter/LICENSE.`
4. **Ledger:** add a row to `third_party/flutter/PORTED.md` for every ported file or test table.
5. **Licenses:** a crate with ported code declares `license = "Apache-2.0 AND BSD-3-Clause"`. Keep `NOTICE` current.
6. **Ported tests are the spec.** A Flutter test case that encodes a bug fix becomes a named row in a Parallax test table, with the Flutter test name in the row.
7. **Naming:** "ports code from Flutter" is fine. Never use "Flutter" or Google marks in a crate or product name.
8. **Pinned Flutter stable tag:** recorded in `third_party/flutter/PORTED.md`. Track stable release tags, never master. When a new stable tag ships, diff every ledger row and port the fixes.
9. **Caveman Code applies to Rust translations.** Flutter's state becomes explicit enums and its rules become tables, with behaviour identical (its ported tests prove it). Ported Java is exempt; don't restyle it.
10. **Ledger records what was dropped.** A ported file that leaves parts of Flutter's original out (engine, plugin or channel code) lists them in its ledger row.

## Decisions (settled, do not reopen without the owner)

| Topic | Decision |
|---|---|
| Upstream source | `gpui-pre-*` crates from crates.io: weekly snapshots of Zed's GPUI (a few files adapted to build standalone, each marked `// Modified for gpui-pre`), standalone-buildable, iOS and web included. Switch to `zed-industries/gpui` once it holds code. Not `gpui-ce`: it has no iOS and is diverging from Zed. |
| Upstream handling | `upstream/` is a pristine copy (`cargo vendor` of the pinned `gpui-pre-*` version), never hand-edited. `patches/*.patch` are applied by script into `build/gpui-patched/` (generated, git-ignored). The workspace points at that via `[patch]`. Patches are filed upstream against Zed (`zed-industries/zed`), never against the snapshot crates. |
| Sync cadence | Every two weeks: move to the latest `gpui-pre` snapshot. |
| Toolchain | Same `rust-toolchain.toml` channel and edition as upstream. Bump only when upstream bumps. |
| Renderer | iOS: upstream Metal (`gpui_apple`). Android: `gpui_wgpu` on Vulkan, GL fallback. Web: upstream `gpui_web`. Desktop: upstream renderers. No hand-written Vulkan or Metal backends. |
| Web threading | Single-threaded on stable: `gpui_platform` without its `multithreaded` feature on wasm32. Revisit when upstream threads work on stable. |
| Text | Upstream per platform: CoreText on Apple, cosmic-text on Android, Linux and web. |
| Minimum OS | iOS 17. Android API 28 (Android 9). |
| Android glue | Own small Activity and View; Flutter's self-contained touch, keyboard and text-input files ported as Java; Rust over JNI. No GameActivity. |
| Android threading | GPUI runs on the Android main (UI) thread, like `gpui_ios` on UIKit's. IME queries are answered synchronously. No second UI thread. |
| Flutter | Second source of truth: port its platform glue and behaviour, use its tests as the oracle for upstream (see below). Pinned commit in `third_party/flutter/PORTED.md`. |
| App build tooling | `cargo-parallax` CLI driven by a declarative `parallax.toml`. Generates Xcode/Gradle projects. |
| Touch → mouse | On by default (upstream behaviour), explicit opt-out per element. Every pointer event carries its device (mouse, touch, pen) and whether it was synthesized; the rule is a row in the platform behaviour table. |
| Platform feel | `ScrollPhysics` / `GestureTuning` chosen from one visible target→preset table. Apps can override. |
| Desktop touch | First-class and always compiled in. No `touch` cargo feature. Behaviour depends on pointer type at runtime, never on OS. |
| Gesture parity | Checked by replaying recorded touch traces from each device through the gesture and physics test tables; outcomes must match on every platform. |
| iOS text input limit | Fixes go in as patches to `gpui_ios` until they total about 800 lines or Zed rejects 3. Past that, iOS text input moves into its own Parallax crate. |
| Releases | Two rolling lines on crates.io. v0.1.x: 0.1.1 is the working prototype skeleton, then a release whenever `main` holds something new, with no planned last number. v0.2.x: accessibility, same rolling rule. |
| Accessibility | AccessKit adapters plus a web semantics DOM. Ships on the v0.2.x line, not in v0.1.x. |
| Errors | Typed errors, no panics in library code (see below). |
| Tests | Pure-logic unit tests plus example smoke runs (see below). |

## Repo layout

```
upstream/      pristine GPUI copy (vendored gpui-pre-* snapshot), never hand-edited
patches/       numbered small diffs against upstream (0001-….patch), applied in order
build/         generated: upstream + patches. Git-ignored. Never edit.
crates/
  parallax/    facade: re-exports GPUI + Parallax crates
  platform/    parallax-platform: one table maps target → platform
  android/     parallax-android: Platform + PlatformWindow for Android
    java/      Flutter's Android embedding, ported as Java
  web/         parallax-web: gaps on top of gpui_web
  ext/         parallax-ext: extension traits on GPUI elements
  cli/         cargo-parallax: build/run on iOS, Android, web
examples/      one app, built for every target
docs/          how the code works: one lean file per topic, indexed in docs/README.md
scripts/       upstream, patch and Flutter tooling (see docs/)
third_party/
  flutter/     Flutter's LICENSE and PORTED.md (ledger of every ported file + pinned commit)
```

## Upstream and patch rules

1. **Upstream is read-only.** Never edit `upstream/` or `build/`. Change upstream only by adding a patch.
2. **Dependencies point one way.** Parallax depends on GPUI. GPUI never depends on Parallax.
3. **Add, don't modify.** New behaviour goes in a Parallax crate or an extension trait first. A patch needs a written reason why neither works.
4. **Patches are small and gated.** One concern each, about 200 lines max, behind a target `cfg` when the code is platform-specific. No feature flags for behaviour.
5. **Each patch has a header:** what it changes, why, and the upstream PR link once filed.
6. **Upstream what you can.** Every patch is meant to be filed upstream. When upstream merges it, delete it at the next sync.
7. **No editor code.** Never copy Zed's editor or text stack crates (GPL). Only Apache-licensed GPUI crates.
8. **Licenses.** Every vendored dependency keeps its license file. Flag anything not Apache-compatible before it lands. Keep `NOTICE` current.
9. **Naming.** "Parallax, built on GPUI". Never "Zed" or "GPUI" in a crate or product name.
10. **One patch, two layouts.** Patches are written against the vendored `gpui-pre` layout; `scripts/patch-paths.sh` translates them to Zed's `crates/gpui*` layout for filing upstream.

## Coding style: Caveman Code

Code small. Code obvious. Apply the `caveman-code` principles to every change. Priority order, earlier wins:

1. **Declarative.** Behaviour is data (tables, maps, `const` arrays) read by a small engine.
   A `match`/`if` chain on one key becomes a lookup. Example: target → platform, target → `ScrollPhysics`,
   Android key code → GPUI key are each one table.
2. **Explicit state.** Every lifecycle is one enum with data attached to the variant that owns it.
   Never two `bool`s for one thing's mode. Transitions live in one place (`state × event → next`).
   Matches are exhaustive: no `_ =>` arm that hides a new variant.
   Example: Android surface = `enum Surface { None, Ready(ANativeWindow), Lost { since: Instant } }`.
3. **Simplest implementation** that keeps 1 and 2. No speculative options, layers, generics or caching.

Hard rules:

- **No implicit state.** No globals, thread-locals or statics holding mutable state unless the OS forces it
  (e.g. JNI callbacks). When forced, wrap it in one named type and document why at the definition.
- **No hidden behaviour.** No action at a distance: no `Drop` side effects beyond freeing resources,
  no `Deref` to fake inheritance, no `Default` that picks platform behaviour silently, no magic env vars.
  Platform-dependent defaults live in a visible table, and the docs say so.
- **Decide in pure functions, act at the edges.** OS and FFI calls sit in a thin shell. Logic that decides
  what to do takes plain values and returns plain values, so it is testable without a device.
- **Search before you write.** Reuse upstream GPUI types and helpers first, then Parallax ones.
- **No duplication. No single-use helpers.** Inline a helper with one caller.
- **Comments say why, not what.** Every table that drives behaviour gets a one-line comment on how to extend it.
- **Unsafe / FFI:** every `unsafe` block has a `// SAFETY:` comment naming the invariant.

## Errors

- Each library crate has one error enum (`thiserror` is fine). No `anyhow` in library public APIs.
- No `unwrap`, `panic!` or `todo!` in library code (enforced by workspace clippy lints).
- `expect("why this cannot fail")` only for states that are impossible by construction. The message names the invariant.
- Examples, the CLI and tests may use `anyhow` and `unwrap`.

## Tests

- Test pure logic with **table-driven unit tests**: cases as data, one loop, a name per row.
  State machines get one row per `(state, event) → next`, including rejected events.
- Test behaviour through public inputs and outputs. Mock only at I/O edges.
- No throwaway tests. Every test stays in the suite and checks a requirement.
- Platform code is verified by running `examples/` on a simulator, emulator or browser. Before a release, on real devices.
- Never write a test that locks in a bug. Fix the code.

## Docs

`docs/` explains how Parallax's code works. `AGENTS.md` holds the rules, `ROADMAP.md` the plan; docs link to them, never repeat them.

- **One topic per file, lean.** Aim under 80 lines. Split a file before it grows past that.
- **Hyperlink everything.** Link the code it describes (`[apply-patches.sh](../scripts/apply-patches.sh)`), the other
  docs it relies on, and upstream sources. Every doc is listed in [`docs/README.md`](docs/README.md).
- **Say why and how to use it.** What a script or table does, how to run or extend it, what it must never do.
  Don't narrate the code line by line.
- **Docs change with the code.** A change that alters behaviour, a script, a table or a command updates its doc in the same commit.

## Workflow

- Work from `ROADMAP.md`. Tick a box only when its work is merged and verified. A phase ends only when its gate passes.
- Before finishing a change: `cargo fmt --all`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`.
- New targets get a CI row (see ROADMAP Phase 0) in the same change.
- Commits: one concern each, imperative subject line.
- When upstream behaviour surprises you, write it down in `ROADMAP.md` under "Upstream notes" with the upstream commit.
