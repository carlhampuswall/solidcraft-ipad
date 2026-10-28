import CoreGraphics
import Foundation

/// Engine presentation only; the host never evaluates this expression.
struct ModelValuePresentation: Decodable {
    let token: UInt64
    let parameter: String
    let position: [Double]
    let value: Double
    let units: String
    let expression: String
    let valid: Bool
}

enum ModelValuePlacement {
    static func resolve(anchor: CGPoint, size: CGSize, bounds: CGRect, obstacles: [CGRect]) -> CGRect? {
        guard anchor.x.isFinite, anchor.y.isFinite, size.width.isFinite, size.height.isFinite,
              bounds.minX.isFinite, bounds.minY.isFinite, bounds.maxX.isFinite, bounds.maxY.isFinite,
              size.width > 0, size.height > 0, bounds.width >= size.width, bounds.height >= size.height else { return nil }
        let grip = CGRect(x: anchor.x - 26, y: anchor.y - 26, width: 52, height: 52)
        let candidates = [CGPoint(x: anchor.x + 34, y: anchor.y - size.height / 2),
                          CGPoint(x: anchor.x - size.width / 2, y: anchor.y - 34 - size.height),
                          CGPoint(x: anchor.x - 34 - size.width, y: anchor.y - size.height / 2),
                          CGPoint(x: anchor.x - size.width / 2, y: anchor.y + 34)]
        return candidates.map { origin in
            CGRect(x: min(max(origin.x, bounds.minX), bounds.maxX - size.width),
                   y: min(max(origin.y, bounds.minY), bounds.maxY - size.height), width: size.width, height: size.height)
        }.first { frame in !frame.intersects(grip) && !obstacles.contains(where: { $0.intersects(frame) }) }
    }
}

/// Native chrome geometry in safe-area display points, placed into window space
/// before assigning the actual Metal view. Rendering and picking share its bounds.
struct WorkspaceLayout {
    let topBar: CGRect
    let status: CGRect
    let rail: CGRect
    let canvas: CGRect
    let browser: CGRect?
    let inspector: CGRect?
    let compact: Bool
    var navigation: CGRect = .zero
    var history: CGRect = .zero
    var browserAccess: CGRect = .zero
    var floating = false

