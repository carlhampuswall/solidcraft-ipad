#!/usr/bin/env python3
"""Local task delivery and resource locks. Python standard library; no daemon."""

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import time


class ProtocolError(Exception):
    pass


def now():
    return datetime.now(timezone.utc).isoformat()


def atomic_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode="w", dir=path.parent, delete=False) as stream:
        temporary = Path(stream.name)
        json.dump(value, stream, indent=2)
        stream.write("\n")
    try:
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


def slug(value):
    if not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*", value):
        raise argparse.ArgumentTypeError("Use a lowercase task name with optional hyphens.")
    return value


def check_plan(paths, full=False):
    """Fail closed on shared or unfamiliar code; scope known iPad-only changes."""
    def routine(path):
        return (
            path.endswith(".md")
            or path.startswith(("docs/", "skills/", "scripts/ipados/", "apps/solidcraft-ipados/",
                                "crates/ipados/", "assets/", "attribution/pending/"))
        )

    if full or any(not routine(path) for path in paths):
        return "full", [["cargo", "+1.95.0", "xtask", "ci"]]
    commands = [["cargo", "+1.95.0", "fmt", "--all", "--", "--check"]]
    commands += [["cargo", "+1.95.0", "xtask", name]
                 for name in ("assets", "icons", "lists", "skills", "layers", "ui-containers")]
    commands.append(["cargo", "+1.95.0", "xtask", "licences", "--check"])
    if any(path.startswith("scripts/ipados/") for path in paths):
        commands.append([sys.executable, "-I", "scripts/ipados/test_task_protocol.py"])
    rust = any(path.startswith("crates/ipados/") and not path.endswith(".md") for path in paths)
    native = rust or any(
        (path.startswith("apps/solidcraft-ipados/") and not path.endswith(".md"))
        or path.startswith("assets/locales/pending/ipados-") for path in paths
    )
    if rust:
        packages = ["-p", "solidcraft-ipados-session", "-p", "solidcraft-ipados-bridge"]
        commands += [
            ["cargo", "+1.95.0", "test", "--locked", "--profile", "ci", *packages, "--lib"],
            ["cargo", "+1.95.0", "clippy", "--locked", *packages, "--all-targets", "--no-deps", "--", "-D", "warnings"],
        ]
    if native:
        commands += [["sh", "apps/solidcraft-ipados/scripts/build.sh"],
                     ["sh", "apps/solidcraft-ipados/scripts/check-workspace.sh"]]
    return "affected", commands


