# Code ported from GPUI Kit

GPUI Kit ([longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)) is Apache-2.0 (see `LICENSE` here, its
`LICENSE-APACHE`). Its port is tooling only: no published crate carries it, so no crate ships the licence. Every file
that ports or adapts GPUI Kit code gets one row, with what it left out (AGENTS.md porting rules 3, 4 and 10 apply as for
Flutter). Pinned commit: `85c05bf6d977d38383e5b2a919a0abb1429f39d0`, the one that cut gpui-pre 0.3.14.

Sync rule: when a new gpui-pre snapshot ships, diff `script/bump-gpui.ts` between the pinned commit and the one that
cut it, and port the changes to staging, manifests and source adaptations.

| GPUI Kit path | Commit | Parallax path | Kind (port / oracle) | Dropped parts |
|---|---|---|---|---|
| `script/bump-gpui.ts` | `85c05bf` | `scripts/vendor-zed.py` | port (TypeScript → Python) | everything but its staging step (crate renames, workspace dependencies inlined, dev-dependencies dropped, pruning, gpui-pre's source adaptations, `cargo package`): the version bump and changelog, git tagging, publishing to crates.io, the bun/node runtime. Added: unpacking the `.crate` files into `cargo vendor`'s layout for `sync-upstream.sh --zed` |
