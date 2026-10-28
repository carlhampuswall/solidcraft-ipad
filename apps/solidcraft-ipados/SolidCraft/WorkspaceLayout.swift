import CoreGraphics
import Foundation

/// Native chrome geometry in safe-area display points. The actual Metal view is
/// assigned `canvas`, so rendering and picking always share the same local bounds.
struct WorkspaceLayout {
    let topBar: CGRect
    let status: CGRect
    let rail: CGRect
    let canvas: CGRect
    let browser: CGRect?
    let inspector: CGRect?
    let compact: Bool

    static func resolve(size: CGSize, browser: Bool, inspector: Bool,
                        railRight: Bool, largeText: Bool,
                        minimumHeaderHeight: CGFloat = 0, minimumStatusHeight: CGFloat = 0) -> WorkspaceLayout {
        let width = size.width.isFinite ? max(0, size.width) : 0
        let height = size.height.isFinite ? max(0, size.height) : 0
        let compact = width < (largeText ? 1120 : 1000)
        let headerHeight: CGFloat = min(height, max(minimumHeaderHeight, compact ? (largeText ? 128 : 104) : (largeText ? 72 : 56)))
        let statusHeight: CGFloat = min(max(0, height - headerHeight), max(minimumStatusHeight, largeText ? 64 : 44))
        let gap: CGFloat = min(8, width / 8)
        let contentY = headerHeight + min(gap, max(0, height - headerHeight - statusHeight))
        let contentHeight = max(0, height - statusHeight - contentY)
        // Text-labelled tools need room for whole words, including Assemble and
        // Create sketch. Keep the actual canvas/picking frame beside this rail.
        let railWidth: CGFloat = min(largeText ? 160 : 112, max(0, width - 2 * gap))
        let railX = railRight ? max(gap, width - gap - railWidth) : gap
        let railFrame = CGRect(x: railX, y: contentY, width: railWidth, height: contentHeight)
        var left = railRight ? gap : min(width, railFrame.maxX + gap)
        var right = railRight ? max(left, railFrame.minX - gap) : max(left, width - gap)
        var browserFrame: CGRect?
        var inspectorFrame: CGRect?

        if compact {
            // Secondary content is a single modal panel over the work area. UIKit
            // owns its scrim; controller disables canvas input until dismissal.
            let panelWidth = max(0, min(largeText ? 420 : 360, width - 2 * gap))
            let panelFrame = CGRect(x: max(0, width - gap - panelWidth), y: contentY,
                                    width: panelWidth, height: contentHeight)
            if inspector { inspectorFrame = panelFrame }
            else if browser { browserFrame = panelFrame }
        } else {
            // Reserve the existing usable canvas before docking panels. Near a
            // breakpoint the panels share the small deficit instead of squeezing
            // tool labels or reducing the viewport below its minimum.
            let browserWidth: CGFloat = largeText ? 280 : 248
            let inspectorWidth: CGFloat = largeText ? 320 : 280
            let panelCount: CGFloat = (browser ? 1 : 0) + (inspector ? 1 : 0)
            let requested = (browser ? browserWidth : 0) + (inspector ? inspectorWidth : 0)
            let budget = max(0, right - left - 360 - panelCount * gap)
            let reduction = panelCount > 0 ? max(0, requested - budget) / panelCount : 0
            if browser {
                let panelWidth = max(0, browserWidth - reduction)
                browserFrame = CGRect(x: left, y: contentY, width: panelWidth, height: contentHeight)
                left += panelWidth + gap
            }
            if inspector {
                let panelWidth = max(0, inspectorWidth - reduction)
                inspectorFrame = CGRect(x: right - panelWidth, y: contentY, width: panelWidth, height: contentHeight)
                right -= panelWidth + gap
            }
        }
        return WorkspaceLayout(
            topBar: CGRect(x: 0, y: 0, width: width, height: headerHeight),
            status: CGRect(x: gap, y: height - statusHeight, width: max(0, width - 2 * gap), height: statusHeight),
            rail: railFrame,
            canvas: CGRect(x: left, y: contentY, width: max(0, right - left), height: contentHeight),
            browser: browserFrame, inspector: inspectorFrame, compact: compact
        )
    }
}

