"""Review-only probe: run the built native test binary inside isolated mounts.

Usage: python3 native_probe.py /absolute/path/to/native_test_binary
No application source or real user autostart entry is modified.
"""
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path

evidence = Path(__file__).resolve().parent
repo = evidence.parents[3]
binary = Path(sys.argv[1]).resolve()
scratch = repo / ".tmp"
scratch.mkdir(exist_ok=True)
panic_test = "test_cleanup_guard_removes_file_on_panic_unwind"
results = []

with tempfile.TemporaryDirectory(prefix="autostart-native-review-", dir=scratch) as root:
    for name, read_only, nondefault, selected in [
        ("default", False, False, None),
        ("nondefault_xdg", False, True, None),
        ("panic_writable", False, False, panic_test),
        ("panic_readonly", True, False, panic_test),
    ]:
        case = Path(root) / name
        config = case / "config"
        autostart = config / "autostart"
        autostart.mkdir(parents=True)
        (case / "tmp").mkdir()
        (case / "xdg").mkdir()
        sentinel = autostart / "rustsale-review-preserved.desktop"
        sentinel.write_text("[Desktop Entry]\nName=preserve-review-fixture\n")
        before = hashlib.sha256(sentinel.read_bytes()).hexdigest()
        xdg = str(case / "xdg") if nondefault else str(Path.home() / ".config")
        command = [
            "bwrap", "--ro-bind", "/", "/",
            "--bind", str(case), str(case),
            "--ro-bind" if read_only else "--bind",
            str(config), str(Path.home() / ".config"),
            "--proc", "/proc", "--dev", "/dev", "--unshare-net",
            "--setenv", "TMPDIR", str(case / "tmp"),
            "--setenv", "XDG_CONFIG_HOME", xdg,
            str(binary),
        ]
        if selected:
            command.extend(["--exact", selected])
        command.append("--nocapture")
        run = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        (evidence / f"probe-{name}.log").write_text(run.stdout)
        results.append({
            "case": name,
            "command": command,
            "exit_code": run.returncode,
            "intentional_panic_observed": "simulated assertion failure to test RAII CleanupGuard drop" in run.stdout,
            "readonly_error_observed": "Read-only file system" in run.stdout,
            "sentinel_preserved": sentinel.exists() and hashlib.sha256(sentinel.read_bytes()).hexdigest() == before,
            "remaining_test_entries": [p.name for p in autostart.glob("rustsale-crm-test-*.desktop")],
        })

(evidence / "native-probes.json").write_text(json.dumps(results, indent=2))
print(json.dumps(results, indent=2))
