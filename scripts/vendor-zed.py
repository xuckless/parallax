#!/usr/bin/env python3
# Ported from longbridge/gpui-kit script/bump-gpui.ts @ 85c05bf (the version that cut gpui-pre 0.3.14): its staging
# step (manifests, pruning, source adaptations). Copyright 2024 - 2026 Longbridge. Apache-2.0, see
# third_party/gpui-kit/LICENSE; ledger row in third_party/gpui-kit/PORTED.md.
"""Vendor Zed's crates/gpui* as gpui-pre-* crates, the way gpui-pre builds them, for when gpui-pre stops shipping.

Usage: scripts/vendor-zed.py ZED_CHECKOUT VERSION OUT_DIR
  ZED_CHECKOUT  a full checkout of zed-industries/zed (every workspace member present; it reads the crates listed in
                patch-paths.sh and Zed's root Cargo.toml and Cargo.lock)
  VERSION       version the vendored crates get, e.g. 0.3.15-zed.11e4141
  OUT_DIR       gets one directory per crate (gpui-pre, gpui-pre-apple, ...), as `cargo vendor` lays them out

It stages a standalone workspace in OUT_DIR/../vendor-zed-staging (crates renamed, workspace deps inlined, dev-deps
dropped, gpui-pre's source adaptations applied), runs `cargo package --workspace --no-verify` there and unpacks the
.crate files, so manifests come out crates.io-normalized like gpui-pre's. sync-upstream.sh --zed runs it.
Needs network (crates.io index). Fails loudly when Zed changes something an adaptation relies on: fix the rule here.
"""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PREFIX = "gpui-pre"
ZED_URL = "https://github.com/zed-industries/zed"

# Workspace deps swapped for a crates.io release (bump-gpui.ts DEPENDENCY_OVERRIDES at 85c05bf). To extend: one row;
# an API the release lacks gets a rule in ADAPTATIONS.
OVERRIDES = {
    "proptest": {"package": "proptest", "version": "1"},  # Zed pins git; crates.io has the attr-macro feature
    "wgpu": {"package": "wgpu", "version": "30.0.1"},  # Zed pins git; see adapt_wgpu
}
# Extra dependencies a crate needs for an adaptation (zed crate → name → spec).
EXTRA_DEPS = {"gpui_macros": {"proc-macro-crate": "3"}}
DEP_TABLES = ("dependencies", "build-dependencies")
SOURCE_KEYS = ("path", "package", "version", "git", "rev", "branch", "tag", "registry")
EXACT_VERSION = re.compile(r"^\s*=\s*(\d[^\s,]*)\s*$")


def die(message):
    sys.exit(f"vendor-zed: {message}")


def crate_table():
    """(vendored dir, Zed dir) rows of patch-paths.sh: the one list of crates we vendor."""
    text = (ROOT / "scripts/patch-paths.sh").read_text()
    block = re.search(r'^crates="\n(.*?)^"', text, re.M | re.S)
    return [tuple(line.split()) for line in block.group(1).splitlines() if line.strip()]


def without_source(spec):
    return {k: v for k, v in spec.items() if k not in SOURCE_KEYS}


def relax_exact(spec):
    """`=x.y.z` on an external dep becomes `x.y.z`: a pinned library holds consumers back (bump-gpui.ts)."""
    if isinstance(spec, str):
        m = EXACT_VERSION.match(spec)
        return m.group(1) if m else spec
    if isinstance(spec, dict) and not spec.get("workspace") and "path" not in spec:
        m = EXACT_VERSION.match(spec.get("version", "")) if isinstance(spec.get("version"), str) else None
        if m:
            return {**spec, "version": m.group(1)}
    return spec


def dep_entries(manifest):
    """(table dict, name, spec) of every non-dev dependency, target-specific ones included."""
    tables = [manifest.get(t, {}) for t in DEP_TABLES]
    for cfg in manifest.get("target", {}).values():
        tables += [cfg.get(t, {}) for t in DEP_TABLES]
    return [(table, name, spec) for table in tables for name, spec in list(table.items())]


def feature_target(entry):
    if entry.startswith("dep:"):
        return entry[4:], None
    if "/" in entry:
        dep, sub = entry.split("/", 1)
        return dep.rstrip("?"), sub
    return entry, None