/// Native text fields keep their own text undo manager. Document commands are
/// intentionally unavailable while a field or a provisional numeric edit owns focus.
enum WorkspaceFocusPolicy {
    static func documentShortcutsEnabled(textEntryActive: Bool, numericEditActive: Bool) -> Bool {
        !textEntryActive && !numericEditActive
    }

    static func viewportInputEnabled(modalPresented: Bool, textEntryActive: Bool) -> Bool {
        !modalPresented && !textEntryActive
    }
}

/// The command ABI wraps native command values separately from viewport metadata.
/// Kept Foundation-only so deterministic host checks exercise the production decoder.
struct EngineCommandEnvelope<Value: Decodable>: Decodable {
    struct Body: Decodable {
        struct Command: Decodable { let value: Value }
        let command: Command
    }
    let result: Body
}

/// A cancelled or replaced tool job is a successful transport response, but it
/// cannot be presented as an applied modelling operation.
struct SketchToolWorkEnvelope: Decodable {
    struct Work: Decodable {
        let worked: Bool?
        let committed: Bool?
        let discarded: Bool?
        func accepts(commit: Bool) -> Bool { worked == true && discarded != true && committed == commit }
    }
    let result: Work
}

/// Lossless JSON transport for engine-authored command arguments, never CAD logic.
enum PresentationJSON: Decodable, Equatable {
    case string(String), integer(Int64), unsignedInteger(UInt64), number(Double), bool(Bool), array([PresentationJSON]), object([String: PresentationJSON]), null
    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() { self = .null }
        else if let value = try? container.decode(Bool.self) { self = .bool(value) }
        else if let value = try? container.decode(String.self) { self = .string(value) }
        else if let value = try? container.decode(Int64.self) { self = .integer(value) }
        else if let value = try? container.decode(UInt64.self) { self = .unsignedInteger(value) }
        else if let value = try? container.decode(Double.self) { self = .number(value) }
        else if let value = try? container.decode([PresentationJSON].self) { self = .array(value) }
        else { self = .object(try container.decode([String: PresentationJSON].self)) }
    }
    var value: Any {
        switch self {
        case .string(let value): return value
        case .integer(let value): return value
        case .unsignedInteger(let value): return value
        case .number(let value): return value
        case .bool(let value): return value
        case .array(let value): return value.map(\.value)
        case .object(let value): return value.mapValues(\.value)
        case .null: return NSNull()
        }
    }

    static func parameter(_ path: String, in params: [String: PresentationJSON]) -> PresentationJSON? {
        var current = PresentationJSON.object(params)
        for key in path.split(separator: ".").map(String.init) {
            guard case .object(let object) = current, let value = object[key] else { return nil }
            current = value
        }
        return current
    }

    static func setting(_ path: String, value: PresentationJSON, in params: [String: PresentationJSON]) -> [String: PresentationJSON] {
        func replacing(_ object: [String: PresentationJSON], keys: ArraySlice<String>) -> [String: PresentationJSON] {
            guard let key = keys.first else { return object }
            var next = object
            if keys.count == 1 { next[key] = value }
            else {
                let nested: [String: PresentationJSON]
                if case .object(let existing) = next[key] { nested = existing } else { nested = [:] }
                next[key] = .object(replacing(nested, keys: keys.dropFirst()))
            }
            return next
        }
        return replacing(params, keys: ArraySlice(path.split(separator: ".").map(String.init)))
    }
}


