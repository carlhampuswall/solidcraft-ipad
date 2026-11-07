# Tool labels

The iPad workspace shows tool and menu-button names in separate captions beside
their icons. View and History use the same external caption style. Names come
from the existing offline translations, including tools added to quick access.

Use **Settings > Show tool labels** to switch between captions and compact icons.
The preference is enabled by default, applies immediately, and persists across
launches. VoiceOver names, pointer tooltips and menu action text remain available
with captions hidden. Native menu rows keep their existing text.

Changes a default: yes — workspace captions are visible; compact icons remain
available through **Show tool labels**.

The deterministic workspace harness covers the default, compact fallback and
caption layout at different sizes. The unsigned device build checks UIKit
compilation. Debug argument `--tool-labels-check` exercises the real Settings action,
checks external caption placement and saved visibility, and captures shown,
hidden and restored states in the app's Documents folder. Its synthetic actions
do not verify physical finger or Pencil delivery.
