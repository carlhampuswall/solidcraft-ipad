import Foundation

/// Native form presentation preserves the engine's opaque option and reference values.
/// These helpers choose controls and edit lists; they do not interpret CAD semantics.
enum ModellingFormPresentation {
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
