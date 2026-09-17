"""
discover.py -- scan the user home directory for Agent skills folders and
report (optionally register) the ones that SKILL Hub doesn't know about.

This is a safe, read-only-by-default companion to the bulk-registration
helper described in
  E:\\Agent知识中心\\04-进化参考\\已阅读\\Claude-Code-Codex\\gui_skills-hub-跨Agent-Skill集中管理.md

It does NOT touch the SKILL Hub database unless you pass --apply.
When --apply is passed, it asks for confirmation per-entry and writes
only CustomToolConfig records (the same shape the "Add custom tool" UI
form produces). It never deletes or overwrites existing rows.

Detected layout (the three shapes every Agent CLI we care about uses):
  ~/.<tool>/skills/                 (Claude Code, Codex, WorkBuddy, ...)
  ~/.<tool>/agent/skills/           (Pi)
  ~/.config/<tool>/skills/          (OpenCode, Goose, Crush, Amp, Kimi)

Usage:
  python discover.py                     # scan + report only
  python discover.py --apply              # scan + ask before each register
  python discover.py --register <name>   # skip the menu, register one tool
                                         # (still asks y/N unless --yes)
  python discover.py --db <path>          # override DB location
"""
from __future__ import annotations

import argparse
import json
import os
import sqlite3
import sys
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Iterable

# -- known SKILL Hub builtin tools (from src-tauri/src/core/tool_adapters/mod.rs)
# Mirrored here so the script is standalone. Keep in sync if SKILL Hub adds
# new builtins -- the easiest way is to grep "ToolId::" in tool_adapters/mod.rs
# and append the lowercase as_key() values below.
BUILTIN_TOOLS: dict[str, str] = {
    # key -> skills_dir (relative to home)
    "claude_code":      ".claude/skills",
    "codex":            ".codex/skills",
    "opencode":         ".config/opencode/skills",
    "codewhale":        ".codewhale/skills",
    "workbuddy":        ".workbuddy/skills",
    "command_code":     ".commandcode/skills",
    "pi":               ".pi/agent/skills",
    "github_copilot":   ".copilot/skills",
    "hermes_agent":     ".hermes/skills",
    "deepseek_harness": ".dsh/skills",
    "openclaw":         ".openclaw/skills",
    "copaw":            ".copaw/skill_pool",
    "cline":            ".agents/skills",
    "codebuddy":        ".codebuddy/skills",
    "augment":          ".augment/skills",
    "continue":         ".continue/skills",
    "kimi_cli":         ".config/agents/skills",
    "amp":              ".config/agents/skills",
    "openclaude":       ".openclaude/skills",
    "openhands":        ".openhands/skills",
    "goose":            ".config/goose/skills",
    "crush":            ".config/crush/skills",
    "junie":            ".junie/skills",
    "kiro_cli":         ".kiro/skills",
    "kode":             ".kode/skills",
    "mcpjam":           ".mcpjam/skills",
    "mistral_vibe":     ".vibe/skills",
    "mux":              ".mux/skills",
    "cursor":           ".cursor/skills",
    "iflow_cli":        ".iflow/skills",
    "qoder":            ".qoder/skills",
    "qoderwork":        ".qoderwork/skills",
    "qwen_code":        ".qwen/skills",
    "trae":             ".trae/skills",
    "trae_cn":          ".trae-cn/skills",
    "zencoder":         ".zencoder/skills",
    "neovate":          ".neovate/skills",
    "pochi":            ".pochi/skills",
    "adal":             ".adal/skills",
    "kilo_code":        ".kilocode/skills",
    "roo_code":         ".roo/skills",
    "gemini_cli":       ".gemini/skills",
    "clawdbot":         ".clawdbot/skills",
    "droid":            ".factory/skills",
    "windsurf":         ".codeium/windsurf/skills",
    "moltbot":          ".moltbot/skills",
    "antigravity":      ".gemini/config/skills",
    "clawdbot":         ".clawdbot/skills",
    "opensquilla":      ".opensquilla/skills",
}

# -- scanner patterns -------------------------------------------------------
# Every Agent CLI we know of either puts the skills folder directly under
# ~/.<tool>/skills, or wraps it in a subfolder. We don't try to be smart
# about *which* subfolder -- we just look for any directory under $HOME
# that ends in `skills/` and whose parent basename is one of:
#
#   1. a top-level hidden directory ~/.xxx/                  (most agents)
#   2. a top-level hidden directory + /agent/skills/         (Pi)
#   3. a directory under ~/.config/xxx/skills/               (OpenCode etc.)
#
# We also exclude SKILL Hub's own dirs so we don't recommend the user
# re-register ~/.skillshub/skills/ as a "custom tool" (it would be
# flagged MANAGED on import).

