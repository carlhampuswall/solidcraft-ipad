import Foundation

/// Native form presentation preserves the engine's opaque option and reference values.
/// These helpers choose controls and edit lists; they do not interpret CAD semantics.
enum ModellingFormPresentation {
    static let automaticExtrudeDepthKey = "SolidCraft.workspace.automaticExtrudeDepth"

    static func automaticallyPreviews(selectionField: String?) -> Bool {
        guard let selectionField else { return true }
        return !["target", "tools"].contains(selectionField)
    }

    static func needsCombineBodies(_ params: [String: PresentationJSON]) -> Bool {
        guard params["keep_tools"] != nil else { return false }
        guard case .string(let target) = params["target"], !target.isEmpty,
              case .array(let tools) = params["tools"], !tools.isEmpty else { return true }
        return tools.contains { value in
            guard case .string(let name) = value else { return true }
            return name.isEmpty || name == target
        }
    }

    static func usesDirectSurfaceControls<Workspace>(_ fields: [ModellingResponse<Workspace>.Field], compact: Bool) -> Bool {
        compact && fields.contains { $0.key == "edges" || $0.key == "faces" }
            && fields.contains { ["radius", "distance", "thickness"].contains($0.key) }
    }

    static func visibleSurfaceField(_ key: String, params: [String: PresentationJSON]) -> Bool {
        guard let mode = params["chamfer_mode"] else { return true }
        if key == "distance2" { return mode == .string("two_distances") }
        if key == "angle" { return mode == .string("angle") }
        return true
    }

    static func surfaceChoiceIcon(field: String, value: PresentationJSON) -> String? {
        guard case .string(let id) = value else { return nil }
        switch (field, id) {
        case ("chamfer_mode", "equal"): return "chamfer"
        case ("chamfer_mode", "two_distances"): return "sketch_chamfer_two_distances"
        case ("chamfer_mode", "angle"): return "sketch_chamfer_angle"
        case ("type", "constant"), ("type", "chord"), ("type", "curvature"): return "fillet"
        case ("direction", "inside"), ("direction", "outside"): return "shell"
        case ("direction", "both"): return "offset_two_sides"
        default: return nil
        }
    }

    static func initialParameters(operation: String?, feature: UInt64?, automatic: Bool, legacyDepth: Bool,
                                  supplied: [String: PresentationJSON]?) -> [String: PresentationJSON]? {
        guard automatic, operation == "solid.extrude", feature == nil, !legacyDepth else { return supplied }
        var params = supplied ?? [:]
        if params["distance"] == nil { params["distance"] = .string("") }
        return params
    }

    static func needsExtrusionDistance(_ params: [String: PresentationJSON]) -> Bool {
        guard params["profile"] != nil, params["through_all"] != .bool(true),
              case .string(let distance) = params["distance"] else { return false }
        return distance.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    static func booleanValue(kind: String, options: [PresentationJSON], selected: PresentationJSON?) -> Bool? {
        guard kind == "choice", options.count == 2,
              options.contains(.bool(false)), options.contains(.bool(true)),
              let selected, case .bool(let value) = selected else { return nil }
        return value
    }

    static func removing(at index: Int, from values: [PresentationJSON]) -> [PresentationJSON]? {
        guard values.indices.contains(index) else { return nil }
        var result = values
        result.remove(at: index)
        return result
    }

    static func moving(from source: Int, to destination: Int, in values: [PresentationJSON]) -> [PresentationJSON]? {
        guard values.indices.contains(source), values.indices.contains(destination) else { return nil }
        var result = values
        let value = result.remove(at: source)
        result.insert(value, at: destination)
        return result
    }
}
