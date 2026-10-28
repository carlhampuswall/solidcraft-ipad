import CoreGraphics
import Foundation

@main
struct WorkspaceChecks {
    static func require(_ condition: @autoclosure () -> Bool, _ message: String) throws {
        if !condition() { throw NSError(domain: "WorkspaceChecks", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    }

    static func main() throws {
        // No simulator or UIKit runtime. Verify the geometry policy used by the
        // controller, including resizable windows and keyboard-reduced bounds.
        var cases = 0
        for size in [CGSize(width: 1194, height: 790), CGSize(width: 1024, height: 700),
                     CGSize(width: 834, height: 1140), CGSize(width: 390, height: 760),
                     CGSize(width: 320, height: 480), CGSize(width: 1194, height: 360)] {
            for largeText in [false, true] {
                for railRight in [false, true] {
                    for panels in [(false, false), (true, false), (false, true), (true, true)] {
                        let layout = WorkspaceLayout.resolve(size: size, browser: panels.0, inspector: panels.1,
                                                             railRight: railRight, largeText: largeText, compactChrome: false)
                        let bounds = CGRect(origin: .zero, size: size)
                        for frame in [layout.topBar, layout.status, layout.rail, layout.canvas] + [layout.browser, layout.inspector].compactMap({ $0 }) {
                            try require(frame.width >= 0 && frame.height >= 0, "negative frame")
                            try require(bounds.contains(frame), "chrome outside available window: \(size), \(frame)")
                        }
                        try require(!layout.canvas.intersects(layout.rail), "rail overlaps rendering/picking bounds")
                        try require(!layout.canvas.intersects(layout.topBar), "toolbar overlaps rendering/picking bounds")
                        try require(!layout.canvas.intersects(layout.status), "status overlaps rendering/picking bounds")
                        if layout.compact {
                            try require(layout.browser == nil || layout.inspector == nil, "compact presents two panels")
                            try require(layout.topBar.height >= (largeText ? 128 : 104), "compact controls lack second row")
                        } else {
                            try require(layout.canvas.width >= 360, "wide layout leaves insufficient model area")
                            if let frame = layout.browser { try require(!layout.canvas.intersects(frame), "docked browser overlaps picking") }
                            if let frame = layout.inspector { try require(!layout.canvas.intersects(frame), "docked inspector overlaps picking") }
                        }
                        let mirror = WorkspaceLayout.resolve(size: size, browser: panels.0, inspector: panels.1,
                                                             railRight: !railRight, largeText: largeText, compactChrome: false)
                        try require(layout.canvas.width == mirror.canvas.width, "handedness changes usable viewport width")
                        try require(layout.rail.minX == size.width - mirror.rail.maxX, "rail placement does not mirror")
                        cases += 1
                    }
                }
            }
        }
        for invalid in [CGSize.zero, CGSize(width: -10, height: -20), CGSize(width: CGFloat.nan, height: CGFloat.infinity)] {
            let layout = WorkspaceLayout.resolve(size: invalid, browser: true, inspector: true, railRight: false, largeText: true, compactChrome: false)
            try require(layout.canvas.width.isFinite && layout.canvas.height.isFinite, "invalid dimensions escaped")
            try require(layout.canvas.width >= 0 && layout.canvas.height >= 0, "invalid dimensions became negative")
        }
        // Device inspection exposed standard-size tool words being split across
        // syllables in the old64pt rail. Give the production text-labelled rail
        // its full width while retaining the minimum canvas at docking thresholds.
        for largeText in [false, true] {
            for width in [CGFloat(834), 1000, 1024, 1120, 1194] {
                let layout = WorkspaceLayout.resolve(size: CGSize(width: width, height: 790), browser: true, inspector: true,
                                                     railRight: false, largeText: largeText, compactChrome: false)
                try require(layout.rail.width == (largeText ? 160 : 112), "tool rail compresses text-labelled controls")
                if !layout.compact { try require(layout.canvas.width >= 360, "wider tool labels squeezed the docked canvas") }
                cases += 1
            }
        }
        // iPad window resizing and the keyboard can briefly leave almost no
        // client area. Chrome can collapse, but cannot create invalid surfaces.
        for width in [1, 44, 68, 80, 150, 280, 1000] {
            for height in [1, 55, 100, 150, 300] {
                for railRight in [false, true] {
                    let size = CGSize(width: width, height: height)
                    let layout = WorkspaceLayout.resolve(size: size, browser: true, inspector: true,
                                                         railRight: railRight, largeText: true, compactChrome: false)
                    for frame in [layout.topBar, layout.status, layout.rail, layout.canvas] + [layout.browser, layout.inspector].compactMap({ $0 }) {
                        try require(frame.width.isFinite && frame.height.isFinite, "transient size created invalid drawable")
                        try require(frame.width >= 0 && frame.height >= 0, "transient size created negative drawable")
                        try require(frame.minX >= 0 && frame.minY >= 0 && frame.maxX <= size.width && frame.maxY <= size.height,
                                    "transient chrome escaped bounds")
                    }
                    cases += 1
                }
            }
        }
        // The controller supplies real scaled UIFont line heights. Large text
        // and the keyboard can consume the entire client height; the remaining
        // Metal surface must collapse cleanly rather than overlap the chrome.
        for item in [(CGSize(width: 390, height: 760), CGFloat(180), CGFloat(96)),
                     (CGSize(width: 320, height: 340), CGFloat(220), CGFloat(110)),
                     (CGSize(width: 1194, height: 360), CGFloat(180), CGFloat(120)),
                     (CGSize(width: 834, height: 280), CGFloat(250), CGFloat(96)),
                     (CGSize(width: 1024, height: 200), CGFloat(340), CGFloat(144))] {
            for railRight in [false, true] {
                let (size, header, status) = item
                let layout = WorkspaceLayout.resolve(size: size, browser: true, inspector: true,
                                                     railRight: railRight, largeText: true,
                                                     minimumHeaderHeight: header, minimumStatusHeight: status, compactChrome: false)
                for frame in [layout.topBar, layout.status, layout.rail, layout.canvas] + [layout.browser, layout.inspector].compactMap({ $0 }) {
                    try require(frame.origin.x.isFinite && frame.origin.y.isFinite && frame.width.isFinite && frame.height.isFinite,
                                "scaled type created nonfinite geometry")
                    try require(frame.width >= 0 && frame.height >= 0, "scaled type created negative geometry")
                    try require(frame.minX >= 0 && frame.minY >= 0 && frame.maxX <= size.width && frame.maxY <= size.height,
                                "scaled type escaped keyboard-reduced bounds")
                }
                try require(layout.topBar.height >= min(header, size.height), "scaled header was clipped despite available height")
                try require(layout.status.height >= min(status, max(0, size.height - layout.topBar.height)), "scaled status was clipped despite available height")
                try require(layout.canvas.minY >= layout.topBar.maxY && layout.canvas.maxY <= layout.status.minY,
                            "scaled type overlaps rendering/picking bounds")
                try require(!layout.canvas.intersects(layout.rail), "scaled type rail overlaps viewport")
                if !layout.compact {
                    if let frame = layout.browser { try require(!layout.canvas.intersects(frame), "scaled browser overlaps viewport") }
                    if let frame = layout.inspector { try require(!layout.canvas.intersects(frame), "scaled inspector overlaps viewport") }
                }
                cases += 1
            }
        }
        // Floating panels must not resize the viewport. Only an explicitly
        // pinned wide browser consumes content width; closing it restores it.
        for size in [CGSize(width: 1194, height: 790), CGSize(width: 1024, height: 700),
                     CGSize(width: 834, height: 1140), CGSize(width: 600, height: 420),
                     CGSize(width: 390, height: 760), CGSize(width: 320, height: 340)] {
            for largeText in [false, true] {
                for railRight in [false, true] {
                    let closed = WorkspaceLayout.resolve(size: size, browser: false, inspector: false, railRight: railRight, largeText: largeText)
                    try require(closed.canvas == CGRect(origin: .zero, size: size), "closed chrome consumes model space")
                    try require(closed.canvas.width >= size.width * 0.8, "floating workspace misses viewport width target")
                    try require(closed.rail.width >= 44 && closed.rail.width <= 56, "icon rail loses touch width")
                    try require(closed.topBar.width < size.width && closed.topBar.height < 100, "floating header became a full-width banner")
                    for panels in [(true, false), (false, true)] {
                        let opened = WorkspaceLayout.resolve(size: size, browser: panels.0, inspector: panels.1, railRight: railRight, largeText: largeText)
                        try require(opened.canvas == closed.canvas, "floating panel rebases rendering or picking")
                        try require(!opened.compact, "floating panel incorrectly owns a full-screen modal scrim")
                        for frame in [opened.topBar, opened.status, opened.rail, opened.navigation, opened.history, opened.browserAccess] + [opened.browser, opened.inspector].compactMap({ $0 }) {
                            try require(CGRect(origin: .zero, size: size).contains(frame), "floating chrome escaped a resized window")
                        }
                    }
                    let pinned = WorkspaceLayout.resolve(size: size, browser: true, inspector: false, railRight: railRight, largeText: largeText, pinnedBrowser: true)
                    if size.width >= (largeText ? 1180 : 1000), let browser = pinned.browser {
                        try require(pinned.canvas.width >= 360 && !pinned.canvas.intersects(browser), "pinning covers the remaining viewport")
                    } else { try require(pinned.canvas == closed.canvas, "narrow pin preference consumes viewport") }
                    cases += 1
                }
            }
        }
        for size in [CGSize.zero, CGSize(width: 1, height: 1), CGSize(width: 44, height: 55), CGSize(width: -10, height: CGFloat.infinity)] {
            let layout = WorkspaceLayout.resolve(size: size, browser: true, inspector: true, railRight: true, largeText: true)
            for frame in [layout.topBar, layout.status, layout.rail, layout.canvas, layout.navigation, layout.history] {
                try require(frame.width.isFinite && frame.height.isFinite && frame.width >= 0 && frame.height >= 0, "invalid floating drawable")
            }
        }
        let numericPanel = WorkspaceLayout.resolve(size: CGSize(width: 1194, height: 790), browser: false, inspector: true,
                                                   railRight: false, largeText: false, inspectorContentHeight: 500, navigationContentHeight: 146)
        try require(numericPanel.inspector?.height == 500, "simple numeric editor became a full-height inspector")
        try require(numericPanel.navigation.height == 146, "navigation surface retains unused permanent space")
        try require(WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: false, numericEditActive: false), "document shortcuts unavailable at rest")
        try require(!WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: true, numericEditActive: false), "text undo routes to document")
        try require(!WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: false, numericEditActive: true), "numeric draft permits unrelated history action")
        try require(!WorkspaceFocusPolicy.viewportInputEnabled(modalPresented: true, textEntryActive: false), "dismissal tap can reach viewport")
        try require(!WorkspaceFocusPolicy.viewportInputEnabled(modalPresented: false, textEntryActive: true), "typing leaves viewport active")
        try require(WorkspaceFocusPolicy.viewportInputEnabled(modalPresented: false, textEntryActive: false), "viewport unavailable at rest")
        // These fixtures reproduce the generic command envelope returned by the
        // real C ABI, including the command-value layer and sibling viewport data.
        struct ExpressionValue: Decodable { let ok: Bool; let value: Double?; let error: String? }
        struct DependentsValue: Decodable { let deleted_with_it: [String]; let would_fail: [String] }
        let expressionJSON = Data(#"{"ok":true,"result":{"command":{"value":{"ok":true,"value":25.4,"error":null},"document_id":"fixture","revision":2,"geometry_revision":1,"changed":false},"viewport":{"preview":false}}}"#.utf8)
        let expression = try JSONDecoder().decode(EngineCommandEnvelope<ExpressionValue>.self, from: expressionJSON).result.command.value
        try require(expression.ok && expression.value == 25.4, "native expression command value was not decoded")
        let invalidJSON = Data(#"{"ok":true,"result":{"command":{"value":{"ok":false,"value":null,"error":"incomplete expression"}},"viewport":{}}}"#.utf8)
        let invalid = try JSONDecoder().decode(EngineCommandEnvelope<ExpressionValue>.self, from: invalidJSON).result.command.value
        try require(!invalid.ok && invalid.value == nil && invalid.error == "incomplete expression", "expression validation result was lost")
        let dependentsJSON = Data(#"{"ok":true,"result":{"command":{"value":{"deleted_with_it":["Extrusion1"],"would_fail":["Fillet1"]}},"viewport":{}}}"#.utf8)
        let dependents = try JSONDecoder().decode(EngineCommandEnvelope<DependentsValue>.self, from: dependentsJSON).result.command.value
        try require(dependents.deleted_with_it == ["Extrusion1"] && dependents.would_fail == ["Fillet1"], "native dependency command value was not decoded")
        let committedJSON = Data(#"{"ok":true,"result":{"worked":true,"committed":true,"viewport":{}}}"#.utf8)
        let committedWork = try JSONDecoder().decode(SketchToolWorkEnvelope.self, from: committedJSON).result
        try require(committedWork.accepts(commit: true) && !committedWork.accepts(commit: false), "commit result cannot be confused with preview")
        let previewJSON = Data(#"{"ok":true,"result":{"worked":true,"committed":false,"viewport":{}}}"#.utf8)
        let previewWork = try JSONDecoder().decode(SketchToolWorkEnvelope.self, from: previewJSON).result
        try require(previewWork.accepts(commit: false) && !previewWork.accepts(commit: true), "preview result cannot be reported as applied")
        for cancelledJSON in [#"{"ok":true,"result":{"worked":false}}"#, #"{"ok":true,"result":{"discarded":true}}"#] {
            let work = try JSONDecoder().decode(SketchToolWorkEnvelope.self, from: Data(cancelledJSON.utf8)).result
            try require(!work.accepts(commit: true) && !work.accepts(commit: false), "cancelled or stale work reported success")
        }
        // Production modelling forms edit opaque native definitions. Nested
        // expression edits must preserve extent mode and references exactly;
        // large native integer IDs must survive JSON without Double rounding.
        let definitionJSON = Data(#"{"extent":{"type":"distance","distance":"width / 2"},"target":{"feature":18446744073709551615,"face":2},"sections":[{"sketch":9007199254740993,"profiles":[1]}]}"#.utf8)
        let definition = try JSONDecoder().decode([String: PresentationJSON].self, from: definitionJSON)
        try require(PresentationJSON.parameter("extent.distance", in: definition) == .string("width / 2"), "original expression was normalised")
        let draft = PresentationJSON.setting("extent.distance", value: .string("width / 2 + 1 mm"), in: definition)
        try require(PresentationJSON.parameter("extent.distance", in: draft) == .string("width / 2 + 1 mm"), "nested expression did not update")
        try require(PresentationJSON.parameter("extent.type", in: draft) == .string("distance"), "extent mode was replaced")
        try require(draft["extent.distance"] == nil && draft["target"] == definition["target"] && draft["sections"] == definition["sections"], "draft corrupted opaque native references")
        try require(PresentationJSON.parameter("extent.distance", in: definition) == .string("width / 2"), "draft mutated original definition")
        let exactJSON = try JSONSerialization.data(withJSONObject: draft.mapValues(\.value), options: [.sortedKeys])
        let roundTrip = try JSONDecoder().decode([String: PresentationJSON].self, from: exactJSON)
        try require(roundTrip == draft && PresentationJSON.parameter("target.feature", in: roundTrip) == .unsignedInteger(UInt64.max), "native integer reference did not round-trip")
        try require(draft != definition, "changed expressions could reuse old preview eligibility")
        // This is the native model bridge envelope shape: real profile-selection
        // payloads, expression text, nullable feature IDs and ordered input arrays.
        // The production response decoder is generic only over the workspace body.
        struct ModelWorkspace: Decodable { let modelling: ModellingPresentation }
        struct ModelEnvelope: Decodable { let result: ModellingResponse<ModelWorkspace> }
        let modelJSON = Data(#"{"ok":true,"result":{"token":11,"operation":"solid.extrude","feature":null,"params":{"profile":{"sketch":7,"profiles":[{"curves":["l1","l2","l3","l4"]}]},"distance":"height / 2","direction":"positive","operation":"new_body","target":null},"fields":[{"key":"profile","label":"Profile","kind":"reference","options":[{"label":"Sketch · region 1","value":{"sketch":7,"profiles":[{"curves":["l1","l2","l3","l4"]}]}}]},{"key":"distance","label":"Distance","kind":"expression","options":[],"units":"mm"},{"key":"direction","label":"Direction","kind":"choice","options":[{"label":"positive","value":"positive"}]},{"key":"sections","label":"Ordered sections","kind":"references","options":[]}],"valid":true,"workspace":{"modelling":{"tools":[{"id":"solid.extrude","label":"cmd.solid.extrude"}],"timeline":[{"id":7,"name":"Sketch","kind":"Sketch","order":0,"suppressed":false,"error":null,"warning":null,"stale":false}],"parameters":[{"name":"height","expression":"20 mm","unit":"mm","value":20}]}},"viewport":{"preview":true}}}"#.utf8)
        let model = try JSONDecoder().decode(ModelEnvelope.self, from: modelJSON).result
        try require(model.token == 11 && model.feature == nil && model.valid == true && model.operation == "solid.extrude", "native modelling context was not decoded")
        try require(model.fields?.map(\.kind) == ["reference", "expression", "choice", "references"], "modelling input field kinds were lost")
        try require(model.fields?.first?.options?.first?.value == model.params?["profile"], "real profile reference did not match its option")
        try require(model.params?["distance"] == .string("height / 2") && model.fields?[1].units == "mm", "expression or units were changed")
        try require(model.workspace.modelling.timeline.first?.id == 7 && model.workspace.modelling.parameters.first?.expression == "20 mm", "feature history or named parameters were lost")
        let finishedModelJSON = Data(#"{"ok":true,"result":{"token":null,"valid":true,"committed":true,"workspace":{"modelling":{"tools":[],"timeline":[],"parameters":[]}},"viewport":{}}}"#.utf8)
        let finishedModel = try JSONDecoder().decode(ModelEnvelope.self, from: finishedModelJSON).result
        try require(finishedModel.token == nil && finishedModel.fields == nil && finishedModel.params == nil, "completed model response requires an active form")
        struct AssemblyWorkspace: Decodable { let assembly: AssemblyPresentation }
        let assemblyJSON = Data(#"{"token":23,"operation":"joint.drive","feature":null,"params":{"joint":4,"value":"requested_angle"},"fields":[{"key":"value","label":"Angle","kind":"slider","min":-45,"max":45,"value":10,"units":"deg","options":[]}],"valid":true,"feedback":["Requested 90 deg; native limit reached at 45 deg."],"reply":{"moved":false,"reason":"The joint is not connected to grounded geometry."},"workspace":{"assembly":{"active_component":5,"active_name":"Moving arm","active_path":[9007199254740993,7],"tools":[{"id":"joint.create","label":"cmd.joint.create"}],"diagnostics":["Floating occurrence"],"actions":[{"label":"Return to root","command":"component.activate","params":{"component":0}}]}}}"#.utf8)
        let assembly = try JSONDecoder().decode(ModellingResponse<AssemblyWorkspace>.self, from: assemblyJSON)
        try require(assembly.fields?.first?.kind == "slider" && assembly.fields?.first?.min == -45 && assembly.fields?.first?.max == 45 && assembly.fields?.first?.value == 10, "joint slider bounds or current native value were lost")
        try require(assembly.feedback?.first?.contains("45 deg") == true && PresentationJSON.parameter("moved", in: { if case .object(let value) = assembly.reply { return value }; return [:] }()) == .bool(false), "no-movement feedback was presented as movement")
        try require(assembly.workspace.assembly.active_path == [9007199254740993, 7], "nested authoring path was rounded or shortened")
        try require(assembly.workspace.assembly.actions.first?.command == "component.activate" && assembly.workspace.assembly.actions.first?.params["component"] == .integer(0), "assembly action arguments changed")
        let clampedJSON = Data(#"{"token":23,"operation":"joint.drive","params":{"joint":4,"value":"75 deg"},"fields":[{"key":"value","label":"Angle","kind":"slider","min":0,"max":50,"value":50,"units":"deg"}],"valid":true,"feedback":["Native value: 50 deg"],"reply":{"moved":true,"values":[{"name":"Angle","value":50,"unit":"deg"}]},"workspace":{"assembly":{"active_component":0,"active_name":"Root","active_path":[],"tools":[],"diagnostics":[],"actions":[]}}}"#.utf8)
        let clamped = try JSONDecoder().decode(ModellingResponse<AssemblyWorkspace>.self, from: clampedJSON)
        try require(clamped.params?["value"] == .string("75 deg") && clamped.fields?.first?.value == 50 && clamped.feedback == ["Native value: 50 deg"], "clamped motion replaced requested expression or hid the actual value")
        let rowJSON = Data(#"{"component":5,"path":[9007199254740993,7],"joint":null,"active":true,"grounded":false,"visible":true,"pending":true,"diagnostic":"Shared placement","actions":[{"label":"Move","operation":"occurrence.move","params":{"occurrence":7,"path":[9007199254740993,7]},"confirm":"This changes every placement of the shared definition."}]}"#.utf8)
        let row = try JSONDecoder().decode(AssemblyRowPresentation.self, from: rowJSON)
        try require(row.path == [9007199254740993,7] && row.active == true && row.pending == true && row.grounded == false && row.actions.first?.operation == "occurrence.move" && row.actions.first?.confirm != nil, "assembly row identity or confirmation was lost")
        let proposalA: [String: PresentationJSON] = ["value": .string("10 deg")]
        let proposalB: [String: PresentationJSON] = ["value": .string("20 deg")]
        let proposalC: [String: PresentationJSON] = ["value": .string("30 deg")]
        var previews = ModellingPreviewQueue()
        try require(previews.offer(proposalA), "first slider preview was not scheduled")
        try require(!previews.offer(proposalB) && !previews.offer(proposalC) && previews.active == proposalA, "slider input scheduled multiple in-flight evaluations")
        try require(previews.complete() == proposalC, "latest slider sample was dropped")
        try require(previews.offer(proposalA) && !previews.offer(proposalB) && !previews.offer(proposalA) && previews.complete() == nil, "returning to the in-flight value scheduled a redundant preview")
        try require(previews.offer(proposalA) && !previews.offer(proposalC), "slider cancellation fixture failed")
        previews.clear()
        try require(previews.active == nil && previews.complete() == nil, "cancelled slider preview survived its generation")
        try require(previews.offer(proposalA) && !previews.offer(proposalC), "context change fixture failed")
        previews.discardPending()
        try require(previews.active == proposalA && previews.complete() == nil, "changing context released the active worker or kept a proposal for the old controls")
        let labels = ["cmd.joint_drive": "Drive Joints", "cmd.component_create": "New Component", "ui.extrude": "Extrude", "ui.pattern_on_path": "Pattern on Path", "ui.ipados.assembly.field.a_point_x": "First world origin X"]
        let lookup: (String) -> String = { labels[$0] ?? $0 }
        try require(PresentationLabelKey.label("cmd.joint.drive", lookup: lookup) == "Drive Joints", "dotted native joint label did not resolve its canonical key")
        try require(PresentationLabelKey.label("cmd.component_create", lookup: lookup) == "New Component", "canonical command label was rewritten")
        try require(PresentationLabelKey.label("cmd.solid.extrude", lookup: lookup) == "Extrude" && PresentationLabelKey.label("cmd.solid.pattern.path", lookup: lookup) == "Pattern on Path", "legacy modelling command labels did not reuse their native catalogue")
        try require(PresentationLabelKey.label("ui.ipados.assembly.field.a_point_x", lookup: lookup) == "First world origin X", "UI namespace punctuation changed")
        try require(PresentationLabelKey.label("Arm.1 / Bracket_2", lookup: lookup) == "Arm.1 / Bracket_2" && PresentationLabelKey.label("cmd.unknown.command", lookup: lookup) == "cmd.unknown.command", "user text or unknown label fallback was changed")
        print("Workspace layout/focus/command decoding checks PASS: \(cases) adaptive cases; no simulator or device launch")
    }
}