class Repository:
    def __init__(self, root="."):
        self.root = Path(root).resolve()
        self.root = Path(self.git("rev-parse", "--show-toplevel").strip())
        self.common = Path(self.git("rev-parse", "--path-format=absolute", "--git-common-dir").strip())
        self.state = self.common / "solidcraft-tasks"

    def git(self, *args, root=None, check=True):
        result = subprocess.run(
            ["git", "-C", str(root or self.root), *args],
            text=True, capture_output=True,
        )
        if check and result.returncode:
            raise ProtocolError(result.stderr.strip() or result.stdout.strip())
        return result.stdout if check else result

    def sha(self, revision="HEAD", root=None):
        return self.git("rev-parse", "--verify", "--end-of-options", revision + "^{commit}", root=root).strip()

    def branch(self, root=None):
        return self.git("symbolic-ref", "--quiet", "--short", "HEAD", root=root).strip()

    def worktrees(self):
        result = []
        for field in self.git("worktree", "list", "--porcelain", "-z").split("\0"):
            if field.startswith("worktree "):
                result.append({"path": field[9:]})
            elif field.startswith("branch "):
                result[-1]["branch"] = field[7:]
        return result

    def clean(self, root=None):
        if self.git("status", "--porcelain", "--untracked-files=all", root=root).strip():
            raise ProtocolError(f"Checkout has uncommitted work: {root or self.root}. Preserve it; do not auto-stash.")
        for name in ("MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "rebase-merge", "rebase-apply"):
            path = Path(self.git("rev-parse", "--path-format=absolute", "--git-path", name, root=root).strip())
            if path.exists():
                raise ProtocolError(f"Finish or explicitly abandon the existing Git operation: {path}")

    def ancestor(self, older, newer):
        result = self.git("merge-base", "--is-ancestor", older, newer, check=False)
        if result.returncode not in (0, 1):
            raise ProtocolError(result.stderr.strip())
        return result.returncode == 0

    def develop_checkout(self):
        matches = [Path(w["path"]) for w in self.worktrees() if w.get("branch") == "refs/heads/develop"]
        if len(matches) > 1:
            raise ProtocolError("Multiple develop checkouts; resolve ownership before integrating.")
        return matches[0] if matches else None

    def fingerprint(self):
        digest = hashlib.sha256()
        for value in (self.sha(), self.branch(), self.git("write-tree"),
                      self.git("diff", "--binary", "HEAD", "--"),
                      self.git("status", "--porcelain", "--untracked-files=all")):
            digest.update(value.encode())
        for name in self.git("ls-files", "--others", "--exclude-standard", "-z").split("\0"):
            if name:
                path = self.root / name
                digest.update(name.encode())
                digest.update(os.readlink(path).encode() if path.is_symlink() else path.read_bytes())
        return digest.hexdigest()

    def record_path(self, task):
        return self.state / "tasks" / (slug(task) + ".json")

    def load(self, task):
        path = self.record_path(task)
        if not path.exists():
            raise ProtocolError(f"Unknown task {task}; use start or adopt first.")
        return json.loads(path.read_text())

    def save(self, record):
        record["updated"] = now()
        atomic_json(self.record_path(record["task"]), record)

    def owned(self, task):
        record = self.load(task)
        if Path(record["worktree"]).resolve() != self.root or self.branch() != record["branch"]:
            raise ProtocolError("Run this command in the task's recorded worktree and branch.")
        return record

    @contextmanager
    def lock(self, resource, task, wait=0):
        self.state.mkdir(parents=True, exist_ok=True)
        lockpath = self.state / (resource + ".lock")
        # Never unlink lock files: every contender must lock the same inode.
        with lockpath.open("a+") as stream:
            deadline = time.monotonic() + wait
            announced = False
            while True:
                try:
                    fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    if not announced:
                        print(f"Waiting for {resource}; inspect task-protocol status for its owner.", flush=True)
                        announced = True
                    if time.monotonic() >= deadline:
                        raise ProtocolError(f"{resource} is busy. Continue independent work, then retry.")
                    time.sleep(min(0.2, max(0, deadline - time.monotonic())))
            atomic_json(self.state / (resource + ".owner.json"),
                        {"task": task, "pid": os.getpid(), "worktree": str(self.root), "acquired": now()})
            try:
                yield stream.fileno()
            finally:
                # The owner file is diagnostic only; flock is authoritative.
                fcntl.flock(stream, fcntl.LOCK_UN)

    def create_record(self, task, ipad, reason, base=None):
        if self.record_path(task).exists():
            raise ProtocolError(f"Task {task} already exists; inspect its record instead of replacing it.")
        branch = self.branch()
        if not branch.startswith("codex/"):
            raise ProtocolError("Tasks must use a separate codex/ branch, never develop or main.")
        if ipad == "not-needed" and not reason:
            raise ProtocolError("--reason is required when physical iPad testing is not needed.")
        record = {
            "version": 1, "task": task, "worktree": str(self.root), "branch": branch,
            "base": base or self.git("merge-base", "HEAD", "develop").strip(),
            "tip": self.sha(), "state": "working", "ipad_required": ipad == "required",
            "ipad_reason": reason, "checks": [], "integrated": None, "verified": None, "device": None, "note": "",
        }
        self.save(record)
        return record

    def start(self, args):
        with self.lock("integration", args.task, args.wait):
            if self.record_path(args.task).exists():
                raise ProtocolError("Task name already registered.")
            if args.ipad == "not-needed" and not args.reason:
                raise ProtocolError("--reason is required when physical iPad testing is not needed.")
            develop = self.develop_checkout()
            if develop:
                self.clean(develop)
            base = self.sha("develop")
            primary = Path(self.worktrees()[0]["path"])
            path = Path(args.path).resolve() if args.path else primary / "target/task-worktrees" / args.task
            if path.exists():
                raise ProtocolError(f"Worktree path already exists: {path}")
            self.git("worktree", "add", "-b", "codex/" + args.task, str(path), base)
            task_repo = Repository(path)
            record = task_repo.create_record(args.task, args.ipad, args.reason, base)
            remote = self.git("rev-parse", "--verify", "refs/remotes/origin/develop", check=False)
            record["observed_origin_develop"] = remote.stdout.strip() if remote.returncode == 0 else None
            task_repo.save(record)
            print(json.dumps(record, indent=2))

    def run_checks(self, record, extra, fd, wait, full=False, base=None):
        self.git("diff", "--check", "develop", "--")
        base = base or self.git("merge-base", "HEAD", "develop").strip()
        paths = set(self.git("diff", "--name-only", "-z", base, "--").split("\0"))
        paths.update(self.git("ls-files", "--others", "--exclude-standard", "-z").split("\0"))
        paths.discard("")
        scope, commands = check_plan(sorted(paths), full)
        commands += [shlex.split(c) for c in extra]
        record["validation_scope"] = scope
        record["changed_paths"] = sorted(paths)
        self.save(record)
        before = self.fingerprint()
        checks = []
        environment = dict(os.environ)
        environment.setdefault("SOLIDCRAFT_TEST_JOBS", "2")
        environment.setdefault("CARGO_BUILD_JOBS", "2")
        if sys.version_info >= (3, 11):
            environment["PATH"] = str(Path(sys.executable).parent) + os.pathsep + environment.get("PATH", "")
        print(f"Validation scope: {scope}; {len(commands)} checks.", flush=True)
        with self.lock("build", record["task"], wait) as build_fd:
            inherited = (build_fd,) if fd is None else (fd, build_fd)
            for command in commands:
                if not command:
                    raise ProtocolError("Empty check command.")
                result = subprocess.run(command, cwd=self.root, env=environment, pass_fds=inherited)
                checks.append({"command": command, "exit_code": result.returncode, "at": now(),
                               "test_jobs": environment["SOLIDCRAFT_TEST_JOBS"],
                               "build_jobs": environment["CARGO_BUILD_JOBS"]})
                record["checks"] = checks
                self.save(record)
                if result.returncode:
                    raise ProtocolError(f"Required check failed ({result.returncode}): {shlex.join(command)}")
                if self.fingerprint() != before:
                    raise ProtocolError("Source or index changed during verification; review and rerun checks.")

    def check(self, args):
        record = self.owned(args.task)
        record["verified"] = None
        self.save(record)
        self.run_checks(record, args.check, None, args.wait, args.full)
        print("Checks passed. Review, stage only owned paths and commit; integrate before reporting done.")

    def build(self, args):
        self.owned(args.task)
        if not args.command:
            raise ProtocolError("Supply a build command after --.")
        environment = dict(os.environ)
        environment.setdefault("CARGO_BUILD_JOBS", "2")
        environment.setdefault("SOLIDCRAFT_TEST_JOBS", "2")
        with self.lock("build", args.task, args.wait) as fd:
            result = subprocess.run(args.command, cwd=self.root, env=environment, pass_fds=(fd,))
            if result.returncode:
                raise ProtocolError(f"Build command failed ({result.returncode}).")

    def integrate(self, args):
        record = self.owned(args.task)
        self.clean()
        with self.lock("integration", args.task, args.wait) as fd:
            old = self.sha("develop")
            develop = self.develop_checkout()
            if develop:
                self.clean(develop)
            record["verified"] = None
            self.save(record)
            # Merge the latest develop into the isolated task, never into a dirty shared checkout.
            if not self.ancestor(old, self.sha()):
                result = self.git("merge", "--no-ff", "--no-commit", old, check=False)
                if result.returncode:
                    raise ProtocolError("Merge needs review in this task worktree; develop was not changed.\n" + result.stderr.strip())
            record["state"] = "verifying"
            self.save(record)
            self.run_checks(record, args.check, fd, args.wait, args.full, old)
            verified_tree = self.git("write-tree").strip()
            merge_head = Path(self.git("rev-parse", "--path-format=absolute", "--git-path", "MERGE_HEAD").strip())
            if merge_head.exists():
                self.git("commit", "-m", f"Integrate develop into {record['branch']}")
            self.clean()
            tip = self.sha()
            if self.git("rev-parse", "HEAD^{tree}").strip() != verified_tree:
                raise ProtocolError("Commit hooks changed the verified tree; rerun integration checks.")
            if self.sha("develop") != old or self.develop_checkout() != develop:
                raise ProtocolError("develop or its checkout moved during verification; retry integration.")
            if develop:
                self.clean(develop)
                self.git("merge", "--ff-only", tip, root=develop)
            else:
                self.git("update-ref", "refs/heads/develop", tip, old)
            if not self.ancestor(tip, self.sha("develop")):
                raise ProtocolError("Task ancestry was not integrated.")
            record.update(tip=tip, integrated=tip, verified=tip, state="integrated", device=None)
            record.pop("blocker", None)
            self.save(record)
            print(f"Integrated {args.task} into develop at {tip}. Run finish after required device testing.")

    def ipad(self, args):
        record = self.owned(args.task)
        self.clean()
        tip = self.sha()
        if record["integrated"] != tip or record.get("verified") != tip or not self.ancestor(tip, self.sha("develop")):
            raise ProtocolError("Integrate the committed task before testing its candidate on the iPad.")
        command = args.command[1:] if args.command[:1] == ["--"] else args.command
        if not args.hold and not command:
            raise ProtocolError("Supply --hold or a command after -- for the complete device session.")
        if args.hold and command:
            raise ProtocolError("--hold and a command are alternatives.")
        evidence = Path(args.evidence).resolve()
        if evidence.exists():
            raise ProtocolError("Use a fresh evidence path so an old result cannot count as this test.")
        before = self.fingerprint()
        record["device"] = None
        self.save(record)
        with self.lock("ipad", args.task, args.wait) as fd:
            print(f"IPAD RESERVED: task={args.task} device={args.device} candidate={tip}", flush=True)
            environment = dict(os.environ, SOLIDCRAFT_CANDIDATE=tip, SOLIDCRAFT_TASK=args.task,
                               SOLIDCRAFT_DEVICE=args.device)
            if args.hold:
                print("Keep this process open through install, test and evidence capture. Type pass or fail to release.", flush=True)
                verdict = sys.stdin.readline().strip()
                passed = verdict == "pass"
            else:
                passed = subprocess.run(command, cwd=self.root, env=environment, pass_fds=(fd,)).returncode == 0
            if self.fingerprint() != before:
                raise ProtocolError("Source changed during device testing; no successful test was recorded.")
            passed = passed and evidence.is_file() and evidence.stat().st_size > 0
            record["device"] = {"candidate": tip, "device": args.device, "passed": passed,
                                "evidence": str(evidence), "at": now(), "manual": args.hold}
            self.save(record)
            if not passed:
                raise ProtocolError("Device session failed or has no new nonempty evidence file.")
            print("Device result recorded; iPad released.")

    def finish(self, args):
        record = self.owned(args.task)
        self.clean()
        tip = self.sha()
        if record["integrated"] != tip or record.get("verified") != tip or not self.ancestor(tip, self.sha("develop")):
            raise ProtocolError("Integration pending: this task tip must be verified and merged into develop.")
        device = record["device"]
        if device and not device["passed"]:
            raise ProtocolError("Testing pending: the last requested iPad session failed.")
        if record["ipad_required"] and not (device and device["passed"] and device["candidate"] == tip):
            raise ProtocolError("Testing pending: this integrated candidate needs a passing iPad session.")
        record["state"] = "done"
        record.pop("blocker", None)
        self.save(record)
        print(f"DONE: {args.task}; verified task {tip} is in develop.")

    def status(self):
        tasks = [json.loads(p.read_text()) for p in sorted((self.state / "tasks").glob("*.json"))]
        resources = {}
        self.state.mkdir(parents=True, exist_ok=True)
        for name in ("integration", "build", "ipad"):
            with (self.state / (name + ".lock")).open("a+") as stream:
                try:
                    fcntl.flock(stream, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    fcntl.flock(stream, fcntl.LOCK_UN)
                    resources[name] = {"busy": False}
                except BlockingIOError:
                    owner = self.state / (name + ".owner.json")
                    resources[name] = {"busy": True, "owner": json.loads(owner.read_text()) if owner.exists() else None}
        print(json.dumps({"resources": resources, "tasks": tasks}, indent=2))


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    sub = result.add_subparsers(dest="action", required=True)
    for name in ("start", "adopt", "build", "check", "integrate", "ipad", "finish", "note"):
        command = sub.add_parser(name)
        command.add_argument("task", type=slug)
        if name in ("start", "adopt"):
            command.add_argument("--ipad", choices=("required", "not-needed"), default="required")
            command.add_argument("--reason", default="")
        if name in ("start", "build", "check", "integrate", "ipad"):
            command.add_argument("--wait", type=float, default=0, help="Wait at most 60 seconds, then retry later.")
        if name == "start":
            command.add_argument("--path", help="New worktree path; default: primary checkout target/task-worktrees/TASK")
        if name in ("check", "integrate"):
            command.add_argument("--check", action="append", default=[], help="Extra check command, parsed without a shell.")
            command.add_argument("--full", action="store_true", help="Full CI checkpoint, required before main/releases.")
        if name == "ipad":
            command.add_argument("--device", required=True)
            command.add_argument("--evidence", required=True)
            command.add_argument("--hold", action="store_true")
        if name == "note":
            command.add_argument("text")
    sub.add_parser("status")
    return result


def main(argv=None):
    arguments = list(sys.argv[1:] if argv is None else argv)
    separator = arguments.index("--") if "--" in arguments else len(arguments)
    args = parser().parse_args(arguments[:separator])
    if separator < len(arguments) and args.action not in ("ipad", "build"):
        raise ProtocolError("Only ipad/build accept a command after --.")
    if args.action in ("ipad", "build"):
        args.command = arguments[separator + 1:]
    if hasattr(args, "wait") and not 0 <= args.wait <= 60:
        raise ProtocolError("--wait must be between 0 and 60 seconds.")
    repo = Repository()
    if args.action == "start":
        repo.start(args)
    elif args.action == "adopt":
        with repo.lock("integration", args.task):
            print(json.dumps(repo.create_record(args.task, args.ipad, args.reason), indent=2))
    elif args.action == "status":
        repo.status()
    else:
        with repo.lock("task-" + args.task, args.task):
            try:
                if args.action == "note":
                    record = repo.owned(args.task)
                    record["note"] = args.text
                    repo.save(record)
                else:
                    getattr(repo, args.action)(args)
            except (ProtocolError, OSError) as error:
                record = repo.owned(args.task)
                record["state"] = "pending"
                record["blocker"] = str(error)
                repo.save(record)
                raise
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (ProtocolError, OSError, ValueError) as error:
        print(f"task-protocol: {error}", file=sys.stderr)
        sys.exit(1)
