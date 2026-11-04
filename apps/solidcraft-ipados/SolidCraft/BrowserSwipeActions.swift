import Foundation

/// Swipe intent only. The existing engine commands own deletion and grouping.
enum BrowserSwipeAction: Equatable {
    case delete
    case rename
    case newFolder
    case moveToFolder
}

enum BrowserSwipeActions {
    static let preferenceKey = "SolidCraft.workspace.browserSwipeActions"
    static func enabled(_ stored: Bool?) -> Bool { stored ?? true }

    static func leading(enabled: Bool, available: Bool, rename: Bool, folder: Bool, item: Bool) -> [BrowserSwipeAction] {
        guard enabled && available else { return [] }
        var actions: [BrowserSwipeAction] = []
        if rename { actions.append(.rename) }
        if folder { actions.append(.newFolder) }
        if folder && item { actions.append(.moveToFolder) }
        return actions
    }

    static func trailing(enabled: Bool, available: Bool, target: BrowserDeleteTarget?) -> [BrowserSwipeAction] {
        enabled && available && target != nil ? [.delete] : []
    }
}
/// Explicit row targets avoid deleting another item just because it is selected.
enum BrowserDeleteTarget: Equatable {
    case body(String)
    case feature(UInt64)
    case occurrence([UInt64])
    case component(UInt64)
    case joint(UInt64)
    case folder(UInt64)

    static func resolve(kind: String, entityType: String?, name: String?, feature: UInt64?, path: [UInt64]?,
                        component: UInt64?, joint: UInt64?, group: UInt64?) -> Self? {
        switch kind {
        case "body":
            if entityType == "body", let name, !name.isEmpty { return .body(name) }
        case "sketch", "feature":
            if entityType == "feature", let feature { return .feature(feature) }
        case "occurrence":
            if entityType == "occurrence", let path, !path.isEmpty { return .occurrence(path) }
        case "component":
            if let component, component != 0 { return .component(component) }
        case "joint":
            if let joint { return .joint(joint) }
        case "folder":
            if let group { return .folder(group) }
        default: break
        }
        return nil
    }

    var command: String {
        if case .folder = self { return "browser.ungroup" }
        return "selection.delete"
    }

    var parameters: [String: PresentationJSON] {
        switch self {
        case .body(let name): return ["items": .array([.object(["type": .string("body"), "name": .string(name)])])]
        case .feature(let id): return ["items": .array([.object(["type": .string("feature"), "id": .unsignedInteger(id)])])]
        case .occurrence(let path): return ["items": .array([.object(["type": .string("occurrence"), "path": .array(path.map(PresentationJSON.unsignedInteger))])])]
        case .component(let id): return ["components": .array([.unsignedInteger(id)])]
        case .joint(let id): return ["joints": .array([.unsignedInteger(id)])]
        case .folder(let id): return ["group": .unsignedInteger(id)]
        }
    }
}