class Workspace:
    def __init__(self, zed):
        self.zed = zed
        self.root = tomllib.loads((zed / "Cargo.toml").read_text())
        self.deps = self.root["workspace"].get("dependencies", {})
        for name, override in OVERRIDES.items():
            if name in self.deps:
                spec = self.deps[name]
                self.deps[name] = {**override, **(without_source(spec) if isinstance(spec, dict) else {})}
        self.crates = []  # (published name, zed dir, manifest)
        for vendored, zed_dir in crate_table():
            path = zed / zed_dir / "Cargo.toml"
            if not path.exists():
                die(f"{zed_dir} is not in this Zed checkout; fix its row in scripts/patch-paths.sh")
            self.crates.append((vendored, zed_dir, tomllib.loads(path.read_text())))
        self.by_dir = {d: (v, m) for v, d, m in self.crates}
        self.dropped = {}  # zed crate name → dropped features

    def effective(self, name, spec):
        if isinstance(spec, str):
            return {"version": spec}
        if spec.get("workspace"):
            if name not in self.deps:
                die(f"`{name}` inherits from the workspace but Zed does not define it")
            base = self.deps[name]
            return {**({"version": base} if isinstance(base, str) else base), "optional": spec.get("optional", False),
                    "workspace": True}
        return dict(spec)

    def path_target(self, crate_dir, spec):
        if "path" not in spec:
            return None
        return os.path.normpath(spec["path"] if spec.get("workspace") else os.path.join(crate_dir, spec["path"]))

    def check_closure(self):
        for _, crate_dir, manifest in self.crates:
            for _, name, spec in dep_entries(manifest):
                target = self.path_target(crate_dir, self.effective(name, spec))
                if target is not None and target not in self.by_dir:
                    die(f"{crate_dir} needs {target} ({name}), which is not vendored: add a row to patch-paths.sh")

    def prune(self):
        """Optional git deps without a crates.io version go, with the features that enable them (bump-gpui.ts)."""
        for _, crate_dir, manifest in self.crates:
            features = manifest.setdefault("features", {})
            explicit_only = {feature_target(e)[0] for es in features.values() for e in es if e.startswith("dep:")}
            removed = []
            for table, name, spec in dep_entries(manifest):
                merged = self.effective(name, spec)
                if "git" not in merged or "version" in merged:
                    continue
                if not merged.get("optional"):
                    die(f"{crate_dir}: `{name}` comes from {merged['git']} without a crates.io version")
                del table[name]
                removed.append(name)
            dropped = [f for f, es in features.items() if any(e.startswith("dep:") and e[4:] in removed for e in es)]
            for feature in dropped:
                del features[feature]
            for feature, entries in features.items():
                features[feature] = [e for e in entries if feature_target(e)[0] not in removed]
            referenced = {feature_target(e)[0] for es in features.values() for e in es}
            for table, name, spec in dep_entries(manifest):
                if self.effective(name, spec).get("optional") and name in explicit_only and name not in referenced:
                    del table[name]
                    removed.append(name)
            if removed or dropped:
                print(f"vendor-zed: {crate_dir}: dropped deps {removed}, features {dropped}")
            self.dropped[manifest["package"]["name"]] = set(dropped)
        # Features naming a feature another crate dropped.
        for _, crate_dir, manifest in self.crates:
            names = {}
            for _, name, spec in dep_entries(manifest):
                target = self.path_target(crate_dir, self.effective(name, spec))
                if target is not None:
                    names[name] = self.by_dir[target][1]["package"]["name"]
            features = manifest.get("features", {})
            for feature, entries in features.items():
                features[feature] = [
                    e for e in entries
                    if not (feature_target(e)[1] and feature_target(e)[1] in self.dropped.get(
                        names.get(feature_target(e)[0], ""), set()))
                ]


