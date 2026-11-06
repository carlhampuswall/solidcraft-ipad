# Project organisation

In Projects, **Organise** offers **New folder**, **Sort** and **Show Archive**.
Tap a folder to open it; **Back** returns to its parent. New and duplicated
projects are created in the folder currently open. Long-press a project or folder
for **Move to folder** and **Archive**. Folder menus also offer **Rename folder**
and **Folder icon**, using the existing Folder, Project, Library, Cube and
Component artwork. Swipe actions provide Archive/Restore and project Rename.

Sorting is saved separately for each folder, Projects and Archive. The default
remains recently modified; name sorting uses natural order, including numbers.
Folders appear before projects. Search includes descendants of the current
folder; at Projects or Archive it searches that entire collection.

Archiving a folder hides its subtree from Projects. Archive retains the hierarchy
and supports opening designs. Restore returns an item to its original folder
when that folder is active. Restoring one item from inside an archived folder
returns that item to Projects. Restoring a whole folder preserves its hierarchy
and leaves separately archived descendants archived. Archiving the current
project keeps its open session and autosave destination.

## Storage and ownership

`ProjectLibrary.swift` stores versioned host metadata in
`SolidCraft/Projects/.organisation.json`: folder UUIDs, parent relationships,
names, icon IDs, project placements, archive flags and sorting choices. The
existing serial `NativeSession` project queue owns reads and atomic writes.
Missing metadata means the existing flat library; no migration is required.
Invalid metadata is reported and retained rather than replaced.

Project UUID directories, design filenames, native containers, recovery sidecars
and relative component links stay in place during folder moves and archiving.
This is native file organisation, with no CAD document-format change or new
engine command. Metadata is limited to 4 MiB, 10,000 folders, 100,000 placements
and 64 ancestor links. Unknown parents, cycles, duplicate folder IDs and symbolic
links used as the metadata file are rejected.

The deterministic workspace harness covers nested membership, natural sorting,
independent sort preferences, search scope, folder icons and renaming, moves,
archive/restore inheritance, persistence, unchanged design/recovery bytes and
invalid metadata. The unsigned device build checks UIKit integration; it does
not establish physical-device or touch behaviour.
