import Foundation

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

    func overflow(from available: [String]) -> [String] {
        var seen = Set<String>()
        return available.filter { !contains($0) && seen.insert($0).inserted }
    }
}