def crate_manifest(ws, crate_dir, manifest, version, rev):
    pkg = dict(manifest["package"])
    zed_name, zed_version = pkg["name"], str(pkg.get("version", "0.0.0"))
    license_ = pkg.get("license")
    if isinstance(license_, dict):
        license_ = ws.root["workspace"].get("package", {}).get("license")
    if license_ != "Apache-2.0":
        die(f"{crate_dir}: licence is {license_!r}; only Apache-2.0 crates are vendored (Zed's apps are GPL)")
    description = pkg.get("description") or f"Zed's `{zed_name}` crate"
    pkg.update(name=ws.by_dir[crate_dir][0], version=version, publish=True,
               description=f"{description.rstrip('.')} ({PREFIX} snapshot of zed@{rev[:7]})")
    pkg.setdefault("repository", ZED_URL)
    pkg["metadata"] = {**pkg.get("metadata", {}),
                       PREFIX: {"zed-crate": zed_name, "zed-version": zed_version, "zed-rev": rev}}
    lib = {**manifest.get("lib", {})}
    lib.setdefault("name", zed_name)
    out = {"package": pkg, "lib": lib}
    if "features" in manifest:
        out["features"] = manifest["features"]

    def rewrite(table):
        result = {}
        for name, spec in table.items():
            if isinstance(spec, dict) and not spec.get("workspace") and "path" in spec:
                target = ws.path_target(crate_dir, spec)
                published = ws.by_dir[target][0]
                spec = {"path": os.path.relpath(target, crate_dir), "package": published, "version": f"={version}",
                        **without_source(spec)}
            result[name] = relax_exact(spec)
        return result

    for table in DEP_TABLES:
        if table in manifest:
            out[table] = rewrite(manifest[table])
    for name, spec in EXTRA_DEPS.get(zed_name, {}).items():
        if name in out.setdefault("dependencies", {}):
            die(f"{zed_name} already depends on {name}; update EXTRA_DEPS")
        out["dependencies"][name] = spec
    targets = {cfg: {t: rewrite(v) for t, v in tables.items() if t in DEP_TABLES}
               for cfg, tables in manifest.get("target", {}).items()}
    targets = {cfg: tables for cfg, tables in targets.items() if tables}
    if targets:
        out["target"] = targets
    for key, value in manifest.items():
        if key not in out and key not in ("dev-dependencies", "target", "workspace"):
            out[key] = value
    return out


def workspace_manifest(ws, version):
    zed_ws = ws.root["workspace"]
    deps = {}
    for name, spec in ws.deps.items():
        if isinstance(spec, dict) and "path" in spec:
            target = os.path.normpath(spec["path"])
            if target in ws.by_dir:
                deps[name] = {"path": target, "package": ws.by_dir[target][0], "version": f"={version}",
                              **without_source(spec)}
        else:
            deps[name] = relax_exact(spec)
    return {"workspace": {"resolver": zed_ws.get("resolver", "2"), "members": [d for _, d, _ in ws.crates],
                          "package": {**zed_ws.get("package", {}), "publish": True}, "dependencies": deps,
                          "lints": zed_ws.get("lints", {})}}


# TOML writer (bump-gpui.ts tomlDump): dep specs and lint levels inline, everything else one table per header.
def toml_key(key):
    if re.fullmatch(r"[A-Za-z0-9_-]+", key):
        return key
    return f"'{key}'" if "'" not in key else json.dumps(key)


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, list):
        items = [toml_value(v) for v in value]
        joined = ", ".join(items)
        return "[\n" + "".join(f"    {i},\n" for i in items) + "]" if len(joined) > 80 else f"[{joined}]"
    if isinstance(value, dict):
        inner = ", ".join(f"{toml_key(k)} = {toml_value(v)}" for k, v in value.items())
        return f"{{ {inner} }}" if inner else "{}"
    die(f"cannot write {type(value)} as TOML")


def inline(path, value):
    if not path:
        return False
    if path[-1] in (*DEP_TABLES, "dev-dependencies") or path == ["workspace", "dependencies"] or "workspace" in value:
        return True
    return ("lints" in path or "profile" in path) and path[-1] not in ("lints", "profile")


def toml_dump(document):
    lines = []

    def emit(path, table):
        def kind(value):
            if isinstance(value, dict) and not inline(path, value):
                return "table"
            if isinstance(value, list) and value and all(isinstance(v, dict) for v in value):
                return "array of tables"
            return "value"
        groups = {k: [(key, v) for key, v in table.items() if kind(v) == k] for k in ("value", "table", "array of tables")}
        if path and (groups["value"] or not (groups["table"] or groups["array of tables"])):
            lines.extend(([""] if lines else []) + ["[" + ".".join(map(toml_key, path)) + "]"])
        lines.extend(f"{toml_key(k)} = {toml_value(v)}" for k, v in groups["value"])
        for key, value in groups["table"]:
            emit([*path, key], value)
        for key, items in groups["array of tables"]:
            for item in items:
                lines.extend(["", "[[" + ".".join(map(toml_key, [*path, key])) + "]]"])
                lines.extend(f"{toml_key(k)} = {toml_value(v)}" for k, v in item.items())

    emit([], document)
    return "\n".join(lines) + "\n"