SKIP_DIRS = {
    ".skillshub",          # central repo -- not a tool target
    ".skillhub",           # legacy config root
    ".agents",             # already covered by cline builtin
    ".commandcode",        # already covered by command_code builtin
    ".claude",             # already covered by claude_code builtin
    ".codex",              # already covered by codex builtin
    ".config",             # not a tool by itself
    ".cargo", ".npm", ".git", ".local", ".cache",
    ".ssh", ".aws", ".azure", ".vscode", ".vscode-insiders",
    ".dotnet", ".nuget", ".cargo", ".rustup", ".stack",
    "node_modules", "AppData", "Application Data",
    "Documents", "Desktop", "Downloads", "Pictures", "Videos", "Music",
    "OneDrive", "Postman", "IntelGraphicsProfiles",
}

# -----------------------------------------------------------------------------

@dataclass
class Found:
    label: str           # suggested label for CustomToolConfig
    skills_dir: str      # absolute path
    parent: str          # the top-level dir name we inferred from
    n_skill_subdirs: int # how many subdirs with SKILL.md are inside

@dataclass
class Report:
    found: list[Found]
    registered_keys: set[str]      # keys already in tool_config_v1
    enabled_keys: set[str]         # keys in installed_tools_v1
    builtin_keys: set[str]         # BUILTIN_TOOLS keys
    disabled_builtin: set[str]     # disabled_builtin_tools list

    @property
    def unknown(self) -> list[Found]:
        # A found item is "unknown" if neither a custom_<key> nor a built-in
        # key matches the label we derived from its parent directory. The
        # registered_keys set always contains the "custom_<x>" form, so we
        # check both the label-as-key AND the label-as-suffix-of-custom-key
        # to handle the case where the label is the same as a custom_<x>
        # key's tail.
        known = self.builtin_keys
        for k in self.registered_keys:
            known.add(k)
            if k.startswith("custom_"):
                known.add(k[len("custom_"):])
        return [f for f in self.found if self._label_to_key(f.label) not in known]

    @staticmethod
    def _label_to_key(label: str) -> str:
        # If the label already has a canonical mapping (e.g. copilot -> github_copilot)
        # we leave it as-is. Otherwise we sanitize: lowercase, hyphens -> underscores.
        if label in LABEL_ALIASES.values():
            return label
        return label.lower().replace("-", "_")

# -----------------------------------------------------------------------------

# Some tools share a skills directory layout with another tool, or SKILL Hub
# uses a different key than the directory name. When generating CustomToolConfig
# entries, we use the LAST segment of the parent path as the natural label,
# but these aliases map the raw label to the canonical SKILL Hub key.
LABEL_ALIASES: dict[str, str] = {
    # ~/.copilot/skills  ->  built-in key is github_copilot
    "copilot":        "github_copilot",
    # ~/.kilocode/skills -> key is kilo_code
    "kilocode":       "kilo_code",
    # ~/.roo/skills      -> key is roo_code
    "roo":            "roo_code",
    # ~/.factory/skills  -> key is droid
    "factory":        "droid",
    # ~/.codeium/windsurf/skills -> key is windsurf
    "windsurf":       "windsurf",  # already matches; kept for completeness
    # ~/.vibe/skills     -> key is mistral_vibe
    "vibe":           "mistral_vibe",
    # ~/.clawdbot/skills -> key is clawdbot (matches)
    # ~/.moltbot/skills  -> key is moltbot (matches)
}


def _is_valid_key(key: str) -> bool:
    """Mirror is_valid_custom_tool_key in src-tauri/src/core/tool_adapters/mod.rs."""
    if not key or not (key[0].islower() and key[0].isalpha()):
        return False
    return all(c.islower() or c.isdigit() or c in "_-" for c in key)


