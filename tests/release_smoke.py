#!/usr/bin/env python3
"""Run the installed release: version, init-config, headless IPC, clean exit."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

binary = str(Path(sys.argv[1]).resolve())
version = sys.argv[2] if len(sys.argv) > 2 else "0.1.0"
with tempfile.TemporaryDirectory(prefix="truss-smoke-") as tmp:
    root = Path(tmp)
    root.chmod(0o700)
    env = dict(os.environ, XDG_RUNTIME_DIR=tmp, XDG_CONFIG_HOME=tmp,
               HOME=tmp, RUST_LOG="warn")

    def cli(*args, check=True):
        return subprocess.run([binary, *args], env=env, text=True,
                              capture_output=True, check=check, timeout=10)

    assert cli("--version").stdout.strip() == f"truss {version}"
    print(f"version: truss {version}", flush=True)
    cli("init-config")
    config = root / "truss/config.lua"
    assert config.is_file()
    assert cli("init-config", check=False).returncode != 0
    print("init-config: created; existing config preserved", flush=True)
    # Avoid launching user applications during the isolated headless smoke test.
    config.write_text('truss.set("gap", 8)\n')
    with (root / "compositor.log").open("w+") as log:
        proc = subprocess.Popen([binary, "--backend", "headless", "--config", str(config)],
                                env=env, stdout=log, stderr=log)
        try:
            deadline = time.monotonic() + 10
            while not (root / "truss-0.sock").exists():
                if proc.poll() is not None or time.monotonic() >= deadline:
                    log.seek(0)
                    raise RuntimeError("headless startup failed: " + log.read())
                time.sleep(0.05)
            raw = json.loads(cli("msg", "state-get").stdout)
            state = raw.get("State", raw)
            assert state
            print("headless IPC: state-get returned real JSON", flush=True)
            cli("msg", "workspace-switch", "2")
            raw = json.loads(cli("msg", "state-get").stdout)
            state = raw.get("State", raw)
            assert state["active_workspace_id"] == 2, raw
            print("headless IPC: switched to workspace 2", flush=True)
            cli("msg", "quit")
            assert proc.wait(timeout=10) == 0
            print("headless IPC: quit exited 0", flush=True)
        finally:
            if proc.poll() is None:
                proc.terminate()
                try:
                    proc.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    proc.kill()
                    proc.wait()
