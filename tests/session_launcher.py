#!/usr/bin/env python3
"""Exercise session launcher contracts without opening a real VT."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

LAUNCHER = Path(__file__).resolve().parents[1] / "resources/truss-session"


class SessionLauncher(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.log = self.root / "calls"
        self.env = dict(os.environ, PATH=f"{self.root}:/usr/bin:/bin",
                        XDG_RUNTIME_DIR=str(self.root),
                        DBUS_SESSION_BUS_ADDRESS="test:address",
                        CALL_LOG=str(self.log))
        self.mock("id", 'printf "%s\\n" "${TEST_UID:-1000}"')
        self.mock("systemctl", 'printf "systemctl %s\\n" "$*" >> "$CALL_LOG"')
        self.mock("dbus-update-activation-environment",
                  'printf "dbus %s\\n" "$*" >> "$CALL_LOG"')
        self.mock("truss", 'printf "truss %s %s %s %s %s\\n" "$*" "$XDG_SESSION_TYPE" "$WAYLAND_DISPLAY" "$LIBSEAT_BACKEND" "${DISPLAY-unset}" >> "$CALL_LOG"; exit "${TEST_EXIT:-0}"')
        self.mock("dbus-run-session", 'printf "new-bus\\n" >> "$CALL_LOG"; shift; export DBUS_SESSION_BUS_ADDRESS=test:new; exec /bin/sh "$@"')

    def mock(self, name, body):
        p = self.root / name
        p.write_text("#!/bin/sh\n" + body + "\n")
        p.chmod(0o755)

    def run_launcher(self, *args):
        return subprocess.run(["/bin/sh", str(LAUNCHER), *args], env=self.env,
                              capture_output=True, text=True, timeout=5)

    def calls(self):
        return self.log.read_text() if self.log.exists() else ""

    def test_session_and_cleanup(self):
        self.env["DISPLAY"] = ":0"
        self.assertEqual(self.run_launcher().returncode, 0)
        calls = self.calls()
        self.assertIn("truss --backend tty wayland truss-0 logind unset", calls)
        self.assertIn("systemctl --user start truss-session.target", calls)
        self.assertIn("systemctl --user stop truss-session.target", calls)
        self.assertIn("systemctl --user unset-environment WAYLAND_DISPLAY", calls)

    def test_exit_status_preserved(self):
        self.env["TEST_EXIT"] = "23"
        self.assertEqual(self.run_launcher().returncode, 23)
        self.assertIn("stop truss-session.target", self.calls())

    def test_missing_runtime_rejected(self):
        self.env.pop("XDG_RUNTIME_DIR")
        self.assertNotEqual(self.run_launcher().returncode, 0)
        self.assertEqual(self.calls(), "")

    def test_root_rejected(self):
        self.env["TEST_UID"] = "0"
        self.assertNotEqual(self.run_launcher().returncode, 0)
        self.assertEqual(self.calls(), "")

    def test_arguments_rejected(self):
        self.assertNotEqual(self.run_launcher("--backend", "winit").returncode, 0)
        self.assertEqual(self.calls(), "")

    def test_new_dbus_session(self):
        self.env.pop("DBUS_SESSION_BUS_ADDRESS")
        self.assertEqual(self.run_launcher().returncode, 0)
        self.assertIn("new-bus", self.calls())

    def test_without_user_systemd(self):
        self.mock("systemctl", "exit 1")
        self.assertEqual(self.run_launcher().returncode, 0)
        self.assertNotIn("--systemd", self.calls())
        self.assertIn("truss --backend tty", self.calls())

    def test_explicit_seat_backend_preserved(self):
        self.env["LIBSEAT_BACKEND"] = "seatd"
        self.assertEqual(self.run_launcher().returncode, 0)
        self.assertIn("truss-0 seatd", self.calls())


if __name__ == "__main__":
    unittest.main()
