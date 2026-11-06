# Insert a project into another object

Activate the destination component in the Model browser, then open Projects.
Long-press a different project and choose **Insert into current project**.
Alternatively choose **Import file… → Insert into current project** and select a
`.sc`, `.solidcraft`, STEP, STL, OBJ or 3MF file in Files. With no component
active, insertion uses the design root.

A native project is inserted as a new child component containing a snapshot of
its evaluated solid bodies and their materials and appearances. The destination
stays open, and its existing objects and history remain intact. The source
project's sketches, parametric history and assembly hierarchy are not copied;
mesh bodies are omitted with the existing engine import warning. A project with
no solid bodies, corrupt contents or failed features is refused.

Native insertion uses the existing `component.insert_design` command with
`link: false`. This optional engine parameter defaults to `true`, retaining the
desktop/web command's linked-design behavior. Snapshot insertion removes the
link after the existing property inheritance within the same command. No new
document fields, format or iPad-only engine dependency are introduced. A later
source-file edit or deletion therefore cannot change the inserted object, and
undo/redo never restores a link to the temporary Files copy. `link: false` is
incompatible with `reuse: true`, and non-boolean `link` values are rejected.

Check a translated, active component as well as root insertion: the new child's
geometry must follow the destination placement. One Undo removes only the
insertion; Redo restores it. Save and cold-open the destination after removing
the source to verify the snapshot is self-contained. Invalid imports must leave
the destination's objects, active component and undo/redo history intact.

The focused adapter regressions are the `projects_` tests in
`crates/ipados/session/src/tests.rs`. Real shared-renderer evidence, including
before/insertion/cold-reopen PNGs, can be captured on a Mac with:

```sh
cargo +1.95.0 run --locked --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- target/project-insertion.png --project-insertion
```

This checks the macOS GPU path, not UIKit, physical iPad input or the native
project context menu. Verify those separately on an iPad.
