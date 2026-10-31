import Foundation

/// Native form presentation preserves the engine's opaque option and reference values.
/// These helpers choose controls and edit lists; they do not interpret CAD semantics.
enum ModellingFormPresentation {
    static let automaticExtrudeDepthKey = "SolidCraft.workspace.automaticExtrudeDepth"

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
