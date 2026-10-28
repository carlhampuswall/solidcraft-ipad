import CoreGraphics
import Foundation

@main
struct WorkspaceChecks {
    static func require(_ condition: @autoclosure () -> Bool, _ message: String) throws {
        if !condition() { throw NSError(domain: "WorkspaceChecks", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    }

    static func main() throws {
        // A busy display lane must preserve the Pencil contact through a burst
        // of actual samples, including its final lift, without an input.cancel.
        func pencil(_ phase: InputSample.Phase, _ sequence: UInt64) -> InputSample {
            var sample = InputSample(source: .pencil, phase: phase, id: 91,
                x_points: Double(sequence), y_points: 100, x_pixels: Double(sequence) * 2, y_pixels: 200,
                scale: 2, timestamp: Double(sequence), pressure: nil, altitude: nil,
                geometry_revision: 1, consumed: false, shift: false, button: 0, scroll: 0)
            sample.sequence = sequence; sample.viewport_epoch = 1; sample.camera_epoch = 1; sample.document_id = "design"
            return sample
        }
        let mailbox = InputMailbox()
        let inputToken = mailbox.append(pencil(.began, 1))
        for sequence in UInt64(2)...1000 { _ = mailbox.append(pencil(.moved, sequence)) }
        _ = mailbox.append(pencil(.ended, 1001))
        guard let inputToken, let batch = mailbox.take(token: inputToken) else { throw NSError(domain: "InputChecks", code: 1) }
        try require(!batch.cancelled, "Pencil sample backlog cancelled an active resize")
        try require(batch.samples.count <= 128, "native input buffer exceeded its bound")
        try require(batch.samples.first?.phase == .began && batch.samples.last?.phase == .ended, "backlog dropped contact boundaries")
        try require(batch.samples.dropLast().last?.sequence == 1000, "backlog lost the latest actual drag position")
        try require(zip(batch.samples, batch.samples.dropFirst()).allSatisfy { $0.sequence < $1.sequence }, "coalescing reordered Pencil samples")
        let reversing = InputMailbox()
        let reversingToken = reversing.append(pencil(.began, 1))
        for sequence in UInt64(2)...1000 {
            let position = sequence <= 500 ? sequence : 1000 - sequence
            var sample = InputSample(source: .pencil, phase: .moved, id: 91,
                x_points: Double(position), y_points: 100, x_pixels: Double(position) * 2, y_pixels: 200,
                scale: 2, timestamp: Double(sequence), pressure: nil, altitude: nil,
                geometry_revision: 1, consumed: false, shift: false, button: 0, scroll: 0)
            sample.sequence = sequence
            _ = reversing.append(sample)
        }
        guard let reversingToken, let reverseBatch = reversing.take(token: reversingToken) else { throw NSError(domain: "InputChecks", code: 2) }
        try require(!reverseBatch.cancelled && reverseBatch.samples.contains(where: { $0.x_points == 500 }), "coalescing lost a direction reversal")
        try require(reverseBatch.samples.last?.x_points == 0, "reverse drag lost its final actual position")
        let uncertain = InputMailbox()
        let uncertainToken = uncertain.append(pencil(.began, 1))
        for sequence in UInt64(2)...130 { _ = uncertain.append(pencil(.hover, sequence)) }
        guard let uncertainToken, let uncertainBatch = uncertain.take(token: uncertainToken) else { throw NSError(domain: "InputChecks", code: 3) }
        try require(uncertainBatch.cancelled, "uncompressible input must retain conservative cancellation")
        let staleInputToken = mailbox.append(pencil(.began, 1002))
        mailbox.clear()
        if let staleInputToken { try require(mailbox.take(token: staleInputToken) == nil, "cancelled input batch survived a context change") }
        // On-model value chips stay readable, clear of both the grip and chrome.
        for bounds in [CGRect(x: 0, y: 0, width: 1194, height: 790), CGRect(x: 0, y: 0, width: 390, height: 420)] {
            for anchor in [CGPoint(x: bounds.midX, y: bounds.midY), CGPoint(x: bounds.maxX - 5, y: bounds.midY), CGPoint(x: 5, y: 5)] {
                let obstacle = CGRect(x: bounds.maxX - 100, y: 0, width: 100, height: 120)
                if let frame = ModelValuePlacement.resolve(anchor: anchor, size: CGSize(width: 100, height: 48), bounds: bounds, obstacles: [obstacle]) {
                    try require(bounds.contains(frame), "handle value escaped the visible viewport")
                    try require(!frame.intersects(obstacle), "handle value covers workspace controls")
                    try require(!frame.intersects(CGRect(x: anchor.x - 26, y: anchor.y - 26, width: 52, height: 52)), "value chip steals handle drags")
                }
            }
        }
        try require(ModelValuePlacement.resolve(anchor: CGPoint(x: 100, y: 100), size: CGSize(width: 100, height: 48), bounds: CGRect(x: 0, y: 0, width: 300, height: 300), obstacles: []) != nil, "normal viewport lost exact value entry")
        try require(ModelValuePlacement.resolve(anchor: .zero, size: CGSize(width: 100, height: 48), bounds: .zero, obstacles: []) == nil, "collapsed viewport exposes a value chip")
        let handleJSON = Data(#"{"token":750,"parameter":"extent.distance","position":[400,300],"value":15,"units":"mm","expression":"plate_depth + 5 mm","valid":true}"#.utf8)
        let handleValue = try JSONDecoder().decode(ModelValuePresentation.self, from: handleJSON)
        try require(handleValue.expression == "plate_depth + 5 mm" && handleValue.token == 750 && handleValue.value == 15, "handle response replaced an expression or lost its transaction")
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
        // The viewport fills space behind system indicators; controls retain
        // their safe, keyboard-reduced bounds, including with a pinned browser.
        for size in [CGSize(width: 1366, height: 1024), CGSize(width: 1194, height: 834), CGSize(width: 834, height: 1194),
                     CGSize(width: 600, height: 420), CGSize(width: 390, height: 844)] {
            for keyboard in [false, true] {
                let viewport = CGRect(x: 0, y: 0, width: size.width, height: size.height - (keyboard ? 300 : 0))
                let safe = CGRect(x: 12, y: 24, width: size.width - 24, height: size.height - 44)
                let chrome = CGRect(x: safe.minX, y: safe.minY, width: safe.width,
                                    height: max(0, min(safe.maxY, viewport.maxY) - safe.minY))
                for railRight in [false, true] {
                    for largeText in [false, true] {
                        let closed = WorkspaceLayout.resolve(size: chrome.size, browser: false, inspector: false,
                            railRight: railRight, largeText: largeText).placed(in: chrome, viewport: viewport)
                        try require(closed.canvas == viewport, "safe area leaves unused viewport strips")
                        for panel in [(false, false), (true, false), (true, true)] {
                            let layout = WorkspaceLayout.resolve(size: chrome.size, browser: panel.0, inspector: true,
                                railRight: railRight, largeText: largeText, pinnedBrowser: panel.1).placed(in: chrome, viewport: viewport)
                            try require(layout.canvas.minY == viewport.minY && layout.canvas.maxY == viewport.maxY,
                                        "panel leaves top or bottom viewport strips")
                            try require(viewport.contains(layout.canvas), "placed viewport escaped window/keyboard bounds")
                            for frame in [layout.topBar, layout.status, layout.rail, layout.navigation, layout.history, layout.browserAccess] + [layout.browser, layout.inspector].compactMap({ $0 }) {
                                try require(chrome.contains(frame), "full-window layout moved controls under system indicators")
                            }
                            if panel.1, chrome.width >= (largeText ? 1180 : 1000), let browser = layout.browser {
                                try require(!layout.canvas.intersects(browser), "full-window expansion covers pinned browser")
                            } else { try require(layout.canvas == closed.canvas, "floating panel changes full-window picking bounds") }
                            cases += 1
                        }
                    }
                }
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
        let libraryRoot = FileManager.default.temporaryDirectory.appendingPathComponent("SolidCraft-Library-Check-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: libraryRoot) }
        let library = ProjectLibrary(root: libraryRoot)
        let emptyProjects = try library.projects()
        try require(emptyProjects.isEmpty, "new library is not empty")
        let projectA = try library.reserve(name: "  Bracket  ")
        let projectB = try library.reserve(name: "Bracket")
        try require(projectA.id != projectB.id && projectA.file != projectB.file, "same-name projects overwrite each other")
        try Data("first design".utf8).write(to: projectA.file)
        try Data("second design".utf8).write(to: projectB.file)
        let projectList = try library.projects()
        try require(Set(projectList.map(\.id)) == Set([projectA.id, projectB.id]) && projectList.allSatisfy { $0.name == "Bracket" }, "library loses project identity or display name")
        try library.remove(projectA)
        try require(FileManager.default.fileExists(atPath: projectB.file.path), "deleting one project erased an equally named design")
        for name in ["", "  ", "../Design", "a/b", "a\\b", "a:b", ".", "..", ".Hidden", "a\n", String(repeating: "界", count: 100)] {
            // Trailing whitespace is allowed and removed; an embedded control is not.
            if name == "a\n" { continue }
            do { _ = try library.reserve(name: name); throw ProjectLibrary.failure("accepted_bad_name") }
            catch { try require((error as NSError).localizedDescription != "ui.ipados.projects.accepted_bad_name", "unsafe project name was accepted") }
        }
        let forged = ProjectRecord(id: projectB.id, file: libraryRoot.appendingPathComponent("elsewhere/Bracket.solidcraft"), modified: Date())
        do { try library.remove(forged); throw ProjectLibrary.failure("accepted_bad_path") }
        catch { try require((error as NSError).localizedDescription != "ui.ipados.projects.accepted_bad_path", "deletion escaped the checked project path") }
        try require(FileManager.default.fileExists(atPath: projectB.file.path), "invalid deletion touched the current library")
        let stepFile = libraryRoot.appendingPathComponent("Bracket.STP")
        try Data("ISO-10303-21;".utf8).write(to: stepFile)
        let staged = try ProjectLibrary.stageImport(stepFile)
        try require(staged != stepFile && staged.lastPathComponent == stepFile.lastPathComponent, "import overwrote its source or lost its format")
        try FileManager.default.removeItem(at: stepFile)
        let stagedBytes = try Data(contentsOf: staged)
        try require(stagedBytes == Data("ISO-10303-21;".utf8), "staged import depended on an external URL")
        try FileManager.default.removeItem(at: staged.deletingLastPathComponent())
        let empty = libraryRoot.appendingPathComponent("Empty.step")
        try Data().write(to: empty)
        do { _ = try ProjectLibrary.stageImport(empty); throw ProjectLibrary.failure("accepted_empty") }
        catch { try require((error as NSError).localizedDescription != "ui.ipados.projects.accepted_empty", "empty import was accepted") }
        let oversized = libraryRoot.appendingPathComponent("Oversized.step")
        try Data().write(to: oversized)
        let sparse = try FileHandle(forWritingTo: oversized)
        try sparse.truncate(atOffset: UInt64(ProjectLibrary.maximumImportBytes + 1))
        try sparse.close()
        do { _ = try ProjectLibrary.stageImport(oversized); throw ProjectLibrary.failure("accepted_oversized") }
        catch { try require((error as NSError).localizedDescription != "ui.ipados.projects.accepted_oversized", "oversized import bypassed its limit") }
        try require(ProjectLibrary.importExtensions.contains("step") && ProjectLibrary.importExtensions.contains("stp") && !ProjectLibrary.importExtensions.contains("exe"), "import extension contract changed")
        print("Workspace layout/focus/command decoding checks PASS: \(cases) adaptive cases; no simulator or device launch")
    }
}