# Source adaptations, each marked with a first-line notice (Apache-2.0 §4(b)). To extend: one function + one row.
def notice(rev, change):
    return f"// Modified for {PREFIX} (snapshot of zed@{rev[:7]}): {change}.\n"


PROC_MACRO_ATTRIBUTE = re.compile(r"^#\[proc_macro(?:_derive\([^\n]*\)|_attribute)?\]$")
PROC_MACRO_SIGNATURE = re.compile(r"^pub fn ([A-Za-z_][A-Za-z0-9_]*)\(([^)]*)\) -> TokenStream \{$")


def adapt_macros(crate, rev):
    """Every proc-macro entry point resolves `gpui::`/`gpui_platform::` through a facade (gpui-kit)."""
    lib = crate / "src/gpui_macros.rs"
    lines, out, count = lib.read_text().split("\n"), [], 0
    i = 0
    while i < len(lines):
        attribute = lines[i]
        if not PROC_MACRO_ATTRIBUTE.match(attribute):
            out.append(attribute)
            i += 1
            continue
        j = i + 1
        while j < len(lines) and lines[j].startswith("#["):
            j += 1
        between, signature = lines[i + 1:j], lines[j] if j < len(lines) else ""
        m = PROC_MACRO_SIGNATURE.match(signature)
        if not m:
            die(f"gpui_macros: unsupported proc-macro signature after {attribute}: {signature}")
        name, params = m.groups()
        args = [re.match(r"^([A-Za-z_][A-Za-z0-9_]*)\s*:", p.strip()).group(1) for p in params.split(",") if p.strip()]
        preceding = []
        for line in reversed(out):
            if not line.startswith("#["):
                break
            preceding.insert(0, line)
        gates = [line for line in preceding + between if line.startswith("#[cfg")]
        out += [attribute, *between, signature,
                f"    gpui_pre_facade_paths::rewrite(__gpui_pre_{name}({', '.join(args)}))", "}", "", *gates,
                f"fn __gpui_pre_{name}({params}) -> TokenStream {{"]
        count += 1
        i = j + 1
    if count == 0:
        die("gpui_macros has no #[proc_macro*] entry points; update adapt_macros")
    lib.write_text(notice(rev, "the proc-macro entry points resolve gpui through a facade")
                   + "mod gpui_pre_facade_paths;\n" + "\n".join(out))
    # The facade module is gpui-kit's, verbatim; take it from the last gpui-pre snapshot.
    facade = (ROOT / "upstream/gpui-pre-macros/src/gpui_pre_facade_paths.rs").read_text().split("\n", 1)[1]
    (crate / "src/gpui_pre_facade_paths.rs").write_text(notice(rev, "this module is added by the script") + facade)


def adapt_actions(crate, rev):
    """`actions!` derives `$crate::Action`, so it works through a re-export (gpui-kit)."""
    path = crate / "src/action.rs"
    source, header = path.read_text(), "macro_rules! actions {"
    start = source.find(header)
    if start < 0:
        die("gpui src/action.rs no longer defines `macro_rules! actions`")
    depth, end = 0, -1
    for k in range(start + len(header) - 1, len(source)):
        depth += {"{": 1, "}": -1}.get(source[k], 0)
        if source[k] == "}" and depth == 0:
            end = k + 1
            break
    body = source[start:end]
    if "gpui::Action)]" not in body:
        die("gpui actions! no longer derives `gpui::Action`; update adapt_actions")
    path.write_text(notice(rev, "the `actions!` derive paths are crate-relative") + source[:start]
                    + body.replace("gpui::Action)]", "$crate::Action)]") + source[end:])