def scan_home(home: Path) -> list[Found]:
    """Find all top-level hidden dirs / .config subdirs that contain a skills/ folder."""
    out: list[Found] = []
    seen: set[str] = set()

    # 1) Top-level ~/.xxx/skills/ or ~/.xxx/agent/skills/
    try:
        for child in home.iterdir():
            if not child.is_dir():
                continue
            name = child.name
            if name in SKIP_DIRS or not name.startswith("."):
                continue
            for cand in [child / "skills", child / "agent" / "skills"]:
                if cand.is_dir() and str(cand) not in seen:
                    out.append(_make_found(cand, parent=name, home=home))
                    seen.add(str(cand))
    except (PermissionError, OSError) as e:
        print(f"  warn: {home}: {e}", file=sys.stderr)

    # 2) ~/.config/xxx/skills/  (OpenCode, Goose, Crush, Amp, Kimi, Antigravity)
    config = home / ".config"
    if config.is_dir():
        try:
            for child in config.iterdir():
                if not child.is_dir() or child.name in SKIP_DIRS:
                    continue
                cand = child / "skills"
                if cand.is_dir() and str(cand) not in seen:
                    out.append(_make_found(cand, parent=f".config/{child.name}", home=home))
                    seen.add(str(cand))
        except (PermissionError, OSError):
            pass
    return out


def _make_found(skills_dir: Path, parent: str, home: Path) -> Found:
    # Label is the LAST segment of the parent path.
    #   ~/.config/opencode/skills  ->  parent = ".config/opencode"  -> label = "opencode"
    #   ~/.copilot/skills          ->  parent = ".copilot"          -> label = "copilot"
    #   ~/.pi/agent/skills          ->  parent = ".pi"               -> label = "pi"
    raw = parent.rstrip("/").split("/")[-1].lstrip(".")
    label = LABEL_ALIASES.get(raw, raw)
    n = 0
    try:
        for child in skills_dir.iterdir():
            if child.is_dir() and (child / "SKILL.md").is_file():
                n += 1
    except (PermissionError, OSError):
        pass
    return Found(
        label=label,
        skills_dir=str(skills_dir),
        parent=parent,
        n_skill_subdirs=n,
    )


def read_db(db_path: Path) -> Report:
    found = []  # we'll fill in by scanning
    if not db_path.exists():
        raise SystemExit(f"DB not found: {db_path}\n"
                         f"Pass --db <path> to point at skills_hub.db, or open SKILL Hub once to create it.")
    c = sqlite3.connect(db_path); c.row_factory = sqlite3.Row
    settings = {r["key"]: r["value"] for r in c.execute("SELECT key, value FROM settings")}

    tc_raw = settings.get("tool_config_v1")
    if not tc_raw:
        registered_keys = set()
        disabled_builtin = set()
    else:
        tc = json.loads(tc_raw)
        registered_keys = {t["key"] for t in tc.get("custom_tools", [])}
        disabled_builtin = set(tc.get("disabled_builtin_tools", []))

    inst_raw = settings.get("installed_tools_v1", "[]")
    enabled_keys = set(json.loads(inst_raw))

    builtin_keys = set(BUILTIN_TOOLS.keys())
    c.close()
    return Report(
        found=found,
        registered_keys=registered_keys,
        enabled_keys=enabled_keys,
        builtin_keys=builtin_keys,
        disabled_builtin=disabled_builtin,
    )


# -- registration -----------------------------------------------------------

def apply_to_db(db_path: Path, found: list[Found], yes: bool = False) -> None:
    """Add CustomToolConfig entries for each `found` item, with confirmation."""
    if not found:
        print("nothing to register")
        return
    # backup
    backup = db_path.with_suffix(db_path.suffix + f".bak-discover-{datetime.now().strftime('%Y%m%d-%H%M%S')}")
    import shutil
    shutil.copy2(db_path, backup)
    print(f"backup: {backup}")

    c = sqlite3.connect(db_path); c.row_factory = sqlite3.Row
    settings = {r["key"]: r["value"] for r in c.execute("SELECT key, value FROM settings")}
    tc = json.loads(settings.get("tool_config_v1", "{}"))
    if "custom_tools" not in tc:
        tc["custom_tools"] = []
    inst = json.loads(settings.get("installed_tools_v1", "[]"))

    seen_keys = {t["key"] for t in tc["custom_tools"]} | set(BUILTIN_TOOLS.keys())

    for f in found:
        key = f"custom_{Report._label_to_key(f.label)}"
        if not _is_valid_key(key):
            print(f"  skip {f.label!r}: key {key!r} would fail is_valid_custom_tool_key")
            continue
        if key in seen_keys:
            print(f"  skip {f.label}: already known as {key}")
            continue
        if key in inst:
            print(f"  skip {f.label}: {key} already in installed_tools_v1")
            continue
        if not yes:
            ans = input(f"  register {f.label:<18} key={key:<22} dir={f.skills_dir}? [y/N] ").strip().lower()
            if ans not in ("y", "yes"):
                print(f"  skip {f.label} (declined)")
                continue
        entry = {
            "key": key,
            "label": f.label,
            "avatar": None,
            "skills_dir": f.skills_dir,
            "project_skills_dir": None,
            "sync_mode": "auto",
            "enabled": True,
        }
        tc["custom_tools"].append(entry)
        inst.append(key)
        seen_keys.add(key)
        print(f"  + {f.label} -> {key}  ({f.skills_dir}) [{f.n_skill_subdirs} skill(s) inside]")

    c.execute("UPDATE settings SET value=? WHERE key='tool_config_v1'",
              (json.dumps(tc, ensure_ascii=False, separators=(",", ":")),))
    c.execute("UPDATE settings SET value=? WHERE key='installed_tools_v1'",
              (json.dumps(inst, ensure_ascii=False, separators=(",", ":")),))
    c.commit()
    c.close()
    print("done. restart SKILL Hub to load the new entries.")


