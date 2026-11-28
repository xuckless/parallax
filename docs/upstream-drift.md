# Weekly upstream drift report

[upstream-drift.yml](../.github/workflows/upstream-drift.yml) applies every patch to Zed `main` once a week (Monday
06:17 UTC) and on any push that changes its scripts, so drift shows while it is small instead of at the two-weekly
sync ([upstream.md](upstream.md)). It came out of the [rehearsal](upstream-rehearsal.md): five days of Zed cost nine
regenerated patches.

**Report only.** It pushes, publishes and commits nothing; the workspace it changes is the runner's checkout. Its
concurrency group is its own, so it never cancels a `ci.yml` run or is cancelled by one.

## What it runs

1. [`sync-upstream.sh --zed main`](vendor-zed.md): vendors Zed main into `upstream/` through the gpui-pre fallback.
   This step fails only when vendoring fails; its own patch check is recorded, not enforced.
2. [`rebase-patches.sh report`](../scripts/rebase-patches.sh): every patch in order, classified clean / offset /
   fuzz (regenerated) / conflict. In `report` mode a conflict is skipped and the run goes on, so one run lists every
   conflict; later patches that need a skipped one may conflict too. **Fails** when any patch conflicts.
3. [`sync-fork.sh --no-push`](zed-fork.md): builds the fork branch on Zed main from the rebased patches, manifest
   changes included. Fails at the first patch that does not apply.
4. [`patch-drift.sh OLD NEW --diff`](../scripts/patch-drift.sh): per patch, the files Zed changed under it since our
   `upstream/UPSTREAM` commit, and Zed's diff of each.

The job summary shows each step's output; the `drift-report` artifact holds the full logs, Zed's diffs and the
rebased `patches/`.

## Reading it

- **Red job, conflicts listed**: the intended signal. Nothing is broken on `main`; the next sync has that work to do.
  Fix it at the sync (or earlier on a branch) with `rebase-patches.sh` and `regen`, as in [upstream.md](upstream.md).
- **Green but drift listed**: patches still apply; read Zed's diff of the files under each before the sync (the
  rehearsal's 0042 `WM_USER` clash and 0046 drift were of this kind).
- **Vendoring failed**: a `vendor-zed.py` rule no longer matches Zed ([vendor-zed.md](vendor-zed.md#when-it-fails)).

Run it by hand, or against another Zed commit:

```sh
gh workflow run upstream-drift.yml --ref <branch>                 # Zed main
gh workflow run upstream-drift.yml --ref <branch> -f zed_ref=<sha>
```