def adapt_apple(crate, rev, gpui):
    """build.rs reads gpui sources at ../gpui; copy exactly those into vendor/gpui (gpui-kit)."""
    build = crate / "build.rs"
    text = build.read_text()
    if '.join("../gpui")' not in text:
        die('gpui_apple/build.rs no longer finds gpui at `../gpui`; update adapt_apple')
    sources = re.findall(r'gpui_dir\.join\("([^"]+)"\)', text)
    if not sources:
        die("gpui_apple/build.rs lists no gpui_dir.join(...) sources; update adapt_apple")
    for rel in sources:
        (crate / "vendor/gpui" / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy(gpui / rel, crate / "vendor/gpui" / rel)
    build.write_text(notice(rev, "the gpui sources it reads are vendored under `vendor/gpui`")
                     + text.replace('.join("../gpui")', '.join("vendor/gpui")'))


def adapt_wgpu(crate, rev):
    """Zed's git wgpu APIs → crates.io wgpu 30.0.1 (gpui-kit)."""
    for name in ("wgpu_context.rs", "wgpu_atlas.rs", "wgpu_renderer.rs"):
        path = crate / "src" / name
        source = path.read_text()
        out = re.sub(r"^[ \t]*default_queue: wgpu::QueueDescriptor::default\(\),\r?\n", "", source, flags=re.M)
        if "default_queue:" in out or "wgpu::QueueDescriptor" in out:
            die("wgpu queue configuration changed; update adapt_wgpu")
        out = out.replace(".has_srgb_suffix()", ".is_srgb()")
        if out != source:
            path.write_text(notice(rev, "adapt device queues and sRGB detection to wgpu 30.0.1") + out)


# Zed crate → adaptation (crate dir, rev, staging root).
ADAPTATIONS = {
    "gpui_macros": lambda c, rev, s: adapt_macros(c, rev),
    "gpui": lambda c, rev, s: adapt_actions(c, rev),
    "gpui_apple": lambda c, rev, s: adapt_apple(c, rev, s / "crates/gpui"),
    "gpui_wgpu": lambda c, rev, s: adapt_wgpu(c, rev),
}


def copy_crate(src, dst):
    def ignore(directory, names):
        return [n for n in names if n in ("target", ".git") or not os.path.exists(os.path.join(directory, n))]
    shutil.copytree(src, dst, symlinks=False, ignore=ignore)


def checksum(directory, crate_file):
    files = {str(p.relative_to(directory)): hashlib.sha256(p.read_bytes()).hexdigest()
             for p in sorted(directory.rglob("*")) if p.is_file()}
    package = hashlib.sha256(crate_file.read_bytes()).hexdigest()
    (directory / ".cargo-checksum.json").write_text(json.dumps({"files": files, "package": package},
                                                               separators=(",", ":")))


def main():
    if len(sys.argv) != 4:
        die("usage: scripts/vendor-zed.py ZED_CHECKOUT VERSION OUT_DIR")
    zed, version, out = Path(sys.argv[1]).resolve(), sys.argv[2], Path(sys.argv[3]).resolve()
    rev = subprocess.run(["git", "-C", zed, "rev-parse", "HEAD"], check=True, capture_output=True,
                         text=True).stdout.strip()
    ws = Workspace(zed)
    ws.check_closure()
    ws.prune()

    staging = out.parent / "vendor-zed-staging"
    shutil.rmtree(staging, ignore_errors=True)
    staging.mkdir(parents=True)
    # Parents first: a nested crate (refineable/derive_refineable) is copied with its parent; cargo packages each.
    for crate_dir in sorted(d for _, d, _ in ws.crates):
        if not (staging / crate_dir).exists():
            copy_crate(zed / crate_dir, staging / crate_dir)
    for _, crate_dir, manifest in ws.crates:
        (staging / crate_dir / "Cargo.toml").write_text(toml_dump(crate_manifest(ws, crate_dir, manifest, version, rev)))
        if not (staging / crate_dir / "LICENSE-APACHE").exists():
            shutil.copy(zed / "LICENSE-APACHE", staging / crate_dir / "LICENSE-APACHE")
        adapt = ADAPTATIONS.get(manifest["package"]["name"])
        if adapt:
            adapt(staging / crate_dir, rev, staging)
    (staging / "Cargo.toml").write_text(toml_dump(workspace_manifest(ws, version)))
    shutil.copy(zed / "Cargo.lock", staging / "Cargo.lock")
    shutil.copy(zed / "rust-toolchain.toml", staging / "rust-toolchain.toml")

    subprocess.run(["cargo", "package", "--workspace", "--no-verify", "--allow-dirty", "--quiet"], cwd=staging,
                   check=True)
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    for published, _, _ in ws.crates:
        crate_file = staging / "target/package" / f"{published}-{version}.crate"
        with tarfile.open(crate_file) as tar:
            tar.extractall(out, filter="data")
        (out / f"{published}-{version}").rename(out / published)
        checksum(out / published, crate_file)
    print(f"vendor-zed: {len(ws.crates)} crates from zed@{rev[:7]} as {version} in {out}")


if __name__ == "__main__":
    main()