    static func resolve(size: CGSize, browser: Bool, inspector: Bool,
                        railRight: Bool, largeText: Bool,
                        minimumHeaderHeight: CGFloat = 0, minimumStatusHeight: CGFloat = 0,
                        compactChrome: Bool = true, pinnedBrowser: Bool = false, railContentHeight: CGFloat = 460,
                        inspectorContentHeight: CGFloat = 640, navigationContentHeight: CGFloat = 192) -> WorkspaceLayout {
        if compactChrome {
            return floatingLayout(size: size, browser: browser, inspector: inspector, railRight: railRight,
                                  largeText: largeText, header: minimumHeaderHeight, status: minimumStatusHeight,
                                  pinnedBrowser: pinnedBrowser, railContentHeight: railContentHeight, inspectorContentHeight: inspectorContentHeight,
                                  navigationContentHeight: navigationContentHeight)
        }
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

extension WorkspaceLayout {
    /// Keep controls safe while the floating viewport extends underneath system
    /// indicators. A pinned browser still reserves only its horizontal region.
    func placed(in chrome: CGRect, viewport: CGRect) -> WorkspaceLayout {
        func offset(_ frame: CGRect) -> CGRect { frame.offsetBy(dx: chrome.minX, dy: chrome.minY) }
        let insetCanvas = offset(canvas)
        let left = floating && canvas.minX <= 0 ? viewport.minX : insetCanvas.minX
        let right = floating && canvas.maxX >= chrome.width ? viewport.maxX : insetCanvas.maxX
        let modelFrame = floating ? CGRect(x: left, y: viewport.minY, width: max(0, right - left), height: viewport.height) : insetCanvas
        var result = WorkspaceLayout(topBar: offset(topBar), status: offset(status), rail: offset(rail),
            canvas: modelFrame, browser: browser.map(offset), inspector: inspector.map(offset), compact: compact)
        result.navigation = offset(navigation)
        result.history = offset(history)
        result.browserAccess = offset(browserAccess)
        result.floating = floating
        return result
    }

    /// Floating controls consume only their visible bounds. Unpinned panels do
    /// not move the Metal view or change its rendering/picking coordinate space.
    private static func floatingLayout(size: CGSize, browser: Bool, inspector: Bool,
                                       railRight: Bool, largeText: Bool, header: CGFloat, status: CGFloat,
                                       pinnedBrowser: Bool, railContentHeight: CGFloat, inspectorContentHeight: CGFloat,
                                       navigationContentHeight: CGFloat) -> WorkspaceLayout {
        let w = size.width.isFinite ? max(0, size.width) : 0
        let h = size.height.isFinite ? max(0, size.height) : 0
        let gap = min(8, w / 8, h / 8)
        let railWidth = min(largeText ? 56 : 52, max(0, w - 2 * gap))
        let headerHeight = min(max(0, h - 2 * gap), max(largeText ? 76 : 52, header))
        let headerWidth = min(largeText ? 420 : 340, max(0, w - 2 * (railWidth + 2 * gap)))
        let top = CGRect(x: (w - headerWidth) / 2, y: gap, width: headerWidth, height: headerHeight)
        let bottomHeight = min(max(0, h - top.maxY - 2 * gap), max(48, status))
        let controlsY = min(h, top.maxY + gap)
        let controlsHeight = max(0, h - bottomHeight - 2 * gap - controlsY)
        let railX = railRight ? max(gap, w - gap - railWidth) : gap
        let navX = railRight ? gap : max(gap, w - gap - railWidth)
        let rail = CGRect(x: railX, y: controlsY, width: railWidth,
                          height: min(controlsHeight, max(0, railContentHeight)))
        let nav = CGRect(x: navX, y: controlsY, width: railWidth, height: min(controlsHeight, max(0, navigationContentHeight)))
        var canvas = CGRect(x: 0, y: 0, width: w, height: h)
        var browserFrame: CGRect?
        var inspectorFrame: CGRect?
        let panelHeight = max(0, min(largeText ? 760 : 640, h - controlsY - bottomHeight - gap))
        let panelWidth = min(largeText ? 380 : 320, max(0, w - 2 * (railWidth + 2 * gap)))
        let dock = pinnedBrowser && browser && w >= (largeText ? 1180 : 1000)
        if browser {
            let x = railRight ? max(0, w - 2 * gap - railWidth - panelWidth) : min(w, railWidth + 2 * gap)
            browserFrame = CGRect(x: x, y: controlsY, width: panelWidth, height: panelHeight)
            if dock {
                let inset = panelWidth + railWidth + 3 * gap
                canvas = CGRect(x: railRight ? 0 : inset, y: 0, width: max(0, w - inset), height: h)
            }
        }
        if inspector {
            let x = railRight ? min(w, railWidth + 2 * gap) : max(0, w - 2 * gap - railWidth - panelWidth)
            inspectorFrame = CGRect(x: x, y: controlsY, width: panelWidth,
                                    height: min(panelHeight, max(180, inspectorContentHeight)))
        }
        let historyWidth = min(100, max(0, w - 2 * gap))
        let statusX = min(w, historyWidth + 2 * gap)
        let statusWidth = max(0, min(560, w - statusX - railWidth - 2 * gap))
        var result = WorkspaceLayout(topBar: top,
            status: CGRect(x: statusX, y: max(0, h - bottomHeight - gap), width: statusWidth, height: bottomHeight),
            rail: rail, canvas: canvas, browser: browserFrame, inspector: inspectorFrame, compact: false)
        result.navigation = nav
        result.history = CGRect(x: gap, y: max(0, h - bottomHeight - gap), width: historyWidth, height: bottomHeight)
        result.browserAccess = CGRect(x: railX, y: gap, width: railWidth, height: min(headerHeight, 52))
        result.floating = true
        return result
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
    struct Handle: Decodable { let distance: Double; let units: String }
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
    let handle: Handle?
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

/// UIKit captures source identity and local coordinates. Rust owns gesture policy and camera math.
struct InputSample: Encodable {
    enum Source: String, Encodable { case pencil, finger, indirect, unknown }
    enum Phase: String, Encodable { case began, moved, ended, cancelled, hover, scroll }
    let source: Source
    let phase: Phase
    let id: UInt64
    let x_points: Double
    let y_points: Double
    let x_pixels: Double
    let y_pixels: Double
    let scale: Double
    let timestamp: Double
    let pressure: Double?
    let altitude: Double?
    let geometry_revision: UInt64?
    let consumed: Bool
    let shift: Bool
    let button: UInt8
    let scroll: Double
    var predicted = false
    var display_preview = false
    var sequence: UInt64 = 0
    var viewport_epoch: UInt64? = nil
    var camera_epoch: UInt64? = nil
    var document_id: String? = nil
    var azimuth: Double? = nil
}

/// No UIKit objects cross a queue. Under load, thin redundant actual Pencil
/// motion along the same screen path, retaining contact boundaries and turns.
/// If no redundant motion remains, discard the uncertain interaction as a whole.
final class InputMailbox {
    private let lock = NSLock()
    private var samples: [InputSample] = []
    private var scheduled = false
    private var cancelled = false
    private var epoch: UInt64 = 0
    func append(_ sample: InputSample) -> UInt64? {
        lock.lock(); defer { lock.unlock() }
        if samples.count >= 128 {
            if let index = (1..<(samples.count - 1)).first(where: { redundantMotion(at: $0) }) {
                samples.remove(at: index)
            } else {
                samples.removeAll(keepingCapacity: true); cancelled = true
            }
        }
        samples.append(sample)
        if scheduled { return nil }
        scheduled = true
        return epoch
    }
    private func redundantMotion(at index: Int) -> Bool {
        let before = samples[index - 1], current = samples[index], after = samples[index + 1]
        guard current.phase == .moved, current.source == .pencil,
              before.phase == .began || before.phase == .moved, after.phase == .moved,
              [before, after].allSatisfy({
                  $0.source == current.source && $0.id == current.id && !$0.predicted && !current.predicted
                      && $0.document_id == current.document_id && $0.geometry_revision == current.geometry_revision
                      && $0.viewport_epoch == current.viewport_epoch && $0.camera_epoch == current.camera_epoch
                      && $0.scale == current.scale && $0.display_preview == current.display_preview
                      && $0.consumed == current.consumed && $0.shift == current.shift && $0.button == current.button
              }), before.timestamp <= current.timestamp, current.timestamp <= after.timestamp else { return false }
        let ax = current.x_points - before.x_points, ay = current.y_points - before.y_points
        let bx = after.x_points - current.x_points, by = after.y_points - current.y_points
        let cross = ax * by - ay * bx
        // Quarter-point screen tolerance; reversals/curves keep their samples.
        return ax * bx + ay * by >= 0 && cross * cross <= 0.0625 * ((ax + bx) * (ax + bx) + (ay + by) * (ay + by))
    }
    func take(token: UInt64) -> (samples: [InputSample], cancelled: Bool)? {
        lock.lock(); defer { lock.unlock() }
        guard token == epoch else { return nil }
        let result = (samples, cancelled)
        samples.removeAll(keepingCapacity: true); cancelled = false; scheduled = false
        return result
    }
    func clear() {
        lock.lock(); defer { lock.unlock() }
        samples.removeAll(keepingCapacity: true); cancelled = true
        epoch &+= 1; scheduled = false
    }
}
