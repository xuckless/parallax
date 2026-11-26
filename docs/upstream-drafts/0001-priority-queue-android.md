# Draft: PR for patch 0001 (export the priority queues on Android)

## For the owner

- **Where:** a pull request on zed-industries/zed, base `main`, from a branch on
  [`xuckless/zed`](https://github.com/xuckless/zed). Patch:
  [`patches/0001-export-priority-queue-android.patch`](../../patches/0001-export-priority-queue-android.patch).
- **Branch:** `gpui-android-priority-queue`, cut from the fork's `parallax` branch onto current Zed main
  (commit `5e9ac685` on `parallax` is this patch):

  ```sh
  git fetch https://github.com/zed-industries/zed main
  git switch -c gpui-android-priority-queue FETCH_HEAD
  git cherry-pick 5e9ac685cce4c07fbd21a3954cd13d6ecffdc31b
  git push https://github.com/xuckless/zed gpui-android-priority-queue
  ```

- **Still applies to Zed main:** yes, cleanly with no offset, checked against main `a3ce8d5e` (2026-11-26).
  Recheck right before opening it.
- **Rewrite the body in your own words.** Zed's CONTRIBUTING.md "AI Policy": no PRs from autonomous agents, and PR
  descriptions should come from you, not a model. Open it yourself, and only after you've read and understood the diff.
- **Before opening:** sign the [Zed CLA](https://zed.dev/cla). CONTRIBUTING.md asks you to build and run Zed locally
  and test manually; for a cfg-only change the Android `cargo check` below is the meaningful test, plus a normal
  build on your desktop OS to show nothing else changed. At most three open PRs at a time.
- **To decide:** Zed has no in-tree Android user of this export, so they may ask why. The answer is out-of-tree
  platforms (`Platform` is a public trait); say so plainly. Whether to send this before or after the
  [BSD question](bsd-licence-question.md): this patch has no Flutter code, so it doesn't depend on the answer.
- Once filed, put the PR link in the patch header (AGENTS.md patch rule 5).

## PR

**Title:** gpui: Export PriorityQueueSender/Receiver on Android

## Summary

GPUI's `queue` module (`PriorityQueueSender`/`PriorityQueueReceiver`) is compiled and re-exported for Windows,
Linux and wasm, where the platform dispatchers use it. Android's `target_os` is `"android"`, not `"linux"`, so it
is left out there, and an out-of-tree Android platform has to copy the module to get the same priority handling as
the Linux and Windows dispatchers.

This adds `target_os = "android"` to the two cfgs. No change on any other target.

```diff
@@ crates/gpui/src/gpui.rs
     target_os = "windows",
     target_os = "linux",
+    target_os = "android",
     target_family = "wasm",
...
-#[cfg(any(target_os = "windows", target_os = "linux", target_family = "wasm"))]
+#[cfg(any(
+    target_os = "windows",
+    target_os = "linux",
+    target_os = "android",
+    target_family = "wasm"
+))]
 pub use queue::{PriorityQueueReceiver, PriorityQueueSender};
```

Context: I'm working on an Android backend for GPUI as a separate crate; its dispatcher runs on the Android main
Looper and uses these queues for the main-thread queue and the background pool.

## Testing

- `cargo check -p gpui --target aarch64-linux-android` builds, and the two types are importable from `gpui`.
- That backend's dispatcher runs on the Android emulator today with a copy of `queue.rs` (GPUI at `f126950f`);
  this change is what lets it drop the copy.
- Desktop builds are unaffected (cfg-only).

## Self-review

- [x] I've reviewed my diff for quality, security, reliability, and performance.
- [x] UI changes follow the [checklist](https://zed.dev/docs/development/ui-checklist). (No UI changes.)
- [ ] Tests cover the new or changed behavior. (Cfg-only re-export; no behaviour to test.)

Release Notes:

- N/A