# -- reporting --------------------------------------------------------------

def print_report(report: Report, found: list[Found]) -> None:
    print(f"scanned home: {Path.home()}")
    print(f"DB:           (will be opened on --apply)")
    print()
    print(f"== Built-in tools ({len(report.builtin_keys)} known) ==")
    print(f"  enabled:    {sorted(report.enabled_keys & report.builtin_keys) or '(none)'}")
    print(f"  disabled:   {sorted(report.disabled_builtin) or '(none)'}")
    print(f"  unknown to SKILL Hub: "
          f"{sorted(report.builtin_keys - report.enabled_keys - report.disabled_builtin) or '(none)'}")
    print()

    print(f"== Custom tools already registered ({len(report.registered_keys)}) ==")
    if report.registered_keys:
        for k in sorted(report.registered_keys):
            mark = "✓ installed" if k in report.enabled_keys else "  registered, not in installed list"
            print(f"  {k:<32} {mark}")
    else:
        print("  (none)")
    print()

    print(f"== Skills folders found under home ({len(found)}) ==")
    for f in sorted(found, key=lambda x: x.skills_dir):
        registered_key = Report._label_to_key(f.label)
        if registered_key in report.builtin_keys:
            status = "[built-in]"
        elif f"custom_{registered_key}" in report.registered_keys:
            status = "[registered as custom]"
        else:
            status = "[NOT registered]"
        n_marker = f"({f.n_skill_subdirs} skills)" if f.n_skill_subdirs else "(empty)"
        print(f"  {f.label:<18} {status:<22} {n_marker:<14} {f.skills_dir}")

    unknown = report.unknown
    print()
    if unknown:
        print(f"== {len(unknown)} candidate(s) for new custom tool registration ==")
        for f in unknown:
            print(f"  {f.label}    -> custom_{Report._label_to_key(f.label)}    {f.skills_dir}")
        print()
        print("Run with --apply to register them (with per-entry confirmation).")
        print("Or: --register <label> for one specific tool.")
    else:
        print("== no new candidates; everything found is already known to SKILL Hub ==")


# -- CLI --------------------------------------------------------------------

def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--db", default=r"C:\Users\xiaoyidemm\AppData\Roaming\com.qufei1993.skillshub\skills_hub.db",
                   help="path to skills_hub.db (default: %(default)s)")
    p.add_argument("--apply", action="store_true",
                   help="register the unknown candidates into the DB (asks y/N per entry)")
    p.add_argument("--yes", action="store_true",
                   help="with --apply, skip the per-entry confirmation")
    p.add_argument("--register", metavar="LABEL",
                   help="register a single candidate by its label (still asks y/N unless --yes)")
    p.add_argument("--home", default=None,
                   help="override $HOME (default: the current user's home)")
    args = p.parse_args()

    home = Path(args.home) if args.home else Path.home()
    db_path = Path(args.db)

    # Phase 1: scan (always read-only)
    found = scan_home(home)
    # Phase 2: read DB
    if not db_path.exists():
        print(f"DB not found: {db_path}", file=sys.stderr)
        print("  Pass --db <path> to override, or run SKILL Hub once to create the default DB.", file=sys.stderr)
        return 2
    report = read_db(db_path)
    report.found = found

    if args.register:
        # single-shot mode
        for f in found:
            if f.label == args.register or Report._label_to_key(f.label) == args.register:
                apply_to_db(db_path, [f], yes=args.yes)
                return 0
        print(f"label {args.register!r} not found among {len(found)} candidate skills folders", file=sys.stderr)
        return 1

    # Phase 3: print report
    print_report(report, found)

    # Phase 4: optionally apply
    if args.apply:
        print()
        apply_to_db(db_path, report.unknown, yes=args.yes)
    return 0


if __name__ == "__main__":
    sys.exit(main())
