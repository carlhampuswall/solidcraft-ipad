import Foundation

/// Native chrome only. Group membership is independent of tool availability,
/// and saved placement retains the user's order within each family.
enum ToolGrouping {
    static func family(_ id: String) -> String {
        switch id {
        case "sketch.create", "modelling", "modify", "pattern", "post_radius": return "modelling"
        case "assembly": return "assembly"
        case "navigation.home", "navigation.projection": return "view"
        case "navigation.constraints", "navigation.dimensions", "navigation.construction": return "sketch-settings"
        case "edit.undo", "edit.redo": return "history"
        default:
            if id.hasPrefix("sketch.") { return "sketch" }
            if id.hasPrefix("view.") { return "view" }
            if ["component.", "joint.", "occurrence."].contains(where: id.hasPrefix) { return "assembly" }
            if ["solid.", "construct.", "parameters."].contains(where: id.hasPrefix) { return "modelling" }
            // Select, overflow and Properties each keep their own surface.
            return id
        }
    }

    static func ordered(_ ids: [String], enabled: Bool) -> [String] {
        guard enabled else { return ids }
        var families: [String] = []
        var members: [String: [String]] = [:]
        for id in ids {
            let family = family(id)
            if members[family] == nil { families.append(family) }
            members[family, default: []].append(id)
        }
        return families.flatMap { members[$0] ?? [] }
    }
}

/// App preferences only: tool placement never changes a CAD document or command.
struct QuickAccessLayout: Codable, Equatable {
    private(set) var order: [String]
    private var knownDefaults: [String]

    init(defaults: [String], saved: Data? = nil) {
        let restored = saved.flatMap { try? JSONDecoder().decode(Self.self, from: $0) }
        var seen = Set<String>()
        order = (restored?.order ?? defaults).filter { !$0.isEmpty && seen.insert($0).inserted }
        let known = Set(restored?.knownDefaults ?? defaults)
        for id in defaults where !known.contains(id) && seen.insert(id).inserted { order.append(id) }
        knownDefaults = defaults
    }

    func contains(_ id: String) -> Bool { order.contains(id) }
    var saved: Data? { try? JSONEncoder().encode(self) }

    mutating func add(_ id: String) {
        guard !id.isEmpty, !contains(id) else { return }
        order.append(id)
    }

    mutating func remove(_ id: String) { order.removeAll { $0 == id } }

    /// Insert relative to a visible neighbor, preserving the other mode's tools.
    mutating func move(_ id: String, beside neighbor: String, after: Bool) {
        guard id != neighbor, contains(id), contains(neighbor) else { return }
        remove(id)
        guard let index = order.firstIndex(of: neighbor) else { return }
        order.insert(id, at: index + (after ? 1 : 0))
    }

    /// Commands covered by a category menu stay there, even when that menu's
    /// button is removed from the bar and moves into overflow itself.
    func overflow(from available: [String], excluding groupedCommands: Set<String> = []) -> [String] {
        var seen = Set<String>()
        return available.filter { !contains($0) && !groupedCommands.contains($0) && seen.insert($0).inserted }
    }
}
