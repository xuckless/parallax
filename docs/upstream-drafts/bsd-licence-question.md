# Draft: BSD-3 (Flutter-derived) code in GPUI contributions

## For the owner

- **Where:** a GitHub Discussion on zed-industries/zed, category **Zed GPUI**
  (`https://github.com/zed-industries/zed/discussions/new?category=zed-gpui`). Blank issues are disabled there, and
  [CONTRIBUTING.md](https://github.com/zed-industries/zed/blob/main/CONTRIBUTING.md) asks for a discussion, not an
  issue or PR, before work Zed hasn't confirmed it wants.
- **Rewrite it in your own words before posting.** Zed's AI policy (CONTRIBUTING.md, "AI Policy") says they don't
  accept contributions from autonomous agents and want maintainer communication, PR descriptions included, written
  by you rather than a model. Treat the post below as notes, not text to paste.
- **CLA:** every PR needs the [Zed CLA](https://zed.dev/cla) signed. Read it before posting. The question is whether
  you can sign it for code whose copyright partly belongs to The Flutter Authors; that is part of what we are asking.
- **Context:** Zed's [`script/licenses/zed-licenses.toml`](https://github.com/zed-industries/zed/blob/main/script/licenses/zed-licenses.toml)
  already accepts BSD-3-Clause for *dependencies*; `crates/gpui` itself is `Apache-2.0`. In-tree BSD code is a
  different question, so this ask stays open (ROADMAP "Open questions").
- **To decide:** whether to name a concrete first patch (e.g. Flutter's velocity-tracker constants or a text-input
  fix) or keep it general. The draft keeps it general.
- Other limits worth knowing: at most three open PRs at a time; non-trivial changes need tests.

## Post

**Title:** Are contributions with code ported from Flutter (BSD-3-Clause) acceptable in GPUI?

Hi! I'm working on Parallax, a set of add-on crates that bring GPUI to Android and touchscreen desktops (it builds on
the `gpui-pre` snapshots and leaves GPUI itself untouched). Along the way we find small gaps in GPUI that we'd like to
fix upstream rather than carry as patches. The first ones are tiny cfg changes, written from scratch.

Some later fixes would come from Flutter, which has spent years on touch, gesture and text-input edge cases on every
platform: for example its velocity-tracker constants or fixes from its iOS text-input plugin. Flutter is
BSD-3-Clause, and GPUI is Apache-2.0.

Before sending any such PR, I'd like to ask:

1. Would you accept code ported or adapted from Flutter into GPUI crates, keeping the BSD-3 notice
   ("Copyright 2014 The Flutter Authors") as a header on the affected file or block, and the license text in the repo?
2. If yes, is there a preferred way to record it (a header comment, a `third_party/` license file, something in
   `script/licenses`)?
3. Does the CLA allow it, given part of the copyright stays with The Flutter Authors?
4. If not, is reimplementing the behaviour from Flutter's tests (using them only as a spec) fine?

Happy to go with whatever works best for you. Thanks!
