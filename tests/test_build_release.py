"""Exercise the real packager with a tiny, native Rust fixture (not the compositor).

Run with: python3 -m unittest discover -s tests -p test_build_release.py -v
Requires Python 3.11+, cargo/rustc, and GNU tar/coreutils; no third-party modules.
"""

import hashlib
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts/build-release.sh"


class BuildReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="truss-release-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        for directory in ("scripts", "src", "resources", "examples/waybar"):
            (self.root / directory).mkdir(parents=True)
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "truss"\nversion = "2.3.4"\nedition = "2021"\n'
        )
        (self.root / "src/main.rs").write_text(
            'fn main() { assert_eq!(std::env::args().nth(1).as_deref(), '
            'Some("--version")); println!("truss {}", env!("CARGO_PKG_VERSION")); }\n'
        )
        (self.root / "resources/config.default.lua").write_text("-- fixture config\n")
        for name in ("truss-session", "truss.desktop", "truss-session.target"):
            (self.root / "resources" / name).write_text("# fixture session resource\n")
        (self.root / "examples/waybar/config.jsonc").write_text("{}\n")
        status = self.root / "examples/waybar/truss-status.py"
        status.write_text("#!/usr/bin/env python3\n")
        status.chmod(0o755)
        self.env = os.environ.copy()
        for key in ("RELEASE_TAG", "CARGO_TARGET_DIR", "CARGO_BUILD_TARGET"):
            self.env.pop(key, None)
        subprocess.run(["cargo", "generate-lockfile"], cwd=self.root, env=self.env, check=True)

    def run_packager(self, **extra_env):
        self.assertTrue(SCRIPT.is_file(), "release packaging script must exist")
        shutil.copy2(SCRIPT, self.root / "scripts/build-release.sh")
        return subprocess.run(
            ["bash", str(self.root / "scripts/build-release.sh"), str(self.root / "output with spaces")],
            cwd="/tmp", env=self.env | extra_env, text=True, capture_output=True,
        )

    def test_release_tag_must_match_manifest(self):
        result = self.run_packager(RELEASE_TAG="v2.3.5")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Release tag mismatch", result.stderr)
        self.assertFalse((self.root / "target").exists())

    def test_native_archive_layout_checksum_and_version(self):
        result = self.run_packager()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        name = f"truss-2.3.4-{platform.machine()}-linux.tar.gz"
        output = self.root / "output with spaces"
        archive = output / name
        self.assertTrue(archive.is_file(), result.stdout)
        self.assertEqual(
            (output / f"{name}.sha256").read_text(),
            f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {name}\n",
        )
        with tarfile.open(archive) as tar:
            files = {member.name for member in tar.getmembers() if member.isfile()}
            self.assertEqual(files, {
                "bin/truss", "share/truss/config.default.lua", "bin/truss-session",
                "share/wayland-sessions/truss.desktop", "lib/systemd/user/truss-session.target",
                "examples/waybar/config.jsonc", "examples/waybar/truss-status.py",
            })
            self.assertEqual(tar.getmember("bin/truss").mode, 0o755)
            self.assertTrue(tar.getmember("examples/waybar/truss-status.py").mode & 0o111)
            tar.extractall(self.root / "unpacked", filter="data")
        smoke = subprocess.run(
            [str(self.root / "unpacked/bin/truss"), "--version"],
            text=True, capture_output=True, check=True,
        )
        self.assertEqual(smoke.stdout, "truss 2.3.4\n")
        self.assertEqual(sorted(p.name for p in output.iterdir()), [name, f"{name}.sha256"])


if __name__ == "__main__":
    unittest.main()
