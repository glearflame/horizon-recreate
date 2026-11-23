"""Offline smoke tests: real client startup and vanilla resource integrity."""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

CLIENT = os.environ.get("LL_CLIENT", os.path.expanduser("~/.local/bin/ll-client"))
RESOURCES = os.environ.get("LL_RESOURCES", os.path.expanduser("~/.horizon/resources"))
SCRIPT = Path(__file__).resolve().parents[1] / "play-offline.sh"


class OfflinePlayTests(unittest.TestCase):
    def test_client_help_runs(self):
        out = subprocess.run([CLIENT, "--help"], capture_output=True, text=True, timeout=30)
        self.assertEqual(out.returncode, 0, f"--help falhou: {out.stderr}")
        self.assertIn("resources", out.stdout, f"--help nao menciona resources: {out.stdout}")

    def test_resources_dir_exists(self):
        self.assertTrue(os.path.isdir(RESOURCES), f"resources nao encontrados em {RESOURCES}")

    def test_damage_catalog_uses_vanilla_identifiers(self):
        catalog = json.loads((Path(RESOURCES) / "damage-catalog.json").read_text(encoding="utf-8"))
        allowed = {"default", "fall_variants", "intentional_game_design"}
        actual = {entry.get("death_message_type") for entry in catalog["types"]}
        self.assertLessEqual(actual, allowed, f"identificadores corrompidos: {sorted(actual - allowed)}")

    def test_resource_json_has_no_rebrand_corruption(self):
        forbidden = ("faglear", "beglear", "figlear", "ceglear", "smaglear", "taglear", "waglear")
        offenders = []
        for path in Path(RESOURCES).rglob("*.json"):
            data = path.read_bytes()
            if any(token.encode() in data for token in forbidden):
                offenders.append(str(path.relative_to(RESOURCES)))
        self.assertEqual(offenders, [], f"arquivos com identificadores mutilados: {offenders}")

    def test_research_lock_was_restored(self):
        lock = json.loads((SCRIPT.parent / "research-lock.json").read_text(encoding="utf-8"))
        self.assertEqual(lock["minecraft"]["version"], "26.3")
        self.assertEqual(lock["minecraft"]["java_major"], 25)

    def test_no_microsoft_offline(self):
        out = subprocess.run([CLIENT, "--help"], capture_output=True, text=True, timeout=30)
        help_text = (out.stdout + out.stderr).lower()
        self.assertNotIn("microsoft", help_text, "help ainda menciona microsoft")
        self.assertNotIn("oauth", help_text, "help ainda menciona oauth")

    @unittest.skipUnless(os.environ.get("LL_SMOKE") == "1", "smoke grafico opt-in: LL_SMOKE=1")
    def test_play_script_draws_frames(self):
        scratch_root = Path.home() / ".hermes" / "cache" / "scratch"
        scratch_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="ll-offline-test-", dir=scratch_root) as directory:
            home = Path(directory)
            env = os.environ.copy()
            env.pop("DISPLAY", None)
            env.pop("WAYLAND_DISPLAY", None)
            env.update({
                "HOME": str(home),
                "CLIENT": CLIENT,
                "RESOURCES": RESOURCES,
                "JAVA_HOME": str(Path.home() / ".java" / "temurin-25"),
                "HORIZON_HOME": str(home / "horizon-home"),
                "HORIZON_LAUNCHER_HOME": str(home / "launcher-home"),
                "HORIZON_LAUNCHER_NO_SELF_UPDATE": "1",
                "HORIZON_NO_AUDIO": "1",
                "XDG_DATA_HOME": str(home / "data"),
                "XDG_CONFIG_HOME": str(home / "config"),
            })
            out = subprocess.run(
                [str(SCRIPT), "--frames", "5", "--offscreen"],
                capture_output=True,
                text=True,
                timeout=90,
                env=env,
            )
            crashes = "\n".join(
                path.read_text(encoding="utf-8", errors="replace")
                for path in home.rglob("crash-*-client.txt")
            )
            self.assertEqual(out.returncode, 0, f"cliente nao desenhou 5 frames: {out.stdout}\n{out.stderr}\n{crashes}")


if __name__ == "__main__":
    unittest.main()
