import CoreGraphics
import Foundation

@main
struct WorkspaceChecks {
    static func require(_ condition: @autoclosure () -> Bool, _ message: String) throws {
        if !condition() { throw NSError(domain: "WorkspaceChecks", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    }

    static func main() throws {
        let edgeTemplate = "Edge length: {length} mm"
        let edgeLocale = Locale(identifier: "en_US")
        try require(EdgeLengthReadout.restored(nil) && EdgeLengthReadout.restored(true) && !EdgeLengthReadout.restored(false),
                    "edge length preference lost its default or saved value")
        try require(EdgeLengthReadout.text(lengthMM: 40, enabled: true, template: edgeTemplate, locale: edgeLocale) == "Edge length: 40 mm",
                    "edge length omitted its label or units")
        try require(EdgeLengthReadout.text(lengthMM: 12.34567, enabled: true, template: edgeTemplate, locale: Locale(identifier: "de_DE")) == "Edge length: 12,346 mm",
                    "edge length did not use locale-aware rounding")
        try require(EdgeLengthReadout.text(lengthMM: 40, enabled: false, template: edgeTemplate) == nil,
                    "disabled edge length preference did not restore the previous status")
        for value: Double? in [nil, .nan, .infinity, -1] {
            try require(EdgeLengthReadout.text(lengthMM: value, enabled: true, template: edgeTemplate) == nil,
                        "an unavailable or invalid edge produced a length readout")
        }
        let radiusTemplate = "Radius: {radius} mm"
        try require(EdgeLengthReadout.text(lengthMM: .pi * 7, radiusMM: 7, enabled: true, template: edgeTemplate,
            radiusTemplate: radiusTemplate, locale: edgeLocale) == "Edge length: 21.991 mm · Radius: 7 mm",
            "a semicircle did not display arc length together with its measured radius")
        try require(EdgeLengthReadout.text(lengthMM: 12.34567, radiusMM: 4.12345, enabled: true, template: edgeTemplate,
            radiusTemplate: radiusTemplate, locale: Locale(identifier: "de_DE")) == "Edge length: 12,346 mm · Radius: 4,123 mm",
            "round-edge length and radius did not share locale-aware precision")
        try require(EdgeLengthReadout.text(lengthMM: 40, radiusMM: 7, enabled: false, template: edgeTemplate,
            radiusTemplate: radiusTemplate) == nil, "disabled measurement preference left a radius readout")
        for radius: Double? in [nil, .nan, .infinity, -1] {
            try require(EdgeLengthReadout.text(lengthMM: 40, radiusMM: radius, enabled: true, template: edgeTemplate,
                radiusTemplate: radiusTemplate, locale: edgeLocale) == "Edge length: 40 mm",
                "a straight or invalid-radius edge displayed a fabricated radius")
        }
        // Boolean presentation follows typed options, never labels or truthy strings.
        let automatic = ModellingFormPresentation.initialParameters(operation: "solid.extrude", feature: nil,
            automatic: true, legacyDepth: false, supplied: nil)
        try require(automatic == ["distance": .string("")], "automatic Extrude proposed a default distance")
        for (operation, feature, enabled, legacy) in [("solid.extrude", UInt64?.none, false, false),
            ("solid.extrude", UInt64(7), true, false), ("solid.revolve", nil, true, false),
            ("solid.extrude", nil, true, true)] {
            try require(ModellingFormPresentation.initialParameters(operation: operation, feature: feature,
                automatic: enabled, legacyDepth: legacy, supplied: nil) == nil,
                "manual/feature editing or the legacy preference lost its initial value")
        }
        let supplied: [String: PresentationJSON] = ["distance": .string("7 mm"), "operation": .string("join")]
        try require(ModellingFormPresentation.initialParameters(operation: "solid.extrude", feature: nil,
            automatic: true, legacyDepth: false, supplied: supplied) == supplied, "explicit user distance was overwritten")
        for source in [PresentationJSON.object(["sketch": .integer(1)]), .object(["face_body": .string("Stock")])] {
            var params: [String: PresentationJSON] = ["profile": source, "distance": .string("")]
            try require(ModellingFormPresentation.needsExtrusionDistance(params), "empty face/sketch depth allowed confirmation")
            params["distance"] = .string(" \n ")
            try require(ModellingFormPresentation.needsExtrusionDistance(params), "whitespace depth allowed confirmation")
            params["through_all"] = .bool(true)
            try require(!ModellingFormPresentation.needsExtrusionDistance(params), "Through All incorrectly required a blind depth")
            params["through_all"] = .bool(false); params["distance"] = .string("7 mm")
            try require(!ModellingFormPresentation.needsExtrusionDistance(params), "entered distance was ignored")
        }
        let booleanOptions: [PresentationJSON] = [.bool(false), .bool(true)]
        for options in [booleanOptions, Array(booleanOptions.reversed())] {
            for value in [false, true] {
                try require(ModellingFormPresentation.booleanValue(kind: "choice", options: options, selected: .bool(value)) == value,
                            "boolean control changed its typed value or depended on option order")
            }
        }
        for selected: PresentationJSON? in [nil, .null, .string("false"), .integer(0)] {
            try require(ModellingFormPresentation.booleanValue(kind: "choice", options: booleanOptions, selected: selected) == nil,
                        "non-boolean selection was coerced into a switch")
        }
        let otherOptions: [[PresentationJSON]] = [[], [.bool(false)], [.bool(true), .bool(true)],
            [.string("false"), .string("true")], [.integer(0), .integer(1)], [.bool(false), .null],
            [.bool(false), .bool(true), .null]]
        for options in otherOptions {
            try require(ModellingFormPresentation.booleanValue(kind: "choice", options: options, selected: .bool(false)) == nil,
                        "a switch hid an option or replaced a non-boolean choice")
        }
        for kind in ["reference", "references", "inline_choice", "text"] {
            try require(ModellingFormPresentation.booleanValue(kind: kind, options: booleanOptions, selected: .bool(true)) == nil,
                        "boolean options replaced another field's presentation contract")
        }
        // Reordering retains opaque references exactly, including IDs beyond Double precision.
        let opaqueReference = PresentationJSON.object(["feature": .unsignedInteger(UInt64.max),
            "profiles": .array([.object(["curves": .array([.string("edge-2"), .string("edge-1")])])])])
        let references: [PresentationJSON] = [.string("Body2"), opaqueReference, .string("Body1")]
        try require(ModellingFormPresentation.removing(at: 1, from: references) == [.string("Body2"), .string("Body1")],
                    "removing a reference changed the remaining order")
        try require(ModellingFormPresentation.removing(at: 0, from: [opaqueReference]) == [], "last reference could not be removed")
        try require(ModellingFormPresentation.moving(from: 0, to: 2, in: references) == [opaqueReference, .string("Body1"), .string("Body2")],
                    "moving down swapped distant inputs instead of retaining their order")
        try require(ModellingFormPresentation.moving(from: 2, to: 0, in: references) == [.string("Body1"), .string("Body2"), opaqueReference],
                    "moving up changed opaque reference content or intervening order")
        try require(ModellingFormPresentation.moving(from: 1, to: 1, in: references) == references, "moving to the same position changed the list")
        for index in [Int.min, -1, references.count, Int.max] {
            try require(ModellingFormPresentation.removing(at: index, from: references) == nil, "invalid removal index was accepted")
            try require(ModellingFormPresentation.moving(from: index, to: 0, in: references) == nil, "invalid move source was accepted")
            try require(ModellingFormPresentation.moving(from: 0, to: index, in: references) == nil, "invalid move destination was accepted")
        }
        try require(ModellingFormPresentation.removing(at: 0, from: []) == nil &&
                    ModellingFormPresentation.moving(from: 0, to: 0, in: []) == nil, "empty reference list accepted an index")
        try require(references == [.string("Body2"), opaqueReference, .string("Body1")], "reference presentation mutated the original draft")
        try require(DragResolution.restored(nil) == 0.1, "drag resolution must default to 0.1 mm")
        try require(DragResolution.restored(.nan) == 0.1 && DragResolution.restored(-1) == 0.1, "invalid saved drag resolution")
        for step in DragResolution.options { try require(DragResolution.restored(step) == step, "saved drag resolution was not restored") }
        for (value, expected) in [(12.34, 12.3), (12.36, 12.4), (-12.36, -12.4)] {
            try require(abs(DragResolution.snapped(value, units: "mm", step: 0.1) - expected) < 1e-10, "slider resolution failed")
        }
        try require(DragResolution.snapped(12.3456, units: "mm", step: 0) == 12.3456, "continuous slider behaviour changed")
        try require(DragResolution.snapped(12.3456, units: "deg", step: 0.1) == 12.3456, "length resolution changed an angle")
        try require(DragResolution.snapped(12.3456, units: "", step: 0.1) == 12.3456, "length resolution changed a unitless slider")
        try require(WorkspaceSelectionFilter(storedValue: nil) == .auto, "new workspaces exclude edges from Pencil selection")
        try require(WorkspaceSelectionFilter(storedValue: "unknown") == .auto, "invalid saved selection filter did not fall back to Auto")
        for filter in WorkspaceSelectionFilter.allCases {
            try require(WorkspaceSelectionFilter(storedValue: filter.rawValue) == filter, "saved selection filter was not restored")
        }
        func dismissFaceExtrude(_ selection: [String], automatic: Bool = true, committing: Bool = false) -> Bool {
            SelectionExtrudeTrigger<String>.shouldDismiss(automatic: automatic, committing: committing,
                                                     selection: selection, source: { $0.hasPrefix("face:") ? "face" : nil })
        }
        try require(dismissFaceExtrude([]), "deselecting the last face left automatic Extrude active")
        try require(dismissFaceExtrude(["edge:1"]), "switching away from faces left automatic Extrude active")
        try require(!dismissFaceExtrude(["face:top"]), "a remaining face dismissed automatic Extrude")
        try require(!dismissFaceExtrude([], automatic: false), "deselection cancelled a manually opened tool")
        try require(!dismissFaceExtrude([], committing: true), "deselection interrupted Apply")
        var faceExtrude = SelectionExtrudeTrigger<String>()
        func selectionReport(_ selection: [String], enabled: Bool = true, idle: Bool = true,
                             document: String = "part", revision: UInt64 = 1) -> Bool {
            faceExtrude.update(document: document, revision: revision, selection: selection,
                               enabled: enabled, idle: idle, source: { $0.hasPrefix("face:") ? "face" : nil })
        }
        try require(!selectionReport([]), "initial workspace opened Extrude")
        try require(selectionReport(["face:top"]), "selecting a face did not activate Extrude")
        try require(!selectionReport(["face:top"]), "a refresh reopened Extrude after cancellation")
        try require(selectionReport(["face:top", "face:side"]), "adding a face did not activate Extrude")
        try require(!selectionReport(["face:top"]), "deselecting a face opened Extrude")
        for selection in [[], ["edge:1"], ["vertex:1"], ["body:1"], ["feature:1"], ["profile:1"], ["face:top", "edge:1"]] {
            try require(!selectionReport(selection), "non-face or mixed selection opened Extrude")
        }
        try require(!selectionReport(["face:top"], enabled: false), "legacy preference opened Extrude")
        try require(!selectionReport(["face:top"]), "enabling the preference reopened an existing selection")
        _ = selectionReport([])
        try require(!selectionReport(["face:top"], idle: false), "face selection interrupted another interaction")
        try require(!selectionReport(["face:top"]), "a blocked selection opened Extrude after its interaction ended")
        try require(!selectionReport(["face:side"], revision: 2), "geometry publication opened Extrude")
        try require(!selectionReport(["face:top"], document: "other", revision: 2), "document replacement opened Extrude")
        _ = selectionReport([], document: "other", revision: 2)
        try require(selectionReport(["face:top"], document: "other", revision: 2), "reselecting a face did not reopen Extrude")
        var sketchExtrude = SelectionExtrudeTrigger<String>()
        func extrudeSource(_ value: String) -> String? {
            let parts = value.split(separator: ":")
            if parts.first == "face" { return "face" }
            if parts.first == "profile", parts.count == 3 { return "profile:" + parts[1] }
            return nil
        }
        func sketchSelectionReport(_ selection: [String], enabled: Bool = true, idle: Bool = true,
                                   revision: UInt64 = 1) -> Bool {
            sketchExtrude.update(document: "part", revision: revision, selection: selection,
                                 enabled: true, idle: idle, source: { value in
                if value.hasPrefix("profile:"), !enabled { return nil }
                return extrudeSource(value)
            })
        }
        _ = sketchSelectionReport([])
        try require(sketchSelectionReport(["profile:1:0"]), "selecting a sketch region did not open Extrude")
        try require(!sketchSelectionReport(["profile:1:0"]), "a refresh reopened sketch Extrude")
        try require(sketchSelectionReport(["profile:1:0", "profile:1:1"]), "adding a sketch region did not open Extrude")
        try require(!sketchSelectionReport(["profile:1:0"]), "removing a sketch region reopened Extrude")
        try require(!sketchSelectionReport(["profile:1:0", "face:top"]), "mixed face and sketch sources opened Extrude")
        try require(!sketchSelectionReport(["profile:1:0", "profile:2:0"]), "regions from different sketches opened Extrude")
        try require(!sketchSelectionReport(["plane:XY"]), "an empty reference plane opened Extrude")
        try require(!sketchSelectionReport(["profile:1:0"], enabled: false), "disabled sketch preference opened Extrude")
        try require(!sketchSelectionReport(["profile:1:0"]), "enabling sketch Extrude reopened the existing selection")
        try require(sketchSelectionReport(["face:top"], enabled: false), "sketch preference disabled automatic face Extrude")
        _ = sketchSelectionReport([])
        try require(!sketchSelectionReport(["profile:1:0"], idle: false), "sketch selection interrupted an active tool")
        try require(!sketchSelectionReport(["profile:1:0"]), "a blocked sketch selection opened Extrude later")
        try require(!sketchSelectionReport(["profile:1:1"], revision: 2), "geometry publication opened sketch Extrude")
        _ = sketchSelectionReport([], revision: 2)
        try require(sketchSelectionReport(["profile:1:0"], revision: 2), "reselecting a sketch region did not reopen Extrude")
        for selection in [["profile:1:0"], ["face:top"]] {
            try require(!SelectionExtrudeTrigger<String>.shouldDismiss(automatic: true, committing: false,
                selection: selection, source: extrudeSource), "an extrusion source dismissed automatic Extrude")
        }
        try require(SelectionExtrudeTrigger<String>.shouldDismiss(automatic: true, committing: false,
            selection: [], source: extrudeSource), "clearing sketch selection left automatic Extrude open")
        try require(!SelectionExtrudeTrigger<String>.shouldDismiss(automatic: true, committing: true,
            selection: [], source: extrudeSource), "clearing sketch selection interrupted Apply")
        try require(!SelectionExtrudeTrigger<String>.shouldDismiss(automatic: false, committing: false,
            selection: [], source: extrudeSource), "clearing sketch selection cancelled a manual tool")
        let dimensionContext = DimensionTapTracker.Context(document: "sketch", geometry: 1, viewport: 2, camera: 3)
        let wideCaption = CGRect(x: -14, y: 58, width: 100, height: 44)
        let helperTarget = DimensionTapTracker.hitFrame(wideCaption, helper: true)
        try require(helperTarget.size == CGSize(width: 44, height: 44) && helperTarget.contains(CGPoint(x: 36, y: 80)),
                    "a wide free-size caption lost its centred native touch target")
        try require(!helperTarget.contains(CGPoint(x: 0, y: 80)), "a wide helper caption stole its vertical edge's contact")
        try require(DimensionTapTracker.hitFrame(wideCaption, helper: false) == wideCaption, "existing dimension targets changed")
        let dimensionPoint = CGPoint(x: 100, y: 80)
        var dimensionTap = DimensionTapTracker()
        dimensionTap.begin(id: 1, point: dimensionPoint, target: "edge:l1", context: dimensionContext)
        try require(dimensionTap.end(id: 1, point: dimensionPoint, context: dimensionContext, cancelled: false) == "edge:l1", "dimension tap did not open its original target")
        dimensionTap.begin(id: 1, point: dimensionPoint, target: "edge:l1", context: dimensionContext)
        try require(dimensionTap.end(id: 1, point: CGPoint(x: 105.5, y: 80), context: dimensionContext, cancelled: false) == nil,
                    "a short committed dimension drag also opened the editor")
        for interruption in ["drag", "return", "multitouch", "cancel", "display", "suspend", "edge"] {
            dimensionTap.cancel()
            dimensionTap.begin(id: 1, point: dimensionPoint, target: interruption == "edge" ? nil : "dimension:d1", context: dimensionContext)
            if interruption == "drag" || interruption == "return" { dimensionTap.move(id: 1, point: CGPoint(x: 107, y: 80)) }
            if interruption == "multitouch" {
                dimensionTap.begin(id: 2, point: dimensionPoint, target: nil, context: dimensionContext)
                _ = dimensionTap.end(id: 2, point: dimensionPoint, context: dimensionContext, cancelled: false)
            }
            if interruption == "suspend" { dimensionTap.cancel() }
            let context = interruption == "display" ? DimensionTapTracker.Context(document: "sketch", geometry: 1, viewport: 2, camera: 4) : dimensionContext
            let end = interruption == "drag" ? CGPoint(x: 107, y: 80) : dimensionPoint
            try require(dimensionTap.end(id: 1, point: end, context: context, cancelled: interruption == "cancel") == nil, "\(interruption) opened a dimension editor")
        }
        // A busy display lane must preserve the Pencil contact through a burst
        // of actual samples, including its final lift, without an input.cancel.
        func pencil(_ phase: InputSample.Phase, _ sequence: UInt64, _ id: UInt64 = 91) -> InputSample {
            var sample = InputSample(source: .pencil, phase: phase, id: id,
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
        let hovering = InputMailbox()
        let hoverToken = hovering.append(pencil(.hover, 1))
        _ = hovering.append(pencil(.began, 2))
        for sequence in UInt64(3)...1000 { _ = hovering.append(pencil(.hover, sequence)) }
        _ = hovering.append(pencil(.ended, 1001))
        guard let hoverToken, let hoverBatch = hovering.take(token: hoverToken) else { throw NSError(domain: "InputChecks", code: 3) }
        try require(!hoverBatch.cancelled && hoverBatch.samples.count <= 128, "hover backlog cancelled a tap or exceeded the buffer bound")
        try require(hoverBatch.samples.filter { $0.phase != .hover }.map(\.phase) == [.began, .ended], "hover backlog lost tap boundaries")
        try require(zip(hoverBatch.samples, hoverBatch.samples.dropFirst()).allSatisfy { $0.sequence < $1.sequence }, "hover eviction reordered input")
        // Hover exits use cancelled/id zero, not a real contact cancellation.
        // Preserve three successive taps (select, clear, select) under that traffic.
        let exitingHover = InputMailbox()
        let exitingToken = exitingHover.append(pencil(.began, 1, 91))
        _ = exitingHover.append(pencil(.ended, 2, 91))
        for sequence in UInt64(3)...400 { _ = exitingHover.append(pencil(.cancelled, sequence, 0)) }
        _ = exitingHover.append(pencil(.began, 401, 92))
        _ = exitingHover.append(pencil(.ended, 402, 92))
        for sequence in UInt64(403)...800 { _ = exitingHover.append(pencil(.cancelled, sequence, 0)) }
        _ = exitingHover.append(pencil(.began, 801, 93))
        _ = exitingHover.append(pencil(.ended, 802, 93))
        guard let exitingToken, let exitingBatch = exitingHover.take(token: exitingToken) else { throw NSError(domain: "InputChecks", code: 3) }
        let exitingContacts = exitingBatch.samples.filter { $0.id != 0 }
        try require(!exitingBatch.cancelled && exitingBatch.samples.count <= 128, "hover exit backlog cancelled successive taps or exceeded the buffer bound")
        try require(exitingContacts.map(\.phase) == [.began, .ended, .began, .ended, .began, .ended]
                    && exitingContacts.map(\.id) == [91, 91, 92, 92, 93, 93], "hover exits lost select/clear/select tap boundaries")
        try require(zip(exitingBatch.samples, exitingBatch.samples.dropFirst()).allSatisfy { $0.sequence < $1.sequence }, "hover exit eviction reordered input")
        let fullContacts = InputMailbox()
        let fullToken = fullContacts.append(pencil(.began, 1))
        for sequence in UInt64(2)...128 { _ = fullContacts.append(pencil(.scroll, sequence)) }
        _ = fullContacts.append(pencil(.hover, 129))
        guard let fullToken, let fullBatch = fullContacts.take(token: fullToken) else { throw NSError(domain: "InputChecks", code: 3) }
        try require(!fullBatch.cancelled && fullBatch.samples.count == 128 && fullBatch.samples.first?.phase == .began,
                    "incoming hover cancelled a full queue of authoritative input")
        let fullExitContacts = InputMailbox()
        let fullExitToken = fullExitContacts.append(pencil(.began, 1))
        for sequence in UInt64(2)...128 { _ = fullExitContacts.append(pencil(.scroll, sequence)) }
        _ = fullExitContacts.append(pencil(.cancelled, 129, 0))
        guard let fullExitToken, let fullExitBatch = fullExitContacts.take(token: fullExitToken) else { throw NSError(domain: "InputChecks", code: 3) }
        try require(!fullExitBatch.cancelled && fullExitBatch.samples.count == 128 && fullExitBatch.samples.first?.phase == .began,
                    "incoming hover exit cancelled a full queue of authoritative input")
        let cancelledContact = InputMailbox()
        let cancelledToken = cancelledContact.append(pencil(.began, 1))
        _ = cancelledContact.append(pencil(.cancelled, 2))
        for sequence in UInt64(3)...1000 { _ = cancelledContact.append(pencil(.cancelled, sequence, 0)) }
        guard let cancelledToken, let cancelledBatch = cancelledContact.take(token: cancelledToken) else { throw NSError(domain: "InputChecks", code: 3) }
        try require(!cancelledBatch.cancelled && cancelledBatch.samples.filter { $0.id != 0 }.map(\.phase) == [.began, .cancelled],
                    "hover exits discarded a real contact cancellation")
        let uncertain = InputMailbox()
        let uncertainToken = uncertain.append(pencil(.began, 1))
        for sequence in UInt64(2)...130 { _ = uncertain.append(pencil(.scroll, sequence)) }
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
        let finishBounds = CGRect(x: 8, y: 8, width: 784, height: 584)
        for anchor in [CGPoint(x: 400, y: 300), CGPoint(x: 15, y: 15), CGPoint(x: 785, y: 585)] {
            let obstacle = CGRect(x: 700, y: 100, width: 90, height: 300)
            guard let frame = ModelValuePlacement.resolve(anchor: anchor, size: CGSize(width: 44, height: 44), bounds: finishBounds, obstacles: [obstacle]) else {
                throw NSError(domain: "WorkspaceChecks", code: 1, userInfo: [NSLocalizedDescriptionKey: "Line finish control disappeared near a viewport corner"])
            }
            try require(finishBounds.contains(frame) && !frame.intersects(obstacle), "Line finish control escaped the canvas or covered chrome")
            try require(!frame.contains(anchor) && hypot(frame.midX - anchor.x, frame.midY - anchor.y) <= 90, "Line finish control covers or is too far from the endpoint")
        }
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
        // The browser starts at its button's top-left and shifts the chrome to
        // its right. Handedness still controls the modelling/navigation rails.
        for size in [CGSize(width: 1194, height: 790), CGSize(width: 834, height: 1140), CGSize(width: 600, height: 420)] {
            for largeText in [false, true] {
                for railRight in [false, true] {
                    let closed = WorkspaceLayout.resolve(size: size, browser: false, inspector: false, railRight: railRight, largeText: largeText)
                    let opened = WorkspaceLayout.resolve(size: size, browser: true, inspector: false, railRight: railRight, largeText: largeText)
                    guard let browser = opened.browser else { throw NSError(domain: "BrowserChecks", code: 1) }
                    try require(browser.minX == closed.browserAccess.minX && browser.minY == closed.browserAccess.minY, "browser does not open from its button")
                    for frame in [opened.rail, opened.topBar, opened.navigation, opened.history, opened.browserAccess, opened.status] {
                        try require(!frame.intersects(browser), "browser covers adjacent workspace controls")
                    }
                    try require(opened.canvas == closed.canvas, "unpinned browser changes picking coordinates")
                    let legacy = WorkspaceLayout.resolve(size: size, browser: true, inspector: false, railRight: railRight, largeText: largeText, leftBrowser: false)
                    try require(legacy.browser?.minY == legacy.rail.minY, "legacy anchored browser preference was lost")
                    try require(legacy.canvas == closed.canvas, "legacy browser changes the viewport")
                    cases += 1
                }
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
        // Bottom controls use the home-indicator area by default. Top/side
        // insets, the keyboard boundary and the previous spacing remain usable.
        for size in [CGSize(width: 1366, height: 1024), CGSize(width: 1194, height: 834), CGSize(width: 834, height: 1194),
                     CGSize(width: 600, height: 420), CGSize(width: 390, height: 844)] {
            for keyboard in [false, true] {
                let viewport = CGRect(x: 0, y: 0, width: size.width, height: size.height - (keyboard ? 300 : 0))
                let safe = CGRect(x: 12, y: 24, width: size.width - 24, height: size.height - 44)
                let oldChrome = CGRect(x: safe.minX, y: safe.minY, width: safe.width,
                                    height: max(0, min(safe.maxY, viewport.maxY) - safe.minY))
                let chrome = WorkspaceLayout.chromeBounds(safeArea: safe, viewport: viewport)
                try require(chrome.minX == safe.minX && chrome.minY == safe.minY && chrome.width == safe.width,
                            "bottom extension changed top/side system insets")
                try require(chrome.maxY == viewport.maxY, "bottom safe area is still reserved for chrome")
                let restored = WorkspaceLayout.chromeBounds(safeArea: safe, viewport: viewport, useBottomSafeArea: false)
                try require(restored == oldChrome, "bottom spacing preference does not restore the previous bounds")
                for available in [chrome, restored] {
                    for compactChrome in [false, true] {
                        for historyExpanded in [false, true] {
                            let layout = WorkspaceLayout.resolve(size: available.size, browser: true, inspector: true,
                                railRight: false, largeText: false, compactChrome: compactChrome, historyExpanded: historyExpanded)
                                .placed(in: available, viewport: viewport)
                            try require(layout.featureHistory.maxY == available.maxY - 8,
                                        "history does not use the available bottom space")
                            try require(layout.featureHistory.intersects(safe) && viewport.contains(layout.featureHistory),
                                        "history escaped the window/keyboard boundary")
                            try require(layout.topBar.minY >= safe.minY, "header moved under the status bar")
                            cases += 1
                        }
                    }
                }
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
                                try require(chrome.contains(frame), "full-window layout moved controls outside usable bounds")
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
        // Only the floating header may enter the centre of the status strip.
        // Its full touch target and all other chrome/viewport frames survive.
        for width: CGFloat in [600, 834, 1194, 1366] {
            for topInset: CGFloat in [0, 24, 50, 80] {
                for largeText in [false, true] {
                    for browser in [false, true] {
                        let viewport = CGRect(x: 0, y: 0, width: width, height: 700)
                        let chrome = CGRect(x: 0, y: topInset, width: width, height: 700 - topInset)
                        let source = WorkspaceLayout.resolve(size: chrome.size, browser: browser, inspector: true,
                            railRight: false, largeText: largeText)
                        let old = source.placed(in: chrome, viewport: viewport)
                        let raised = source.placed(in: chrome, viewport: viewport, useTopSafeArea: true)
                        let fits = !largeText && topInset >= 20 && topInset <= 64 &&
                            old.topBar.minX >= 240 && old.topBar.maxX <= viewport.maxX - 240
                        if fits {
                            try require(raised.topBar.minY == 4 && raised.topBar.minY < chrome.minY,
                                        "header did not use the available upper strip")
                        } else {
                            try require(raised.topBar == old.topBar, "crowded or accessibility header entered system UI")
                        }
                        try require(raised.topBar.size == old.topBar.size && raised.topBar.minX == old.topBar.minX,
                                    "raising the header changed its content or touch bounds")
                        try require(viewport.contains(raised.topBar), "raised header escaped the window")
                        try require(raised.canvas == old.canvas && raised.status == old.status && raised.rail == old.rail &&
                            raised.navigation == old.navigation && raised.browser == old.browser && raised.inspector == old.inspector &&
                            raised.history == old.history && raised.featureHistory == old.featureHistory && raised.browserAccess == old.browserAccess,
                                    "raising the header moved other controls or rebased rendering/picking")
                        for frame in [raised.rail, raised.navigation, raised.browserAccess] + [raised.browser, raised.inspector].compactMap({ $0 }) {
                            try require(!raised.topBar.intersects(frame), "raised header covers another control")
                        }
                        let legacy = WorkspaceLayout.resolve(size: chrome.size, browser: browser, inspector: true,
                            railRight: false, largeText: largeText, compactChrome: false)
                        try require(legacy.placed(in: chrome, viewport: viewport, useTopSafeArea: true).topBar ==
                            legacy.placed(in: chrome, viewport: viewport).topBar, "top strip preference changed the docked toolbar")
                        cases += 1
                    }
                }
            }
        }
        let numericPanel = WorkspaceLayout.resolve(size: CGSize(width: 1194, height: 790), browser: false, inspector: true,
                                                   railRight: false, largeText: false, inspectorContentHeight: 500, navigationContentHeight: 146)
        try require(numericPanel.inspector?.height == 500, "simple numeric editor became a full-height inspector")
        try require(numericPanel.navigation.height == 146, "navigation surface retains unused permanent space")
        for size in [CGSize(width: 1194, height: 790), CGSize(width: 744, height: 790), CGSize(width: 500, height: 400)] {
            for right in [false, true] {
                for large in [false, true] {
                    let closed = WorkspaceLayout.resolve(size: size, browser: false, inspector: false, railRight: right, largeText: large)
                    let opened = WorkspaceLayout.resolve(size: size, browser: false, inspector: true, railRight: right, largeText: large,
                                                         inspectorContentHeight: 620, inspectorPreferredWidth: 380)
                    try require(closed.canvas == opened.canvas, "compact inspector changed the Metal picking/camera bounds")
                    if let panel = opened.inspector {
                        try require(CGRect(origin: .zero, size: size).contains(panel), "compact inspector escaped a narrow/keyboard-reduced window")
                        try require(panel.width <= 380, "compact inspector exceeded its requested readable width")
                    }
                }
            }
        }
        try require(WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: false, numericEditActive: false), "document shortcuts unavailable at rest")
        try require(!WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: true, numericEditActive: false), "text undo routes to document")
        try require(!WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: false, numericEditActive: true), "numeric draft permits unrelated history action")
        // Scrubbing snaps in content coordinates and preserves the final Pencil
        // position when evaluation is slower than the incoming drag samples.
        let centres: [CGFloat] = [30, 78, 126]
        for (x, position) in [(CGFloat(-100), 0), (8, 0), (31, 1), (80, 2), (140, 3), (1000, 3)] {
            try require(HistoryScrubGeometry.position(at: x, centres: centres) == position, "history drag snapped to the wrong feature gap")
            try require(HistoryScrubGeometry.position(at: x + 200, centres: centres.map { $0 + 200 }) == position, "scroll offset changed the dragged feature boundary")
        }
        try require(HistoryScrubGeometry.position(at: .nan, centres: centres) == nil, "nonfinite marker contact was accepted")
        try require(HistoryScrubGeometry.position(at: 10, centres: []) == nil, "empty history exposed a draggable boundary")
        try require(HistoryScrubGeometry.position(at: 10, centres: [30, 20]) == nil, "unordered feature frames were accepted")
        var scrub = HistoryScrubQueue()
        scrub.begin(at: 3); scrub.move(to: 2)
        guard let firstScrub = scrub.next() else { throw NSError(domain: "HistoryScrub", code: 1) }
        scrub.move(to: 1); scrub.move(to: 0); scrub.end()
        try require(scrub.next() == nil, "a drag queued multiple concurrent evaluations")
        scrub.complete(firstScrub, succeeded: true)
        guard let finalScrub = scrub.next() else { throw NSError(domain: "HistoryScrub", code: 2) }
        try require(finalScrub.position == 0, "Pencil lift lost its latest requested history position")
        try require(!scrub.complete(firstScrub, succeeded: true), "an old completion accepted a second time")
        scrub.complete(finalScrub, succeeded: true)
        try require(scrub.active, "marker stopped tracking before the authoritative workspace acknowledged it")
        scrub.acknowledge(0)
        try require(!scrub.active, "final marker position never settled")
        // Reversing to the starting position must still run after a slower
        // earlier request, even if the cached workspace still reports that start.
        scrub.begin(at: 3); scrub.move(to: 1)
        guard let reverse = scrub.next() else { throw NSError(domain: "HistoryScrub", code: 3) }
        scrub.move(to: 3); scrub.end(); scrub.complete(reverse, succeeded: true); scrub.acknowledge(3)
        guard let returnToStart = scrub.next() else { throw NSError(domain: "HistoryScrub", code: 4) }
        try require(returnToStart.position == 3, "stale workspace echo discarded the final reverse drag")
        scrub.cancel(); scrub.begin(at: 0); scrub.move(to: 2)
        guard let replacement = scrub.next() else { throw NSError(domain: "HistoryScrub", code: 5) }
        try require(!scrub.complete(returnToStart, succeeded: true) && scrub.inFlight == replacement, "completion from a cancelled document moved its replacement")
        scrub.complete(replacement, succeeded: false)
        try require(!scrub.active && scrub.next() == nil, "failed timeline command continued the drag")
        // A long View page must scroll above History, including in short windows;
        // a short page should retain its natural height without empty space.
        for container in [CGRect(x: 0, y: 24, width: 1366, height: 976),
                          CGRect(x: 0, y: 24, width: 1024, height: 720),
                          CGRect(x: 0, y: 24, width: 390, height: 400)] {
            let source = CGRect(x: container.maxX - 100, y: container.minY + 8, width: 88, height: 44)
            let large = ViewPopoverLayout.size(contentHeight: 1400, container: container, source: source)
            try require(large.height <= 560 && source.maxY + large.height + 80 <= container.maxY,
                        "View covered the workspace bottom controls")
            try require(large.width <= container.width - 24, "View exceeded a narrow window")
            let short = ViewPopoverLayout.size(contentHeight: 180, container: container, source: source)
            try require(short.height == 180, "short View page expanded unnecessarily")
        }
        // Decode the existing Rust viewport projection, including automatic
        // effective visibility; every subset must advertise the correct action.
        for mask in 0..<128 {
            let visible = (0..<7).map { mask & (1 << $0) != 0 }
            let json = try JSONSerialization.data(withJSONObject: ["result": [
                "origin": ["visible": visible, "choosing": mask % 2 == 0],
                "grid": ["visible": true, "integrated_axes": mask % 2 == 0], "planar": false, "camera": ["fov": 0]
            ]])
            let projection = try JSONDecoder().decode(ViewControlsPresentation.Envelope.self, from: json).result
            let expected: ViewControlsPresentation.Visibility = mask == 0 ? .none : (mask == 127 ? .all : .some)
            try require(projection.visibility == expected, "reference master lost an effective mixed state")
            try require(projection.masterTarget == (mask != 127), "reference master must show every mixed or hidden set")
            for (index, item) in ["O", "X", "Y", "Z", "XY", "XZ", "YZ"].enumerated() {
                try require(projection.originVisible(item) == visible[index], "individual reference did not match the master source")
            }
            try require(projection.grid.visible, "reference projection changed the independent grid")
            try require(projection.grid.integrated_axes == (mask % 2 == 0), "grid axis preference did not decode from authoritative viewport")
            cases += 1
        }
        // Selected View buttons follow the actual camera, including orbit and
        // roll away from a standard orientation, rather than the last button.
        let orientations: [(String, Double, Double)] = [
            ("front", 0, 0), ("back", .pi, 0), ("left", .pi / 2, 0), ("right", -.pi / 2, 0),
            ("top", 0, .pi / 2 - 0.000001), ("bottom", 0, -.pi / 2 + 0.000001),
            ("isometric", -.pi / 4, 0.6154797086703874)
        ]
        for (name, yaw, pitch) in orientations {
            for fov in [0.0, 0.6] {
                let cameraJSON = try JSONSerialization.data(withJSONObject: ["fov": fov, "yaw": yaw + 2 * Double.pi, "pitch": pitch, "roll": 0])
                let camera = try JSONDecoder().decode(ViewControlsPresentation.Camera.self, from: cameraJSON)
                try require(camera.standardView == name, "View selection does not match native camera orientation")
                cases += 1
            }
        }
        for values in [["fov": 0.0, "yaw": 0.3, "pitch": 0.2, "roll": 0.0],
                       ["fov": 0.0, "yaw": 0.0, "pitch": 0.0, "roll": 0.2], ["fov": 0.0]] {
            let json = try JSONSerialization.data(withJSONObject: values)
            let camera = try JSONDecoder().decode(ViewControlsPresentation.Camera.self, from: json)
            try require(camera.standardView == nil, "free-orbit or unavailable camera falsely selected a standard View")
        }
        // Long translations and Dynamic Type enlarge just the labelled controls.
        let ordinaryViewLayout = WorkspaceLayout.resolve(size: CGSize(width: 1194, height: 790), browser: false,
            inspector: false, railRight: false, largeText: false, historyToggleWidth: 112, viewButtonSize: CGSize(width: 96, height: 52))
        let largeViewLayout = WorkspaceLayout.resolve(size: CGSize(width: 1194, height: 790), browser: false,
            inspector: false, railRight: false, largeText: false, historyToggleWidth: 220, viewButtonSize: CGSize(width: 180, height: 52))
        try require(ordinaryViewLayout.rail == largeViewLayout.rail && ordinaryViewLayout.canvas == largeViewLayout.canvas &&
                    ordinaryViewLayout.topBar == largeViewLayout.topBar && ordinaryViewLayout.browserAccess == largeViewLayout.browserAccess,
                    "View/History labels moved the left rail, document header or rendering coordinates")
        try require(largeViewLayout.featureHistory.width == 220 && largeViewLayout.viewAccess.width == 180 &&
                    largeViewLayout.navigation.width == ordinaryViewLayout.navigation.width,
                    "label sizing discarded measured text width or widened the navigation icons")
        // History starts as one touch target. Its expanded strip stays above the
        // keyboard, clear of chrome, and never rebases the floating Metal view.
        for size in [CGSize(width: 1194, height: 790), CGSize(width: 834, height: 1140),
                     CGSize(width: 390, height: 420), CGSize(width: 320, height: 240)] {
            for compactChrome in [false, true] {
                for largeText in [false, true] {
                    for railRight in [false, true] {
                        for browser in [false, true] {
                            let closed = WorkspaceLayout.resolve(size: size, browser: browser, inspector: !browser,
                                railRight: railRight, largeText: largeText, compactChrome: compactChrome)
                            let opened = WorkspaceLayout.resolve(size: size, browser: browser, inspector: !browser,
                                railRight: railRight, largeText: largeText, compactChrome: compactChrome, historyExpanded: true)
                            let bounds = CGRect(origin: .zero, size: size)
                            try require(closed.featureHistory.width == 112 && closed.featureHistory.height >= 44, "collapsed history does not fit its text and icon")
                            try require(opened.featureHistory.width >= size.width - 16, "expanded history does not expose the full strip")
                            for layout in [closed, opened] {
                                try require(bounds.contains(layout.featureHistory), "history escaped the keyboard-reduced window")
                                if compactChrome {
                                    try require(bounds.contains(layout.viewAccess), "View escaped the available window")
                                    for other in [layout.topBar, layout.navigation, layout.browserAccess, layout.rail] {
                                        try require(!layout.viewAccess.intersects(other), "standalone View overlaps other chrome: size=\(size) railRight=\(railRight) browser=\(browser) expanded=\(layout.featureHistory.width > 112) view=\(layout.viewAccess) other=\(other)")
                                    }
                                    try require(layout.viewAccess.maxY <= layout.navigation.minY, "View is not above the navigation controls")
                                }
                                for frame in [layout.topBar, layout.rail, layout.navigation, layout.history, layout.status] + [layout.browser, layout.inspector].compactMap({ $0 }) {
                                    try require(!frame.intersects(layout.featureHistory), "history covers chrome: size=\(size) floating=\(compactChrome) large=\(largeText) railRight=\(railRight) browser=\(browser) frame=\(frame) history=\(layout.featureHistory)")
                                }
                            }
                            if compactChrome { try require(opened.canvas == closed.canvas, "expanding history changed rendering/picking coordinates") }
                            cases += 1
                        }
                    }
                }
            }
        }
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
        // Preview validation follows the exact draft revision. UIKit owns raw
        // text and selection; the policy never parses or normalises expressions.
        var queuedPreviews: [DispatchWorkItem] = []
        var previewValues: [Int] = []
        let debouncer = DraftPreviewDebouncer { queuedPreviews.append($0) }
        debouncer.schedule { previewValues.append(1) }
        debouncer.schedule { previewValues.append(2) }
        queuedPreviews.forEach { $0.perform() }; queuedPreviews.removeAll()
        try require(previewValues == [2], "rapid draft edits evaluated superseded previews")
        debouncer.schedule { previewValues.append(3) }
        debouncer.cancel()
        queuedPreviews.forEach { $0.perform() }; queuedPreviews.removeAll()
        try require(previewValues == [2], "Apply or Cancel allowed a delayed preview to run")
        debouncer.schedule { previewValues.append(4) }
        queuedPreviews.forEach { $0.perform() }
        try require(previewValues == [2, 4], "a new draft could not preview after cancellation")
        var numericDraft = NumericDraftValidation()
        try require(!numericDraft.isValidated, "a new numeric draft was already validated")
        let originalNumericRevision = numericDraft.revision
        numericDraft.changed() // a fast edit while the first preview is pending
        try require(!numericDraft.complete(revision: originalNumericRevision, valid: true), "an older preview validated newer numeric input")
        try require(!numericDraft.isValidated, "a stale numeric preview validated newer input")
        let currentNumericRevision = numericDraft.revision
        try require(numericDraft.complete(revision: currentNumericRevision, valid: true) && numericDraft.isValidated, "the current numeric preview did not enable Apply")
        numericDraft.changed() // edit away, then back to exactly the original text
        numericDraft.changed()
        try require(!numericDraft.complete(revision: currentNumericRevision, valid: true) && !numericDraft.isValidated, "retyping old text reused its earlier validation")
        try require(numericDraft.complete(revision: numericDraft.revision, valid: false) && !numericDraft.isValidated, "invalid expression enabled Apply")
        try require(numericDraft.complete(revision: numericDraft.revision, valid: true) && numericDraft.isValidated, "correcting an invalid expression did not recover validation")
        try require(!numericDraft.complete(revision: originalNumericRevision, valid: false) && numericDraft.isValidated, "a stale error removed newer validation")
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
        let operationJSON = Data(#"{"key":"operation","label":"Operation","kind":"inline_choice","options":[{"label":"New Body","value":"new"},{"label":"Union","value":"join"},{"label":"Intersect","value":"intersect"},{"label":"Subtract","value":"cut"}]}"#.utf8)
        let operation = try JSONDecoder().decode(ModellingResponse<ModelWorkspace>.Field.self, from: operationJSON)
        let allFields = (model.fields ?? []) + [operation]
        let ordered = InspectorPresentation.orderedFields(allFields, compact: true)
        try require(ordered.map(\.key) == ["profile", "distance", "operation", "direction", "sections"], "compact Extrude hid or reordered additional references")
        try require(ordered[2].options?.map(\.value) == [.string("new"), .string("join"), .string("intersect"), .string("cut")], "boolean presentation changed the engine's option IDs/order")
        try require(InspectorPresentation.orderedFields(allFields, compact: false).map(\.key) == ["operation", "profile", "distance", "direction", "sections"], "previous inspector ordering is unavailable")
        try require(InspectorPresentation.choiceIcon(field: "operation", value: .string("new_body")) == "body", "feature edits lost the distinct New Body icon")
        try require(InspectorPresentation.choiceIcon(field: "other", value: .string("cut")) == nil, "operation artwork was assigned to an unrelated choice")
        try require(model.workspace.modelling.timeline.first?.id == 7 && model.workspace.modelling.parameters.first?.expression == "20 mm", "feature history or named parameters were lost")
        try require(model.workspace.modelling.activeFeatureCount == 1, "older timeline payload lost its end-position fallback")
        let rolledHistory = Data(#"{"tools":[],"timeline":[{"id":7,"name":"Sketch","kind":"Sketch","order":0,"suppressed":false}],"position":0,"parameters":[]}"#.utf8)
        let history = try JSONDecoder().decode(ModellingPresentation.self, from: rolledHistory)
        try require(history.activeFeatureCount == 0 && history.timeline.count == 1, "rollback position must preserve future feature icons")
        try require(history.documentPosition(for: 1) == 1, "older timeline payload lost its document-position fallback")
        let faceHistoryJSON = Data(#"{"tools":[],"timeline":[{"id":1,"name":"Box","kind":"BoxFeature","order":0,"document_order":0,"suppressed":false},{"id":3,"name":"Extrude1","kind":"ExtrudeFeature","order":1,"document_order":2,"suppressed":false},{"id":5,"name":"Extrude2","kind":"ExtrudeFeature","order":2,"document_order":4,"suppressed":false}],"position":2,"parameters":[]}"#.utf8)
        let faceHistory = try JSONDecoder().decode(ModellingPresentation.self, from: faceHistoryJSON)
        try require(faceHistory.activeFeatureCount == 2, "internal boundaries changed the visible rollback position")
        for (visible, document) in [0, 1, 3, 5].enumerated() {
            try require(faceHistory.documentPosition(for: visible) == document, "history scrub stopped on an internal boundary")
        }
        try require(faceHistory.documentPosition(for: -1) == nil && faceHistory.documentPosition(for: 4) == nil, "invalid history positions were accepted")
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
        try require(projectA.file.pathExtension == "solidcraft" && projectB.file.pathExtension == "solidcraft", "native extension compatibility changed the library save default")
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
        for nativeExtension in ["SC", "solidcraft"] {
            let nativeSource = libraryRoot.appendingPathComponent("Desktop design").appendingPathExtension(nativeExtension)
            let nativeBytes = Data("native container staging fixture".utf8)
            try nativeBytes.write(to: nativeSource)
            let stagedNative = try ProjectLibrary.stageImport(nativeSource)
            let stagedNativeBytes = try Data(contentsOf: stagedNative)
            let sourceNativeBytes = try Data(contentsOf: nativeSource)
            try require(stagedNative != nativeSource && stagedNative.lastPathComponent == nativeSource.lastPathComponent, "native import overwrote its source or changed its extension")
            try require(stagedNativeBytes == nativeBytes && sourceNativeBytes == nativeBytes, "native import did not preserve both source and staged bytes")
            try FileManager.default.removeItem(at: stagedNative.deletingLastPathComponent())
        }
        let desktopFile = projectB.file.deletingPathExtension().appendingPathExtension("sc")
        try FileManager.default.moveItem(at: projectB.file, to: desktopFile)
        let desktopProjects = try library.projects()
        try require(desktopProjects.count == 1 && desktopProjects.first?.id == projectB.id, "library did not discover the current desktop native extension")
        try require(desktopProjects.first?.file.resolvingSymlinksInPath().standardizedFileURL == desktopFile.resolvingSymlinksInPath().standardizedFileURL, "desktop native discovery changed the project path")
        let desktopBytes = try Data(contentsOf: desktopFile)
        try require(desktopProjects.first?.file.pathExtension == "sc" && desktopBytes == Data("second design".utf8), "desktop native discovery lost its extension or bytes")
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
        let browserNodes = [
            BrowserSearchNode(id: "root", parent: nil, name: "SolidCraft"),
            BrowserSearchNode(id: "bodies", parent: "root", name: "Bodies"),
            BrowserSearchNode(id: "folder", parent: "bodies", name: "Frame"),
            BrowserSearchNode(id: "a:Post", parent: "folder", name: "Póst"),
            BrowserSearchNode(id: "b:Post", parent: "bodies", name: "Post"),
            BrowserSearchNode(id: "other", parent: "bodies", name: "Base")
        ]
        let closed: Set<String> = ["bodies", "folder"]
        let ordinaryBrowser = BrowserSearchProjection.make(nodes: browserNodes, query: "", collapsed: closed)
        try require(ordinaryBrowser.ids == ["root", "bodies"], "unfiltered browser ignores stored collapsed state")
        let found = BrowserSearchProjection.make(nodes: browserNodes, query: "  POST  ", collapsed: closed)
        try require(found.ids == ["root", "bodies", "folder", "a:Post", "b:Post"], "find loses order, ancestor context or duplicate occurrence identities")
        try require(found.matches == ["a:Post", "b:Post"], "find counts context or loses case/diacritic matches")
        try require(BrowserSearchProjection.make(nodes: browserNodes, query: "frame", collapsed: closed).ids == ["root", "bodies", "folder"], "matching folder incorrectly includes unrelated descendants")
        try require(BrowserSearchProjection.make(nodes: browserNodes, query: "missing", collapsed: closed).ids.isEmpty, "no-results still shows unrelated roots")
        try require(BrowserSearchProjection.make(nodes: browserNodes, query: "   ", collapsed: closed).ids == ordinaryBrowser.ids, "whitespace query fails to restore tree")
        let renamedBrowser = browserNodes.filter { $0.id != "a:Post" }.map {
            BrowserSearchNode(id: $0.id, parent: $0.parent, name: $0.id == "b:Post" ? "Rail" : $0.name)
        }
        try require(BrowserSearchProjection.make(nodes: renamedBrowser, query: "post", collapsed: closed).matches.isEmpty, "renamed/deleted row remains in results")
        try require(BrowserSearchProjection.make(nodes: browserNodes, query: "post", collapsed: closed).ids == found.ids, "undo/refresh does not recover original identities")
        let cycle = [BrowserSearchNode(id: "a", parent: "b", name: "post"), BrowserSearchNode(id: "b", parent: "a", name: "post")]
        try require(BrowserSearchProjection.make(nodes: cycle, query: "post", collapsed: []).ids.isEmpty, "malformed browser cycle invented a root")
        let largeTree = [BrowserSearchNode(id: "root", parent: nil, name: "Root")] + (0..<10000).map {
            BrowserSearchNode(id: String($0), parent: "root", name: "Part \($0)")
        }
        try require(BrowserSearchProjection.make(nodes: largeTree, query: "part 9999", collapsed: ["root"]).ids == ["root", "9999"], "large-tree search lost identity or collapsed ancestor")
        for width: CGFloat in [200, 280, 320, 380] {
            for height: CGFloat in [44, 56, 90] {
                let bounds = CGRect(x: 0, y: 0, width: width, height: height)
                let eye = BrowserRowColumns.eye(in: bounds), disclosure = BrowserRowColumns.disclosure(in: bounds)
                try require(bounds.contains(eye) && bounds.contains(disclosure) && !eye.intersects(disclosure), "browser eye/disclosure targets overlap or clip")
                try require(eye.width == 44 && eye.height == 44 && disclosure.width == 44 && BrowserRowColumns.width == 88, "browser controls lost fixed touch columns")
            }
        }
        let bookmark = BrowserScrollBookmark(id: "a:Post", displacement: -8, fallback: 100)
        try require(bookmark.restoredY(rowY: 320, maximum: 500) == 312, "search restoration lost row-relative scroll")
        try require(bookmark.restoredY(rowY: nil, maximum: 50) == 50, "deleted scroll anchor not clamped")
        try require(bookmark.restoredY(rowY: 0, maximum: 500) == 0, "scroll restoration escaped top")
        print("Workspace layout/focus/command decoding checks PASS: \(cases) adaptive cases; no simulator or device launch")
    }
}
