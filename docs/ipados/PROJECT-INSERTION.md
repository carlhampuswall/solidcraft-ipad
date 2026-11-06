# Insert a project into another object

Activate the destination component in the Model browser, then open Projects.
Long-press a different project and choose **Insert into current project**.
Alternatively choose **Import file… → Insert into current project** and select a
`.sc`, `.solidcraft`, STEP, STL, OBJ or 3MF file in Files. With no component
active, insertion uses the design root.

A native project is inserted as a linked child component referencing the actual
source project. Inserting the same source again reuses its linked definition
with a separate occurrence and placement. Edit the source project to change its
parametric history; those saved changes refresh in every reference. The
containing project's own objects and history remain intact.

Insertion uses `component.insert_design` with `reuse: true`. The app checks
references after opening a project or returning to the foreground, and every
three seconds while idle. It uses the engine's file digest check and
`component.update_links` only for changed sources. A library source that was
renamed is found through its UUID directory and rebound with `component.relink`.
Folder moves, sorting and archiving do not change that identity. Repeated
instances keep their placements and share source updates.

Files sources retain a native bookmark in the private library's
`.reference-bookmarks.json`, and reads hold security scope and file-provider
coordination. Native insertion never points to a temporary import copy. The
source stays outside the containing project and is never rewritten by refresh.
Bookmarks are host access metadata; CAD references use the existing upstream
`DesignLink` format without new document fields or a format change.

Refresh waits during sketches, tools and numeric/model edits, and while Undo
has left a Redo choice pending. Unchanged sources create no undo entries. If a
source is missing, unreadable or invalid, its last evaluated display remains
and a warning is reported; refresh retries when it becomes available. The
existing engine linked-design support carries evaluated solid bodies and their
materials and appearances; it warns about omitted meshes and refuses projects
without solids or with failed source features. Source sketch history stays in
the source project.

Earlier app versions inserted independent snapshots. Those existing imports
remain independent because the temporary source path cannot establish which
original project was intended. Remove and reinsert one to establish a reference.
STEP and mesh imports continue to insert geometry.

Check a translated, active component as well as root insertion: the new child's
geometry must follow the destination placement. One Undo removes only the
insertion; Redo restores it. Change a source parameter, save, and return to the
containing project: the linked geometry and materials must update without
changing reference identities or placements. Save and cold-open to verify the
link persists. Rename a library source, then remove/restore it to check rebinding
and cached-display warnings. Invalid imports preserve the destination.

The focused adapter regressions are the `projects_` tests in
`crates/ipados/session/src/tests.rs`. Real shared-renderer evidence, including
before/insertion/source-update/cold-reopen PNGs, can be captured on a Mac with:

```sh
cargo +1.95.0 run --locked --profile ci -p solidcraft-ipados-bridge \
  --example render_proof -- target/project-insertion.png --project-insertion
```

This checks the macOS GPU path, not UIKit, physical iPad input or the native
project context menu. Verify those separately on an iPad.
