"""Run unchanged issue-208 command bodies against synthetic, temporary data."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def run(args, cwd, env, log):
    with log.open("w", encoding="utf-8") as stream:
        process = subprocess.Popen(args, cwd=cwd, env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
        for line in process.stdout:
            stream.write(line)
            print(line, end="", flush=True)
        return process.wait()


def recycle_snapshot(root):
    if os.name != "nt":
        return {"available": False, "reason": "Windows only"}
    script = r'''
$ErrorActionPreference = 'Stop'
$shell = New-Object -ComObject Shell.Application
$bin = $shell.Namespace(10)
if ($null -eq $bin) { throw 'Windows recycle-bin namespace is unavailable' }
$items = @(foreach ($item in $bin.Items()) {
    $from = [string]$item.ExtendedProperty('System.Recycle.DeletedFrom')
    if ($from.StartsWith($env:ISSUE208_REPRO_ROOT, [StringComparison]::OrdinalIgnoreCase)) {
        [ordered]@{ name=$item.Name; deleted_from=$from; recycle_path=$item.Path }
    }
})
ConvertTo-Json -InputObject $items -Depth 4 -Compress
'''
    env = dict(os.environ, ISSUE208_REPRO_ROOT=str(root))
    result = subprocess.run(["pwsh", "-NoProfile", "-NonInteractive", "-Command", script],
                            env=env, capture_output=True, text=True, encoding="utf-8")
    if result.returncode:
        raise RuntimeError("Cannot inspect Windows recycle bin: " + result.stderr)
    return {"available": True, "entries": json.loads(result.stdout.lstrip("\ufeff"))}


def nt_open_directory(path):
    from ctypes import wintypes
    class UnicodeString(ctypes.Structure):
        _fields_ = [("length", wintypes.USHORT), ("maximum_length", wintypes.USHORT), ("buffer", wintypes.LPWSTR)]
    class ObjectAttributes(ctypes.Structure):
        _fields_ = [("length", wintypes.ULONG), ("root", wintypes.HANDLE),
                    ("name", ctypes.POINTER(UnicodeString)), ("attributes", wintypes.ULONG),
                    ("security", ctypes.c_void_p), ("quality", ctypes.c_void_p)]
    class IoStatus(ctypes.Structure):
        _fields_ = [("status", ctypes.c_void_p), ("information", ctypes.c_size_t)]
    raw = str(path)
    if raw.startswith("\\\\?\\"):
        raw = raw[4:]
    buffer = ctypes.create_unicode_buffer("\\??\\" + raw)
    name = UnicodeString(len(buffer.value.encode("utf-16-le")), len(buffer.value.encode("utf-16-le")) + 2, ctypes.cast(buffer, wintypes.LPWSTR))
    attributes = ObjectAttributes(ctypes.sizeof(ObjectAttributes), None, ctypes.pointer(name), 0x40, None, None)
    status, handle = IoStatus(), wintypes.HANDLE()
    function = ctypes.WinDLL("ntdll").NtOpenFile
    function.argtypes = [ctypes.POINTER(wintypes.HANDLE), wintypes.ULONG,
                         ctypes.POINTER(ObjectAttributes), ctypes.POINTER(IoStatus), wintypes.ULONG, wintypes.ULONG]
    function.restype = wintypes.LONG
    code = function(ctypes.byref(handle), 0x100081, ctypes.byref(attributes), ctypes.byref(status), 7, 0x21) & 0xFFFFFFFF
    if code == 0:
        close = ctypes.WinDLL("kernel32").CloseHandle
        close.argtypes = [wintypes.HANDLE]
        close.restype = wintypes.BOOL
        assert close(handle)
    return {"path": str(path), "ntstatus": f"0x{code:08X}", "delete_pending": code == 0xC0000056,
            "access_denied": code == 0xC0000022, "sharing_violation": code == 0xC0000043}


def ntfs_observation(root):
    if os.name != "nt":
        return {"available": False, "reason": "Windows only"}
    from ctypes import wintypes
    filesystem = ctypes.create_unicode_buffer(128)
    function = ctypes.WinDLL("kernel32", use_last_error=True).GetVolumeInformationW
    function.argtypes = [wintypes.LPCWSTR, wintypes.LPWSTR, wintypes.DWORD,
                         ctypes.POINTER(wintypes.DWORD), ctypes.POINTER(wintypes.DWORD),
                         ctypes.POINTER(wintypes.DWORD), wintypes.LPWSTR, wintypes.DWORD]
    function.restype = wintypes.BOOL
    assert function(Path(root).anchor, None, 0, None, None, None, filesystem, len(filesystem)), ctypes.get_last_error()
    assert filesystem.value == "NTFS", f"Expected NTFS, found {filesystem.value}"
    statuses = []
    for manifest in (root / "reports").glob("*/fixture.json"):
        fixture = json.loads(manifest.read_text(encoding="utf-8"))
        for directory in [fixture["central"], fixture["source"], *fixture["targets"]]:
            directory = Path(directory)
            assert directory.is_relative_to(root)
            statuses.append(nt_open_directory(directory))
            if directory.is_dir():
                for child in directory.iterdir():
                    statuses.append(nt_open_directory(child))
    return {"available": True, "filesystem": filesystem.value, "directories": statuses,
            "all_opened": bool(statuses) and all(item["ntstatus"] == "0x00000000" for item in statuses)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--checkout", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--fixed", choices=["true", "false"], required=True)
    args = parser.parse_args()
    checkout, output = args.checkout.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    assert subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip() == args.commit
    assert not subprocess.check_output(["git", "status", "--porcelain"], cwd=checkout, text=True).strip()
    root = Path(tempfile.mkdtemp(prefix="issue208-", dir=os.environ.get("RUNNER_TEMP"))).resolve()
    save(output / "environment.json", {"commit": args.commit, "fixed":args.fixed,"os":platform.platform(),
        "temporary_data_root":str(root),"harness_sha256":hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "entry_points":["commands::import_existing_skill","commands::sync_skill_to_tool","core::onboarding::build_onboarding_plan_for_runtime"],
        "runtime":"Tauri MockRuntime; real command bodies, database and Windows filesystem; no desktop webview",
        "deployment_setup":"Real sync_skill_to_tool commands deploy to seven built-in tools; privileged-runner symlinks are normalized to actual junctions using product APIs before the baseline. The official Skill and deletion controls use synthetic seed records.",
        "database_path_policy":"Short isolated temporary database paths; unicode-long varies source and central paths only. The first attempt retained evidence of startup failure with a 310-character database path.",
        "recycler":"Production trash::delete body enabled in test runtime; test-only permanent-deletion substitute removed"})
    target = checkout / "src-tauri/src/services/tests/library.rs"
    target.write_text(target.read_text(encoding="utf-8") + '\n#[path = "issue208_repro.rs"]\nmod issue208_repro;\n', encoding="utf-8")
    shutil.copyfile(Path(__file__).with_name("probe.rs"), target.with_name("issue208_repro.rs"))
    engine = checkout / "src-tauri/src/core/sync_engine.rs"
    original = engine.read_text(encoding="utf-8")
    production = '#[cfg(not(test))]\nfn recycle_path(path: &Path) -> Result<()> {\n    trash::delete(path).map_err(anyhow::Error::from)\n}'
    substitute = '#[cfg(test)]\nfn recycle_path(path: &Path) -> Result<()> {\n    remove_path_permanently(path)\n}'
    assert original.count(production) == original.count(substitute) == 1
    engine.write_text(original.replace(production, production.replace('#[cfg(not(test))]\n', '')).replace(substitute, ''), encoding="utf-8")
    (output / "instrumentation.patch").write_text(subprocess.check_output(["git","diff"],cwd=checkout,text=True),encoding="utf-8")
    env = dict(os.environ, ISSUE208_REPRO_ROOT=str(root), ISSUE208_EXPECT_FIXED=args.fixed)
    for name in ["DSH_HOME", "KIMI_CODE_HOME", "CODEX_HOME", "CLAUDE_CONFIG_DIR", "SKILLSHUB_DESKTOP_TEST_ROOT"]:
        env.pop(name, None)
    succeeded = False
    try:
        before_recycle = recycle_snapshot(root)
        save(output / "system-recycle-before.json", before_recycle)
        if os.name == "nt":
            assert before_recycle["entries"] == []
        host = next(line.split(": ",1)[1] for line in subprocess.check_output(["rustc","-vV"],text=True).splitlines() if line.startswith("host: "))
        assert run(["node","scripts/prepare-cli-sidecar.mjs","--target",host,"--debug"],checkout,env,output/"prepare.log") == 0
        code = run(["cargo","test","--locked","--features","cli","--lib","issue208_repro::","--","--nocapture"],checkout/"src-tauri",env,output/"test.log")
        save(output / "test-exit.json", {"exit_code":code})
        assert code == 0, f"reproduction test failed with exit code {code}; snapshots retained"
        ntfs = ntfs_observation(root)
        save(output / "ntfs-observation.json", ntfs)
        if os.name == "nt":
            assert ntfs["all_opened"], "one or more fixture directories cannot be opened; inspect NTSTATUS evidence"
        recycled = recycle_snapshot(root)
        save(output / "system-recycle-after.json", recycled)
        if os.name == "nt":
            entries = recycled["entries"]
            assert len(entries) == 1 and entries[0]["name"] == "system-recycle-control", entries
        succeeded = True
    finally:
        if (root/"reports").exists():
            shutil.copytree(root/"reports", output/"reports")
        if (root/"summary.json").exists():
            shutil.copyfile(root/"summary.json",output/"summary.json")
        if not (output/"ntfs-observation.json").exists():
            try:
                save(output/"ntfs-observation.json",ntfs_observation(root))
            except Exception as error:
                save(output/"ntfs-observation-error.json",{"error":str(error)})
        save(output/"result.json",{"passed":succeeded,"commit":args.commit,"fixed":args.fixed})
    print(f"ISSUE208_REPRO_OUTPUT={output}",flush=True)


if __name__ == "__main__":
    main()
