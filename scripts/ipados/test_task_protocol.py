#!/usr/bin/env python3
"""Protocol tests using temporary Git repos and a fake CI command; no iPad needed."""

import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest


SCRIPT = Path(__file__).with_name("task_protocol.py").resolve()
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("task_protocol", SCRIPT)
PROTOCOL = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROTOCOL)


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="solidcraft-protocol-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name) / "repo with spaces"
        self.root.mkdir()
        self.git(self.root, "init", "-q", "-b", "develop")
        for key, value in (("user.name", "Protocol Test"), ("user.email", "test@example.invalid"),
                           ("commit.gpgsign", "false")):
            self.git(self.root, "config", key, value)
        (self.root / ".gitignore").write_text("target/\n")
        (self.root / "base.txt").write_text("base\n")
        self.git(self.root, "add", ".gitignore", "base.txt")
        self.git(self.root, "commit", "-qm", "baseline")
        self.base = self.git(self.root, "rev-parse", "HEAD").strip()
        self.bin = Path(self.tmp.name) / "bin"
        self.bin.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text(
            "#!" + sys.executable + "\n"
            "import os, pathlib, subprocess, sys, time\n"
            "assert sys.argv[1:] == ['+1.95.0', 'fmt', '--all', '--', '--check'] or "
            "(sys.argv[1:3] == ['+1.95.0', 'xtask'] and sys.argv[3] in "
            "['ci', 'assets', 'icons', 'lists', 'skills', 'layers', 'ui-containers', 'licences'])\n"
            "if os.environ.get('MUTATE_SOURCE'):\n"
            "    pathlib.Path('base.txt').write_text('changed during check\\n')\n"
            "if os.environ.get('MOVE_DEVELOP_TO'):\n"
            "    subprocess.run(['git', 'update-ref', 'refs/heads/develop', os.environ['MOVE_DEVELOP_TO']], check=True)\n"
            "if os.environ.get('CHECK_READY'):\n"
            "    pathlib.Path(os.environ['CHECK_READY']).write_text('ready'); time.sleep(2)\n"
            "sys.exit(int(os.environ.get('CHECK_EXIT', '0')))\n"
        )
        cargo.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"])

    def git(self, root, *args):
        return subprocess.run(["git", "-C", str(root), *args], text=True,
                              capture_output=True, check=True).stdout

    def cli(self, root, *args, ok=True, env=None):
        result = subprocess.run([sys.executable, "-I", str(SCRIPT), *args], cwd=root,
                                env=env or self.env, capture_output=True, text=True)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def task(self, name, ipad=False):
        options = [] if ipad else ["--ipad", "not-needed", "--reason", "Tooling only"]
        result = self.cli(self.root, "start", name, *options)
        return Path(json.loads(result.stdout)["worktree"])

    def change(self, root, name="feature.txt", content="task change\n"):
        (root / name).write_text(content)
        self.git(root, "add", name)
        self.git(root, "commit", "-qm", "task change")
        return self.git(root, "rev-parse", "HEAD").strip()

    def record(self, name):
        return json.loads((self.root / ".git/solidcraft-tasks/tasks" / (name + ".json")).read_text())

    def hold(self, root, name, evidence):
        process = subprocess.Popen(
            [sys.executable, "-I", str(SCRIPT), "ipad", name,
             "--device", "fixture-ipad", "--evidence", str(evidence), "--hold"],
            cwd=root, env=self.env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True,
        )
        self.addCleanup(self.stop, process)
        self.assertIn("IPAD RESERVED", process.stdout.readline())
        self.assertIn("Keep this process open", process.stdout.readline())
        return process

    @staticmethod
    def stop(process):
        if process.poll() is None:
            process.kill()
        process.communicate(timeout=5)

    def test_worktree_isolation_and_shared_record(self):
        first = self.task("first")
        second = self.task("second")
        self.change(first)
        self.assertFalse((second / "feature.txt").exists())
        self.assertFalse((self.root / "feature.txt").exists())
        status = json.loads(self.cli(second, "status").stdout)
        self.assertEqual({t["task"] for t in status["tasks"]}, {"first", "second"})
        self.assertEqual(self.record("first")["base"], self.base)
        self.cli(self.root, "start", "first", ok=False)
        self.cli(first, "finish", "first", ok=False)

    def test_routine_tooling_and_swift_changes_avoid_full_engine_suite(self):
        scope, commands = PROTOCOL.check_plan(["scripts/ipados/task_protocol.py", "AGENTS.md"])
        self.assertEqual(scope, "affected")
        self.assertTrue(any("scripts/ipados/test_task_protocol.py" in command for command in commands))
        self.assertNotIn(["cargo", "+1.95.0", "xtask", "ci"], commands)
        scope, commands = PROTOCOL.check_plan(["apps/solidcraft-ipados/SolidCraft/WorkspaceController.swift"])
        self.assertEqual(scope, "affected")
        self.assertIn(["sh", "apps/solidcraft-ipados/scripts/build.sh"], commands)
        self.assertIn(["sh", "apps/solidcraft-ipados/scripts/check-workspace.sh"], commands)
        self.assertFalse(any("ci" == command[-1] and "xtask" in command for command in commands))

    def test_adapter_changes_test_both_adapter_crates_and_native_boundary(self):
        scope, commands = PROTOCOL.check_plan(["crates/ipados/session/src/input.rs"])
        self.assertEqual(scope, "affected")
        tests = next(command for command in commands if "test" in command)
        self.assertIn("solidcraft-ipados-session", tests)
        self.assertIn("solidcraft-ipados-bridge", tests)
        self.assertNotIn("--workspace", tests)
        self.assertIn(["sh", "apps/solidcraft-ipados/scripts/build.sh"], commands)

    def test_shared_code_unknown_paths_and_explicit_checkpoints_require_full_ci(self):
        for path in ["crates/engine/src/lib.rs", "crates/kernel/src/lib.rs", "Cargo.lock",
                     "apps/solidcraft/src/main.rs", "unclassified.conf"]:
            scope, commands = PROTOCOL.check_plan([path])
            self.assertEqual(scope, "full", path)
            self.assertEqual(commands, [["cargo", "+1.95.0", "xtask", "ci"]])
        self.assertEqual(PROTOCOL.check_plan(["README.md"], full=True)[0], "full")

    def test_dirty_develop_blocks_start_and_integration_without_touching_work(self):
        task = self.task("independent")
        self.change(task)
        (self.root / "base.txt").write_text("another chat's uncommitted work\n")
        self.cli(self.root, "start", "later", ok=False)
        self.cli(task, "integrate", "independent", ok=False)
        self.assertEqual(self.git(self.root, "rev-parse", "HEAD").strip(), self.base)
        self.assertEqual((self.root / "base.txt").read_text(), "another chat's uncommitted work\n")

    def test_concurrent_branches_integrate_and_keep_both_histories(self):
        first = self.task("first")
        second = self.task("second")
        first_tip = self.change(first, "first.txt")
        second_tip = self.change(second, "second.txt")
        self.cli(first, "integrate", "first")
        self.cli(second, "integrate", "second")
        for tip in (first_tip, second_tip):
            self.git(self.root, "merge-base", "--is-ancestor", tip, "develop")
        self.assertTrue((self.root / "first.txt").exists())
        self.assertTrue((self.root / "second.txt").exists())
        self.cli(second, "finish", "second")
        self.assertEqual(self.record("second")["state"], "done")
        self.assertEqual(self.record("second")["checks"][0]["exit_code"], 0)
        self.assertEqual(self.record("second")["checks"][0]["test_jobs"], "2")
        self.assertEqual(self.record("second")["checks"][0]["build_jobs"], "2")

    def test_failed_ci_and_source_mutation_cannot_update_develop(self):
        task = self.task("fails")
        self.change(task)
        self.cli(task, "integrate", "fails", env=dict(self.env, CHECK_EXIT="9"), ok=False)
        self.assertEqual(self.git(self.root, "rev-parse", "HEAD").strip(), self.base)
        self.assertEqual(self.record("fails")["checks"][0]["exit_code"], 9)
        self.cli(task, "finish", "fails", ok=False)
        self.cli(task, "integrate", "fails", env=dict(self.env, MUTATE_SOURCE="1"), ok=False)
        self.assertEqual(self.git(self.root, "rev-parse", "HEAD").strip(), self.base)

    def test_failed_reverification_invalidates_an_earlier_integration(self):
        task = self.task("recheck")
        self.change(task)
        self.cli(task, "integrate", "recheck")
        self.cli(task, "finish", "recheck")
        self.cli(task, "integrate", "recheck", env=dict(self.env, CHECK_EXIT="9"), ok=False)
        self.cli(task, "finish", "recheck", ok=False)
        self.assertIsNone(self.record("recheck")["verified"])
        self.cli(task, "integrate", "recheck")
        self.cli(task, "finish", "recheck")

    def test_integration_lock_is_held_through_candidate_verification(self):
        first = self.task("first")
        second = self.task("second")
        first_tip = self.change(first, "first.txt")
        second_tip = self.change(second, "second.txt")
        ready = Path(self.tmp.name) / "check-ready"
        process = subprocess.Popen(
            [sys.executable, "-I", str(SCRIPT), "integrate", "first"], cwd=first,
            env=dict(self.env, CHECK_READY=str(ready)), stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True,
        )
        self.addCleanup(self.stop, process)
        deadline = time.monotonic() + 5
        while not ready.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertTrue(ready.exists())
        status = json.loads(self.cli(second, "status").stdout)
        self.assertTrue(status["resources"]["integration"]["busy"])
        self.assertTrue(status["resources"]["build"]["busy"])
        self.cli(second, "build", "second", "--", sys.executable, "-c", "pass", ok=False)
        failed = self.cli(second, "integrate", "second", ok=False)
        self.assertIn("integration is busy", failed.stderr)
        self.assertEqual(self.git(self.root, "rev-parse", "HEAD").strip(), self.base)
        output, error = process.communicate(timeout=10)
        self.assertEqual(process.returncode, 0, output + error)
        self.cli(second, "integrate", "second")
        for tip in (first_tip, second_tip):
            self.git(self.root, "merge-base", "--is-ancestor", tip, "develop")

    def test_merge_conflict_preserves_develop_and_both_commits(self):
        first = self.task("first")
        second = self.task("second")
        self.change(first, "base.txt", "first\n")
        second_tip = self.change(second, "base.txt", "second\n")
        self.cli(first, "integrate", "first")
        old = self.git(self.root, "rev-parse", "HEAD")
        self.cli(second, "integrate", "second", ok=False)
        self.assertEqual(self.git(self.root, "rev-parse", "HEAD"), old)
        self.assertEqual(self.git(second, "rev-parse", "HEAD").strip(), second_tip)
        self.assertTrue(self.git(second, "ls-files", "-u").strip())

    def test_external_develop_movement_cannot_be_overwritten(self):
        task = self.task("moving")
        tip = self.change(task)
        tree = self.git(self.root, "rev-parse", "HEAD^{tree}").strip()
        external = self.git(self.root, "commit-tree", tree, "-p", self.base, "-m", "external advance").strip()
        self.cli(task, "integrate", "moving", env=dict(self.env, MOVE_DEVELOP_TO=external), ok=False)
        self.assertEqual(self.git(self.root, "rev-parse", "HEAD").strip(), external)
        self.assertEqual(self.git(task, "rev-parse", "HEAD").strip(), tip)
        self.assertIsNone(self.record("moving")["integrated"])

    def test_device_required_and_new_commits_invalidate_completion(self):
        task = self.task("device", ipad=True)
        self.change(task)
        self.cli(task, "integrate", "device")
        self.cli(task, "finish", "device", ok=False)
        evidence = Path(self.tmp.name) / "device.txt"
        command = [sys.executable, "-c",
                   "import os,pathlib; pathlib.Path(" + repr(str(evidence)) +
                   ").write_text(os.environ['SOLIDCRAFT_CANDIDATE'])"]
        self.cli(task, "ipad", "device", "--device", "fixture-ipad",
                 "--evidence", str(evidence), "--", *command)
        self.cli(task, "finish", "device")
        self.assertEqual(self.record("device")["state"], "done")
        self.change(task, "later.txt")
        self.cli(task, "finish", "device", ok=False)
        self.assertEqual(self.record("device")["state"], "pending")

    def test_device_lock_contention_and_release(self):
        first = self.task("first", ipad=True)
        self.change(first)
        self.cli(first, "integrate", "first")
        second = self.task("second", ipad=True)
        self.change(second, "other.txt")
        self.cli(second, "integrate", "second")
        first_evidence = Path(self.tmp.name) / "first.txt"
        held = self.hold(first, "first", first_evidence)
        status = json.loads(self.cli(second, "status").stdout)
        self.assertTrue(status["resources"]["ipad"]["busy"])
        self.assertEqual(status["resources"]["ipad"]["owner"]["task"], "first")
        self.cli(second, "ipad", "second", "--device", "fixture-ipad",
                 "--evidence", str(Path(self.tmp.name) / "second.txt"), "--hold", ok=False)
        first_evidence.write_text("Captured fixture evidence\n")
        output, error = held.communicate("pass\n", timeout=5)
        self.assertEqual(held.returncode, 0, output + error)
        self.assertFalse(json.loads(self.cli(second, "status").stdout)["resources"]["ipad"]["busy"])

    def test_killed_session_releases_lock_but_never_records_a_pass(self):
        task = self.task("killed", ipad=True)
        self.change(task)
        self.cli(task, "integrate", "killed")
        held = self.hold(task, "killed", Path(self.tmp.name) / "missing.txt")
        held.kill()
        held.communicate(timeout=5)
        self.assertFalse(json.loads(self.cli(task, "status").stdout)["resources"]["ipad"]["busy"])
        self.assertIsNone(self.record("killed")["device"])
        self.cli(task, "finish", "killed", ok=False)

    def test_lock_survives_wrapper_death_until_command_exits(self):
        task = self.task("child", ipad=True)
        self.change(task)
        self.cli(task, "integrate", "child")
        pidfile = Path(self.tmp.name) / "child.pid"
        child = "import os,pathlib,time; pathlib.Path(" + repr(str(pidfile)) + ").write_text(str(os.getpid())); time.sleep(20)"
        process = subprocess.Popen(
            [sys.executable, "-I", str(SCRIPT), "ipad", "child", "--device", "fixture",
             "--evidence", str(Path(self.tmp.name) / "absent.txt"), "--", sys.executable, "-c", child],
            cwd=task, env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
        )
        self.addCleanup(self.stop, process)
        deadline = time.monotonic() + 5
        while not pidfile.exists() and time.monotonic() < deadline:
            time.sleep(0.02)
        self.assertTrue(pidfile.exists())
        child_pid = int(pidfile.read_text())
        try:
            process.kill()
            process.wait(timeout=5)
            self.assertTrue(json.loads(self.cli(task, "status").stdout)["resources"]["ipad"]["busy"])
        finally:
            os.kill(child_pid, signal.SIGTERM)
        process.communicate(timeout=5)
        deadline = time.monotonic() + 5
        while json.loads(self.cli(task, "status").stdout)["resources"]["ipad"]["busy"]:
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.02)

    def test_unsafe_names_and_unjustified_device_skip_are_rejected(self):
        self.cli(self.root, "start", "../other", ok=False)
        self.cli(self.root, "start", "skip", "--ipad", "not-needed", ok=False)
        self.cli(self.root, "start", "wait", "--wait", "nan", ok=False)


if __name__ == "__main__":
    unittest.main()
