# Native Apple host scope

Read `../../docs/ipados/ARCHITECTURE.md` and `../../docs/ipados/UPSTREAM.md` first.
The Step 2 platform exception permits Swift/UIKit only in this directory for
application lifecycle, a native Metal layer, typed input and native file services.
All modelling, document evaluation, command semantics, geometry and rendering
remain in the existing Rust crates through `crates/ipados/` adapters.

- Keep this directory excluded from Cargo workspace membership: it is an Xcode
  project, not a Rust package.
- Never add a second camera controller. Input retains Pencil/finger identity;
  Step 3 must connect navigation to the authoritative shared Rust camera path.
- UIKit objects and layer geometry are managed on the main thread. One serial
  queue owns CAD session calls. Renderer attach/resize/draw/destroy calls share a
  render-only lock with synchronous main-thread resign-active suspension. Never
  hold this lock while evaluating CAD, and never rely on background-task time to
  authorise Metal submissions. Retain the layer until Rust has destroyed or
  replaced its surface.
- Do not use force unwraps, `try!`, `fatalError` or force casts. Display structured
  bridge errors. Discard callbacks after shutdown using a generation token.
- Use system semantic colours for the small native host chrome. This is the
  scoped native equivalent of theme tokens; do not duplicate the CAD theme.
- New messages originate in `assets/locales/pending/`; the host extracts its
  small catalogue at build time from canonical and pending catalogues, and falls
  back to English offline. Python is build tooling only, never a runtime dependency.
- Use `scripts/build.sh` for unsigned device compilation. Never launch simulators
  or change signing identities, credentials or developer settings without an
  explicit user request. Compile-only results do not verify runtime or Pencil.
