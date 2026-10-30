#!/usr/bin/env python3
"""Shared-checkout tests with temporary Git repositories and fake builds; no iPad."""

import fcntl
import importlib.util
import json
import os
from pathlib import Path
import shlex
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
        self.git("init", "-q", "-b", "develop")
        for key, value in (("user.name", "Protocol Test"), ("user.email", "test@example.invalid"),
                           ("commit.gpgsign", "false")):
            self.git("config", key, value)
        (self.root / ".gitignore").write_text("target/\n")
        (self.root / "base.txt").write_text("base\n")
        self.git("add", ".gitignore", "base.txt")
        self.git("commit", "-qm", "baseline")
        self.base = self.git("rev-parse", "HEAD").strip()
        self.state = self.root / ".git/solidcraft-tasks"
        self.control = self.root / ".git/check-control.json"
        self.log = self.root / ".git/check-log.txt"
        self.bin = Path(self.tmp.name) / "bin"
        self.bin.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text(
            "#!" + sys.executable + "\n"
            "import json, os, pathlib, sys, time\n"
            "control = pathlib.Path('.git/check-control.json')\n"
            "config = json.loads(control.read_text()) if control.exists() else {}\n"
            "with pathlib.Path('.git/check-log.txt').open('a') as log: log.write(' '.join(sys.argv[1:]) + '\\n')\n"
            "if config.get('mutate'): pathlib.Path('base.txt').write_text('changed during check\\n')\n"
            "if config.get('ready'):\n"
            "    pathlib.Path(config['ready'], str(os.getpid())).write_text('ready')\n"
            "    deadline = time.monotonic() + 10\n"
            "    while not pathlib.Path(config['release']).exists():\n"
            "        if time.monotonic() > deadline: sys.exit(8)\n"
            "        time.sleep(0.02)\n"
            "sys.exit(config.get('exit', 0))\n"
        )
        cargo.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.bin) + os.pathsep + os.environ["PATH"])

    def git(self, *args):
        return subprocess.run(["git", "-C", str(self.root), *args], text=True,
                              capture_output=True, check=True).stdout

    def cli(self, *args, ok=True, env=None, root=None):
        result = subprocess.run([sys.executable, "-I", str(SCRIPT), *args], cwd=root or self.root,
                                env=env or self.env, capture_output=True, text=True, timeout=15)
        if ok:
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        return result

    def check(self, **kwargs):
        return self.cli("check", "--paths", "base.txt", **kwargs)

    def count(self):
        return len(self.log.read_text().splitlines()) if self.log.exists() else 0

    def device_receipt(self, task):
        return json.loads((self.state / "device-sessions" / (task + ".json")).read_text())

    def hold(self, task, evidence):
        process = subprocess.Popen(
            [sys.executable, "-I", str(SCRIPT), "ipad", task, "--device", "fixture-ipad",
             "--evidence", str(evidence), "--hold"], cwd=self.root, env=self.env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, process)
        self.assertIn("IPAD RESERVED", process.stdout.readline())
        self.assertIn("Keep this process open", process.stdout.readline())
        return process

    @staticmethod
    def stop(process):
        if process.poll() is None:
            process.kill()
        process.communicate(timeout=5)

    def wait_for(self, predicate):
        deadline = time.monotonic() + 5
        while not predicate():
            self.assertLess(time.monotonic(), deadline)
            time.sleep(0.02)

    def test_retired_commands_never_create_worktrees_or_modify_source(self):
        for action in ("start", "adopt", "integrate", "finish", "note", "build"):
            result = self.cli(action, "example", ok=False)
            self.assertIn("retired", result.stderr)
        self.assertEqual(self.git("worktree", "list", "--porcelain").count("worktree "), 1)
        self.assertEqual(self.git("rev-parse", "HEAD").strip(), self.base)
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_primary_develop_required_without_switching_branches(self):
        self.git("switch", "-c", "codex/old")
        self.check(ok=False)
        self.assertEqual(self.git("branch", "--show-current").strip(), "codex/old")
        linked = Path(self.tmp.name) / "linked"
        self.git("worktree", "add", "-b", "codex/linked", str(linked))
        self.cli("check", "--full", root=linked, ok=False)

    def test_dirty_shared_checkout_is_supported_without_staging_others_work(self):
        (self.root / "base.txt").write_text("another task's edits\n")
        (self.root / "notes.md").write_text("owned documentation\n")
        self.cli("check", "--paths", "notes.md")
        self.assertEqual((self.root / "base.txt").read_text(), "another task's edits\n")
        self.assertEqual(self.git("diff", "--cached"), "")
        self.assertEqual(self.git("rev-parse", "HEAD").strip(), self.base)
        self.assertEqual(self.count(), 0)

    def test_docs_and_no_changes_do_not_run_tests(self):
        self.assertIn("no tests run", self.cli("check").stdout)
        self.assertEqual(PROTOCOL.check_plan(["docs/ipados/INPUT.md"]), ("affected", []))
        scope, commands = PROTOCOL.check_plan(["scripts/ipados/task_protocol.py", "AGENTS.md"])
        self.assertEqual(scope, "affected")
        self.assertEqual(len(commands), 2)
        self.assertIn([sys.executable, "-I", "scripts/ipados/test_task_protocol.py"], commands)

    def test_native_and_adapter_checks_stay_scoped(self):
        scope, commands = PROTOCOL.check_plan(["apps/solidcraft-ipados/SolidCraft/WorkspaceController.swift"])
        self.assertEqual(scope, "affected")
        self.assertEqual(commands, [["sh", "apps/solidcraft-ipados/scripts/build.sh"],
                                    ["sh", "apps/solidcraft-ipados/scripts/check-workspace.sh"]])
        _, commands = PROTOCOL.check_plan(["crates/ipados/session/src/input.rs"])
        tests = next(command for command in commands if "test" in command)
        self.assertIn("solidcraft-ipados-session", tests)
        self.assertIn("solidcraft-ipados-bridge", tests)
        self.assertNotIn("--workspace", tests)

    def test_shared_unknown_and_full_checkpoints_retain_full_ci(self):
        for path in ("crates/engine/src/lib.rs", "crates/kernel/src/lib.rs", "Cargo.lock", "unknown.conf"):
            self.assertEqual(PROTOCOL.check_plan([path]), ("full", [["cargo", "+1.95.0", "xtask", "ci"]]))
        self.cli("check", "--full")
        self.cli("check", "--full")
        self.assertEqual(self.count(), 2)

    def test_repeated_checks_and_commit_only_changes_reuse_success(self):
        (self.root / "base.txt").write_text("finished batch\n")
        self.check()
        self.assertIn("Reusing", self.check().stdout)
        self.git("add", "base.txt")
        self.git("commit", "-qm", "finished batch")
        self.assertIn("Reusing", self.check().stdout)
        self.assertEqual(self.count(), 1)
        (self.root / "base.txt").write_text("next batch\n")
        self.check()
        self.assertEqual(self.count(), 2)

    def test_failed_forced_rerun_invalidates_old_success(self):
        self.check()
        self.control.write_text(json.dumps({"exit": 9}))
        self.cli("check", "--paths", "base.txt", "--force", ok=False)
        self.check(ok=False)
        self.assertEqual(self.count(), 3)
        self.control.write_text("{}")
        self.check()
        self.assertIn("Reusing", self.check().stdout)
        self.assertEqual(self.count(), 4)

    def test_environment_and_command_changes_require_new_checks(self):
        self.check()
        self.check(env=dict(self.env, RUSTFLAGS="--cfg fixture"))
        extra = shlex.join([sys.executable, "-c", "print('extra check')"])
        self.cli("check", "--paths", "base.txt", "--check", extra)
        self.assertEqual(self.count(), 3)

    def test_source_mutation_cannot_record_success(self):
        self.control.write_text(json.dumps({"mutate": True}))
        self.assertIn("changed during verification", self.check(ok=False).stderr)
        receipts = list((self.state / "checks").glob("*.json"))
        self.assertFalse(any(json.loads(path.read_text())["passed"] for path in receipts))
        self.assertEqual(self.git("rev-parse", "HEAD").strip(), self.base)

    def test_legacy_build_and_integration_locks_do_not_block_checks(self):
        self.state.mkdir(parents=True)
        with (self.state / "build.lock").open("w") as build, (self.state / "integration.lock").open("w") as integration:
            fcntl.flock(build, fcntl.LOCK_EX | fcntl.LOCK_NB)
            fcntl.flock(integration, fcntl.LOCK_EX | fcntl.LOCK_NB)
            self.check()
        status = json.loads(self.cli("status").stdout)
        self.assertEqual(set(status["resources"]), {"ipad"})
        self.assertFalse((self.state / "build.owner.json").exists())

    def test_independent_checks_can_execute_at_the_same_time(self):
        ready = self.root / ".git/ready"
        ready.mkdir()
        release = self.root / ".git/release"
        self.control.write_text(json.dumps({"ready": str(ready), "release": str(release)}))
        processes = []
        try:
            for _ in range(2):
                process = subprocess.Popen([sys.executable, "-I", str(SCRIPT), "check", "--paths", "base.txt"],
                                           cwd=self.root, env=self.env, stdout=subprocess.PIPE,
                                           stderr=subprocess.PIPE, text=True)
                self.addCleanup(self.stop, process)
                processes.append(process)
            self.wait_for(lambda: len(list(ready.iterdir())) == 2)
        finally:
            release.touch()
        for process in processes:
            output, error = process.communicate(timeout=5)
            self.assertEqual(process.returncode, 0, output + error)

    def test_committed_batch_can_be_selected_by_baseline(self):
        (self.root / "base.txt").write_text("committed work\n")
        self.git("add", "base.txt")
        self.git("commit", "-qm", "batch")
        self.cli("check", "--base", self.base)
        self.assertEqual(self.count(), 1)

    def test_device_exclusion_release_and_commit_identity(self):
        evidence = self.root / "target/device.txt"
        evidence.parent.mkdir()
        held = self.hold("first", evidence)
        status = json.loads(self.cli("status").stdout)
        self.assertTrue(status["resources"]["ipad"]["busy"])
        self.assertEqual(status["resources"]["ipad"]["owner"]["task"], "first")
        self.cli("ipad", "second", "--device", "fixture", "--evidence", str(evidence), "--hold", ok=False)
        evidence.write_text("Fixture device evidence\n")
        output, error = held.communicate("pass\n", timeout=5)
        self.assertEqual(held.returncode, 0, output + error)
        self.assertEqual(self.device_receipt("first")["candidate"], self.base)
        self.assertTrue(self.device_receipt("first")["passed"])
        self.assertFalse(json.loads(self.cli("status").stdout)["resources"]["ipad"]["busy"])

    def test_device_requires_fresh_evidence_and_matching_clean_candidate(self):
        evidence = Path(self.tmp.name) / "device.txt"
        evidence.write_text("old evidence")
        self.cli("ipad", "old", "--device", "fixture", "--evidence", str(evidence), "--hold", ok=False)
        missing = Path(self.tmp.name) / "missing.txt"
        held = self.hold("missing", missing)
        held.communicate("pass\n", timeout=5)
        self.assertNotEqual(held.returncode, 0)
        self.assertFalse(self.device_receipt("missing")["passed"])
        self.git("commit", "--allow-empty", "-qm", "new candidate")
        self.cli("ipad", "wrong", "--candidate", self.base, "--device", "fixture",
                 "--evidence", str(missing), "--hold", ok=False)
        (self.root / "base.txt").write_text("dirty\n")
        self.cli("ipad", "dirty", "--device", "fixture", "--evidence", str(missing), "--hold", ok=False)

    def test_device_command_gets_candidate_and_source_mutation_rejects_pass(self):
        evidence = self.root / "target/command.txt"
        evidence.parent.mkdir()
        child = "import os,pathlib; pathlib.Path(" + repr(str(evidence)) + ").write_text(os.environ['SOLIDCRAFT_CANDIDATE'])"
        args = ["ipad", "command", "--device", "fixture", "--candidate", self.base, "--evidence", str(evidence)]
        self.cli(*args, "--", sys.executable, "-c", child)
        self.assertEqual(evidence.read_text(), self.base)
        held = self.hold("mutated", self.root / "target/mutated.txt")
        (self.root / "base.txt").write_text("changed\n")
        held.communicate("pass\n", timeout=5)
        self.assertNotEqual(held.returncode, 0)
        self.assertFalse(self.device_receipt("mutated")["passed"])

    def test_killed_wrapper_keeps_child_device_reservation_until_child_exits(self):
        pidfile = self.root / ".git/child.pid"
        child = "import os,pathlib,time; pathlib.Path(" + repr(str(pidfile)) + ").write_text(str(os.getpid())); time.sleep(20)"
        process = subprocess.Popen(
            [sys.executable, "-I", str(SCRIPT), "ipad", "child", "--device", "fixture", "--evidence",
             str(self.root / "target/absent.txt"), "--", sys.executable, "-c", child],
            cwd=self.root, env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        self.addCleanup(self.stop, process)
        self.wait_for(pidfile.exists)
        child_pid = int(pidfile.read_text())
        try:
            process.kill()
            process.wait(timeout=5)
            self.assertTrue(json.loads(self.cli("status").stdout)["resources"]["ipad"]["busy"])
        finally:
            os.kill(child_pid, signal.SIGTERM)
        process.communicate(timeout=5)
        self.assertFalse(json.loads(self.cli("status").stdout)["resources"]["ipad"]["busy"])
        self.assertFalse(self.device_receipt("child")["passed"])

    def test_unsafe_paths_names_and_invalid_waits_are_rejected(self):
        for path in ("../other", "/tmp/file", ":(glob)*"):
            self.cli("check", "--paths", path, ok=False)
        for wait in ("nan", "-1", "61"):
            self.cli("ipad", "fixture", "--device", "fixture", "--evidence", "target/no.txt", "--hold", "--wait", wait, ok=False)
        self.cli("ipad", "../unsafe", "--device", "fixture", "--evidence", "target/no.txt", "--hold", ok=False)


if __name__ == "__main__":
    unittest.main()