/// Engine-authored presentation only. Native forms retain opaque references and
/// raw expressions; Rust owns eligibility, evaluation, history and persistence.
struct ModellingPresentation: Decodable {
    struct Tool: Decodable { let id: String; let label: String }
    struct Feature: Decodable {
        let id: UInt64
        let name: String
        let kind: String
        let suppressed: Bool
        let error: String?
        let warning: String?
        let stale: Bool?
        let order: Int
    }
    struct Parameter: Decodable { let name: String; let expression: String; let unit: String; let value: Double? }
    let tools: [Tool]
    let timeline: [Feature]
    let parameters: [Parameter]
}

struct ModellingResponse<Workspace: Decodable>: Decodable {
    struct Field: Decodable {
        struct Option: Decodable { let label: String; let value: PresentationJSON }
        let key: String
        let label: String
        let kind: String
        let options: [Option]?
        let units: String?
        let min: Double?
        let max: Double?
        let value: Double?
    }
    let token: UInt64?
    let operation: String?
    let feature: UInt64?
    let params: [String: PresentationJSON]?
    let fields: [Field]?
    let valid: Bool?
    let feedback: [String]?
    let reply: PresentationJSON?
    let workspace: Workspace
}

/// Assembly controls retain native occurrence paths and command payloads. They
/// are presentation projections, never a second assembly or constraint graph.
struct AssemblyActionPresentation: Decodable {
    let label: String
    let command: String?
    let operation: String?
    let params: [String: PresentationJSON]
    let confirm: String?
}

struct AssemblyRowPresentation: Decodable {
    let component: UInt64?
    let path: [UInt64]?
    let joint: UInt64?
    let joint_type: String?
    let suppressed: Bool?
    let active: Bool?
    let grounded: Bool?
    let visible: Bool?
    let pending: Bool?
    let diagnostic: String?
    let actions: [AssemblyActionPresentation]
}

struct AssemblyPresentation: Decodable {
    let active_component: UInt64
    let active_name: String
    let active_path: [UInt64]
    let tools: [ModellingPresentation.Tool]
    let diagnostics: [String]
    let actions: [AssemblyActionPresentation]
}

/// Only one native slider proposal enters the serial engine lane. While it is
/// evaluating, intermediate input replaces one pending proposal; identical latest
/// input reuses the active request. No geometry or document is stored here.
struct ModellingPreviewQueue {
    private(set) var active: [String: PresentationJSON]?
    private var latest: [String: PresentationJSON]?
    mutating func offer(_ params: [String: PresentationJSON]) -> Bool {
        guard active != nil else { active = params; return true }
        latest = params == active ? nil : params
        return false
    }
    mutating func complete() -> [String: PresentationJSON]? {
        active = nil
        let next = latest; latest = nil
        return next
    }
    mutating func discardPending() { latest = nil }
    mutating func clear() { active = nil; latest = nil }
}

/// Command descriptors carry dotted registry IDs; canonical command labels use
/// underscores. Older modelling labels retain their existing UI catalogue IDs.
enum PresentationLabelKey {
    private static let legacy: [String: String] = [
        "solid_chamfer": "ui.chamfer", "solid_combine": "ui.combine", "solid_extrude": "ui.extrude",
        "solid_fillet": "ui.fillet", "solid_hole": "ui.hole", "solid_loft": "ui.loft", "solid_mirror": "ui.mirror",
        "solid_pattern_circular": "ui.circular_pattern", "solid_pattern_path": "ui.pattern_on_path",
        "solid_pattern_rectangular": "ui.rectangular_pattern", "solid_revolve": "ui.revolve",
        "solid_shell": "ui.shell", "solid_sweep": "ui.sweep", "timeline_edit": "cmd.timeline_redefine"
    ]
    static func label(_ value: String, lookup: (String) -> String) -> String {
        let key: String
        if value.hasPrefix("cmd.") {
            let suffix = value.dropFirst(4).replacingOccurrences(of: ".", with: "_")
            key = legacy[suffix] ?? "cmd." + suffix
        } else if value.hasPrefix("ui.") { key = value }
        else { return value }
        let translated = lookup(key)
        return translated == key ? value : translated
    }
}
