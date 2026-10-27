# Upstream compatibility and integration

Compatibility with future [SolidCraft upstream](https://github.com/tangency-labs/solidcraft)
updates is the highest architectural priority of
[this existing fork](https://github.com/carlhampuswall/solidcraft-ipad).
Keep a small, reviewable platform delta and retain upstream commit ancestry.

## Permanent engineering rules

1. Preserve SolidCraft's existing Rust CAD engine. Follow upstream engine and
   kernel evolution through their existing boundaries, rather than freezing a
   copied implementation.
2. Never duplicate or replace the geometry kernel or sketch solver for iPadOS.
   Use `crates/kernel`, `crates/geom` and `crates/sketch`; respect upstream's rule
   that only the kernel crate accesses its kernel backend.
3. Reuse `crates/doc`'s parametric feature timeline and `crates/engine`'s command
   registry. All CAD operations go through the existing engine; a native UI must
   not implement a parallel command system, document evaluator or file format.
4. Isolate iPad-specific code in `apps/solidcraft-ipados/` and future adapters
   under `crates/ipados/`. Keep platform documentation in `docs/ipados/`.
5. Minimize modifications to upstream crates. Prefer small public boundary
   improvements that can be proposed upstream separately; explain any unavoidable
   shared change and its compatibility impact in its PR.
6. Maintain desktop and web compatibility. Shared crates must not depend on the
   iPad host, Apple-only SDKs or iPad adapter crates. Use target-specific
   dependencies and feature gates at the platform boundary.
7. Prefer upstream-friendly adapters over invasive patches. Do not copy engine
   internals, move upstream crates, restructure the workspace or introduce a new
   CAD engine to accommodate a platform host.

Root AGENTS.md and the upstream contribution, licensing, crash-safety, layering
and testing rules continue to apply. Native host technology and any narrowly
scoped language/FFI exception require an explicit implementation decision; this
preparation grants no blanket exception to upstream rules.

## Fetch and prepare an integration

Use the fork's `main` as the integration base, never overwrite it with upstream.
Keep `feature/ipados` for platform development. Verify remotes with `git remote -v`:

```text
origin    https://github.com/carlhampuswall/solidcraft-ipad.git
upstream  https://github.com/tangency-labs/solidcraft.git
```

Start with a clean working tree, then:

```sh
git fetch --no-tags origin main
git fetch --no-tags upstream main
git log --oneline origin/main..upstream/main
git diff --stat origin/main...upstream/main
git switch -c integration/upstream-YYYYMMDD origin/main
git merge --no-ff --no-commit upstream/main
```

Use a fresh branch name. Review upstream release notes, dependency and licence
changes, document-format changes and command API changes. Resolve conflicts by
preserving upstream behavior and adapting the platform boundary. Never resolve
everything with `ours` or `theirs`. Use `git merge --abort` to abandon a conflicted
attempt safely. Do not hand-edit generated files; follow the upstream pending-file
and generation process. Record any unresolved CI/runner setup before proceeding.

## Validation and review

Before committing a manual merge, run the relevant checks from CONTRIBUTING.md
and `docs/book/src/dev/testing.md` on the actual candidate, including:

- `cargo fmt --all -- --check` and affected-crate tests/Clippy first.
- `cargo xtask ci` as the complete upstream gate before approval, including
  engine tests, repository/layer checks, Clippy and wasm compatibility.
- `cargo xtask web-offline-test` for web integration and the relevant desktop
  smoke checks. Record platform coverage and any unavailable runners explicitly.
- Once an iPad target exists, build the adapter and native app with the selected
  SDK, and verify command execution, timeline rebuilds and document round trips
  on a physical iPad. Do not launch simulators without an explicit request.

Then commit, push **only to origin**, and open an integration PR with base `main`.
Use a merge commit when integrating upstream, not a squash or a history rewrite:
upstream commit ancestry is how the next update is recognized as integrated.
Record base/upstream/candidate SHAs, conflicts, tests, desktop/web/iPad coverage
and outstanding risks. Review the permanent rules before approval. A green
update-detection workflow is not engine validation.

An authorized reviewer must approve and merge only after required checks pass on
the candidate being merged. If the candidate changes, repeat affected checks.
Use branch protection/required checks on `main` to enforce this gate; this
preparation does not change repository protection settings. After merging, fetch
origin and integrate the tested `origin/main` into `feature/ipados` in a reviewed
change, then rerun affected adapter and compatibility checks. Never force-push
`main` or discard platform work to catch up.

## Lightweight GitHub Actions workflow

[upstream-sync.yml](../../.github/workflows/upstream-sync.yml) runs weekly on
Monday at 07:23 UTC and can be dispatched manually. It fetches upstream `main`,
checks ancestry against the fork's `main`, and reports the SHAs and number of
unintegrated upstream commits in the Actions run summary. Scheduled runs only
detect updates and publish no branches or PRs.

To request a draft integration PR after activation:

```sh
gh workflow run upstream-sync.yml --repo carlhampuswall/solidcraft-ipad \
  --ref main -f create_pr=true
```

The workflow first checks for an existing open `integration/upstream-*` PR. If
none exists, it attempts an ordinary merge on a unique integration branch and
opens a **draft** PR against `main`. A conflict fails the run with a summary and
publishes nothing; resolve that update using the manual process above. It never
pushes `main`, force-pushes, changes `feature/ipados`, approves a PR, enables
auto-merge or runs fetched upstream code. A successful PR creation run leaves an
untested candidate for human review. If PR creation fails after the branch push,
the branch remains available for a maintainer to open the draft PR manually.

### Workflow activation and credentials

- Merge this preparation into the fork's default branch before schedule or manual
  dispatch is available. It remains inactive while present only on `feature/ipados`.
- Enable Actions for the fork and permit the pinned checkout action. For the
  default `GITHUB_TOKEN`, enable **Settings > Actions > General > Allow GitHub
  Actions to create and approve pull requests**. The workflow itself never
  approves PRs; it requests only contents and pull-request write permissions.
  See [GitHub's repository Actions settings](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/enabling-features-for-your-repository/managing-github-actions-settings-for-a-repository).
- Detection needs no extra secret. To publish an integration that changes
  `.github/workflows/`, supply repository secret `UPSTREAM_SYNC_TOKEN`, scoped to
  this fork with Contents, Pull requests and Workflows write access (or a classic
  token with `repo` and `workflow` scopes). It is used for both checkout/push and
  PR creation. Never put credentials in committed files. Without it, workflow-file
  updates stop before pushing and must be handled manually. See
  [GitHub's guidance on additional token permissions](https://docs.github.com/en/actions/tutorials/authenticate-with-github_token#granting-additional-permissions).
- Draft PRs skip the inherited main CI selection. Replace the PR body's
  `Changes a default:` review placeholder with the required no/yes declaration
  after inspecting upstream changes. After review, mark the PR ready,
  approve any pending workflow runs and start the appropriate CI on its exact
  head. `GITHUB_TOKEN` events have
  [special workflow-trigger behavior](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow);
  verify that checks actually ran. Inherited CI also gates heavy checks behind
  `approved-for-ci` and uses Blacksmith runners. Configure runner access or make
  a separate, reviewed fork CI adaptation before relying on those checks. Manual
  CI can be dispatched at the integration branch once usable runners exist.
- Protect `main` with review and required validation checks; do not infer safety
  from a draft PR or the detector's green status. Secrets, runner access and branch
  protection are repository administration follow-ups, not configured by this commit.
