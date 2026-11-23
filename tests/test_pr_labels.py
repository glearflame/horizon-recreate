import unittest
from pathlib import Path

from tools.ci import pr_labels as pl

ROOT = Path(__file__).resolve().parents[1]


def body(*ticked, other=""):
    lines = [f"- [{'x' if word in ticked else ' '}] {word}: something" for word, _, _ in pl.KINDS]
    return "## What and why\n\nStuff.\n\n## Kind\n\n" + "\n".join(lines) + f"\n\n## Self-check\n\n{other}"


def event(new, old=None, labels=(), action="edited"):
    ev = {"action": action, "pull_request": {"number": 9, "body": new, "labels": [{"name": n} for n in labels]}}
    if old is not None:
        ev["changes"] = {"body": {"from": old}}
    return ev


class KindTests(unittest.TestCase):
    def test_ticked(self):
        self.assertEqual(pl.ticked(body("parity")), ["parity"])
        self.assertEqual(pl.ticked(body("docs")), ["documentation"])
        self.assertEqual(pl.ticked(body()), [])
        self.assertEqual(pl.ticked(body("perf", "bug")), ["bug", "perf"])
        self.assertEqual(pl.ticked(None), [])

    def test_only_the_kind_section(self):
        self.assertEqual(pl.ticked(body(other="- [x] bug\n- [x] tooling")), [])
        self.assertEqual(pl.ticked("- [x] parity"), [])
        self.assertEqual(pl.ticked("## Kind\n* [X] **perf**: faster\n- [x] unknown\n"), ["perf"])

    def test_the_template_has_every_kind_unticked(self):
        text = (ROOT / ".github/pull_request_template.md").read_text()
        self.assertEqual(pl.ticked(text), [])
        for word, _, _ in pl.KINDS:
            self.assertIn(f"- [ ] {word}", text)

    def test_added_when_none_yet(self):
        self.assertEqual(pl.plan_event(event(body("parity"), action="opened"), []), (["parity"], None))

    def test_one_box_only(self):
        self.assertEqual(pl.plan_event(event(body("parity", "bug"), action="opened"), []), ([], None))

    def test_a_kind_set_by_hand_stays(self):
        self.assertEqual(pl.plan_event(event(body("parity"), body(), labels=("feature",)), None), ([], None))

    def test_changing_the_tick_swaps(self):
        self.assertEqual(pl.plan_event(event(body("bug"), body("parity"), labels=("parity",)), None),
                         (["bug"], "parity"))

    def test_changing_the_tick_never_drops_a_hand_label(self):
        self.assertEqual(pl.plan_event(event(body("bug"), body("parity"), labels=("parity", "feature")), None),
                         ([], None))
        self.assertEqual(pl.plan_event(event(body("bug"), body("parity"), labels=("feature",)), None), ([], None))

    def test_already_there(self):
        self.assertEqual(pl.plan_event(event(body("perf"), body(), labels=("perf",)), None), ([], None))


class AreaTests(unittest.TestCase):
    def test_paths(self):
        cases = {
            "launcher/src/main.rs": "launcher",
            "crates/ll-world/src/worldgen/structure/mod.rs": "worldgen",
            "crates/ll-client/src/mobs/ai.rs": "mobs",
            "crates/ll-client/src/game/wither.rs": "mobs",
            "crates/ll-client-render/src/gfx/cull.rs": "rendering",
            "crates/ll-client-render/src/terrain.wgsl": "rendering",
            "crates/ll-client/src/game/dedicated/status.rs": "server",
            "crates/ll-server-files/src/lib.rs": "server",
            "crates/ll-client/src/lan/hub.rs": "multiplayer",
            "crates/ll-client/src/game/remote_vehicles.rs": "multiplayer",
            "crates/ll-protocol/src/packets/play.rs": "multiplayer",
            "crates/ll-mods/src/lib.rs": "mods",
            "crates/ll-client/src/game/native_hooks.rs": "mods",
            "crates/ll-world/src/datafix/v3.rs": "saves",
            "crates/ll-client/src/game/commands/admin.rs": "commands",
            "crates/ll-command/src/args.rs": "commands",
            "crates/ll-client-render/src/sound_engine.rs": "audio",
            "crates/ll-client-render/src/streaming.rs": "chunks",
            "crates/ll-world/src/level/redstone.rs": "blocks",
            "crates/ll-client/src/title/worlds.rs": "ui",
            "crates/ll-client-render/src/options_ui.rs": "ui",
            "docs/api/shaders.md": "website",
            "roadmap/roadmap.toml": "website",
        }
        for path, area in cases.items():
            self.assertEqual(pl.area_of(path), area, path)

    def test_no_area(self):
        for path in ("tools/parity.py", "crates/ll-client/src/game.rs", "Cargo.lock", "docs/README.md",
                     "mods/horizon/src/test/java/dev/horizonlab/parity/VanillaMobOracle.java",
                     "crates/ll-client/examples/mob_ai_probe.rs"):
            self.assertIsNone(pl.area_of(path), path)

    def test_biggest_three_with_a_tenth_each(self):
        files = [("launcher/src/a.rs", 50), ("crates/ll-client/src/mobs/a.rs", 30),
                 ("crates/ll-command/src/a.rs", 10), ("crates/ll-client-render/src/audio.rs", 9),
                 ("tools/x.py", 1)]
        self.assertEqual(pl.areas(files), ["launcher", "mobs", "commands"])
        self.assertEqual(pl.areas([("tools/x.py", 95), ("launcher/src/a.rs", 5)]), [])
        self.assertEqual(pl.areas([]), [])

    def test_files_count_on_push_not_on_edit(self):
        files = [("launcher/src/a.rs", 5)]
        self.assertEqual(pl.plan_event(event(body(), action="synchronize"), files), (["launcher"], None))
        self.assertEqual(pl.plan_event(event(body(), labels=("launcher",), action="synchronize"), files), ([], None))


class BackfillTests(unittest.TestCase):
    def test_guess_only_when_clear(self):
        self.assertEqual(pl.guess_kind("x", body("extra"), []), "extra")
        self.assertEqual(pl.guess_kind("x", None, [("docs/a.md", 3), ("README.md", 1)]), "documentation")
        self.assertEqual(pl.guess_kind("x", None, [("tools/a.py", 3), ("tests/test_a.py", 1)]), "tooling")
        self.assertEqual(pl.guess_kind("perf(client): less", None, [("launcher/a.rs", 1)]), "perf")
        self.assertEqual(pl.guess_kind("Fix a crash when", None, [("launcher/a.rs", 1)]), "bug")
        self.assertIsNone(pl.guess_kind("Mooshrooms keep their colour", None, [("crates/a.rs", 1)]))
        self.assertIsNone(pl.guess_kind("x", None, []))


class WorkflowTests(unittest.TestCase):
    def test_workflow_is_safe(self):
        text = (ROOT / ".github/workflows/pr-labels.yml").read_text()
        run_lines = [l.strip() for l in text.splitlines() if l.strip().startswith("run:")]
        self.assertEqual(run_lines, ["run: python3 tools/ci/pr_labels.py event"])
        self.assertIn("pull_request_target", text)
        self.assertIn("ref: main", text)
        self.assertIn("persist-credentials: false", text)
        for bad in ("contents: write", "issues: write", "head.sha", "head.ref", "github.event.pull_request.body"):
            self.assertNotIn(bad, text)
        # The checkout fetches the script alone.
        self.assertIn("sparse-checkout: |\n            tools/ci/pr_labels.py\n          sparse-checkout-cone-mode", text)


if __name__ == "__main__":
    unittest.main()
