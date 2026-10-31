import CoreGraphics
import Foundation

/// Native drag presentation preference; exact expressions bypass this mapping.
enum DragResolution {
    static let key = "SolidCraft.workspace.dragStepMM"
    static let options: [Double] = [0.01, 0.1, 0.5, 1, 0]
    static func restored(_ value: Double?) -> Double {
        guard let value, options.contains(value) else { return 0.1 }
        return value
    }
    static var current: Double { restored(UserDefaults.standard.object(forKey: key) as? Double) }
    static func snapped(_ value: Double, units: String, step: Double) -> Double {
        guard units == "mm", value.isFinite, step.isFinite, step > 0 else { return value }
        let scale = 1 / step
        let result = (value * scale).rounded() / scale
        return result.isFinite ? result : value
    }
}

/// Presentation order and artwork only; the descriptor owns every field,
/// option value and CAD rule. Stable sorting retains unrelated field order.
enum InspectorPresentation {
    static func isDistance(_ key: String) -> Bool { key == "distance" || key == "extent.distance" }
    static func isPrimary<Workspace>(_ field: ModellingResponse<Workspace>.Field) -> Bool {
        field.key == "profile" || isDistance(field.key) || field.kind == "inline_choice"
    }
    static func orderedFields<Workspace>(_ fields: [ModellingResponse<Workspace>.Field], compact: Bool) -> [ModellingResponse<Workspace>.Field] {
        guard compact else { return fields.filter { $0.kind == "inline_choice" } + fields.filter { $0.kind != "inline_choice" } }
        guard fields.contains(where: { $0.key == "operation" && $0.kind == "inline_choice" }) else { return fields }
        func rank(_ field: ModellingResponse<Workspace>.Field) -> Int {
            if field.key == "profile" { return 0 }
            if isDistance(field.key) { return 1 }
            if field.kind == "inline_choice" { return 2 }
            return 3
        }
        return fields.enumerated().sorted {
            let a = rank($0.element), b = rank($1.element)
            return a == b ? $0.offset < $1.offset : a < b
        }.map(\.element)
    }
    static func choiceIcon(field: String, value: PresentationJSON) -> String? {
        guard field == "operation", case .string(let id) = value else { return nil }
        switch id {
        case "new", "new_body": return "body"
        case "join": return "combine"
        case "intersect": return "intersect"
        case "cut": return "cut"
        default: return nil
        }
    }
}

/// Validation belongs to an exact draft revision, including an edit away and
/// back to identical text. This is presentation state, never an evaluator.
struct NumericDraftValidation {
    private(set) var revision: UInt64 = 0
    private var validatedRevision: UInt64?
    var isValidated: Bool { validatedRevision == revision }
    mutating func changed() { revision &+= 1; validatedRevision = nil }
    @discardableResult mutating func complete(revision: UInt64, valid: Bool) -> Bool {
        guard revision == self.revision else { return false }
        validatedRevision = valid ? revision : nil
        return true
    }
}

/// A saved explicit filter retains face-only picking; new workspaces accept
/// the closest visible topology without requiring a trip through Tools.
enum WorkspaceSelectionFilter: String, CaseIterable {
    case occurrence, auto, body, face, edge, vertex

    init(storedValue: String?) {
        self = storedValue.flatMap(Self.init(rawValue:)) ?? .auto
    }
}

/// Consume every selection report, including those received while another tool
/// owns input. Only a newly selected face in the same model can open Extrude.
struct FaceExtrudeTrigger<Selection: Equatable> {
    private var document: String?
    private var revision: UInt64?
    private var selection: [Selection] = []

    static func shouldDismiss(automatic: Bool, committing: Bool, selection: [Selection],
                              isFace: (Selection) -> Bool) -> Bool {
        automatic && !committing && !selection.contains(where: isFace)
    }

    mutating func update(document: String, revision: UInt64, selection: [Selection],
                         enabled: Bool, idle: Bool, isFace: (Selection) -> Bool) -> Bool {
        defer { self.document = document; self.revision = revision; self.selection = selection }
        return enabled && idle && self.document == document && self.revision == revision &&
            !selection.isEmpty && selection.allSatisfy(isFace) &&
            selection.contains { !self.selection.contains($0) }
    }
}

/// Observe taps without consuming the canvas contact stream used to drag
/// dimension text. A second contact, movement or stale display cancels entry.
struct DimensionTapTracker {
    /// Free-size captions can be wider than their distance from an edge. Keep
    /// their touch target centred and predictable so long values do not steal
    /// geometry contacts. Existing dimension captions retain their full target.
    static func hitFrame(_ frame: CGRect, helper: Bool) -> CGRect {
        helper ? CGRect(x: frame.midX - 22, y: frame.midY - 22, width: 44, height: 44) : frame
    }

    struct Context: Equatable {
        let document: String
        let geometry: UInt64
        let viewport: UInt64
        let camera: UInt64
    }
    private var contacts: Set<UInt64> = []
    private var candidate: (id: UInt64, target: String, point: CGPoint, context: Context)?
    mutating func begin(id: UInt64, point: CGPoint, target: String?, context: Context) {
        contacts.insert(id)
        guard contacts.count == 1, let target else { candidate = nil; return }
        candidate = (id, target, point, context)
    }
    mutating func move(id: UInt64, point: CGPoint) {
        guard let candidate, candidate.id == id else { return }
        // Match Rust's contact movement threshold so a completed drag cannot
        // also open the value editor on lift.
        if !point.x.isFinite || !point.y.isFinite || hypot(point.x - candidate.point.x, point.y - candidate.point.y) > 5 {
            self.candidate = nil
        }
    }
    mutating func end(id: UInt64, point: CGPoint, context: Context, cancelled: Bool) -> String? {
        move(id: id, point: point)
        let result = candidate.flatMap { $0.id == id && $0.context == context && !cancelled ? $0.target : nil }
        contacts.remove(id)
        if candidate?.id == id { candidate = nil }
        return result
    }
    mutating func cancel() { contacts.removeAll(); candidate = nil }
}

/// Marker dragging is presentation state. At most one existing timeline command
/// runs at once; subsequent samples retain only the latest requested boundary.
struct HistoryScrubQueue {
    struct Request: Equatable { let token: UInt64; let position: Int }
    private var token: UInt64 = 0
    private var applied: Int?
    private(set) var dragging = false
    private(set) var target: Int?
    private(set) var inFlight: Request?
    var active: Bool { target != nil || inFlight != nil || dragging }

    mutating func begin(at position: Int) {
        cancel(); dragging = true; target = position; applied = position
    }
    mutating func move(to position: Int) { if dragging { target = max(0, position) } }
    mutating func end() { dragging = false }
    mutating func next() -> Request? {
        guard inFlight == nil, let target, target != applied else { return nil }
        token &+= 1
        let request = Request(token: token, position: target)
        inFlight = request
        return request
    }
    @discardableResult mutating func complete(_ request: Request, succeeded: Bool) -> Bool {
        guard inFlight == request else { return false }
        if succeeded { inFlight = nil; applied = request.position }
        else { cancel() }
        return true
    }
    mutating func acknowledge(_ position: Int) {
        if !dragging && inFlight == nil && target == position && applied == position { target = nil; applied = nil }
    }
    mutating func cancel() {
        token &+= 1; dragging = false; target = nil; inFlight = nil; applied = nil
    }
}

enum HistoryScrubGeometry {
    /// Icon centres are the halfway thresholds between adjacent history gaps.
    /// Coordinates are in scroll content points, including any scroll offset.
    static func position(at x: CGFloat, centres: [CGFloat]) -> Int? {
        guard x.isFinite, !centres.isEmpty, centres.allSatisfy(\.isFinite),
              zip(centres, centres.dropFirst()).allSatisfy({ $0 < $1 }) else { return nil }
        return centres.firstIndex(where: { x < $0 }) ?? centres.count
    }
}

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

/// Native chrome geometry in available display points, placed into window space
/// before assigning the actual Metal view. Rendering and picking share its bounds.
struct WorkspaceLayout {
    let topBar: CGRect
    var status: CGRect
    let rail: CGRect
    var canvas: CGRect
    var browser: CGRect?
    let inspector: CGRect?
    let compact: Bool
    var navigation: CGRect = .zero
    var history: CGRect = .zero
    var featureHistory: CGRect = .zero
    var browserAccess: CGRect = .zero
    var viewAccess: CGRect = .zero
    var floating = false

    static func resolve(size: CGSize, browser: Bool, inspector: Bool,
                        railRight: Bool, largeText: Bool,
                        minimumHeaderHeight: CGFloat = 0, minimumStatusHeight: CGFloat = 0,
                        compactChrome: Bool = true, pinnedBrowser: Bool = false, railContentHeight: CGFloat = 460,
                        inspectorContentHeight: CGFloat = 640, navigationContentHeight: CGFloat = 192,
                        leftBrowser: Bool = true, historyExpanded: Bool = false, inspectorPreferredWidth: CGFloat = 320,
                        historyToggleWidth: CGFloat = 112, viewButtonSize: CGSize = CGSize(width: 96, height: 52)) -> WorkspaceLayout {
        let width = size.width.isFinite ? max(0, size.width) : 0
        let height = size.height.isFinite ? max(0, size.height) : 0
        let gap = min(8, width / 8, height / 8)
        let barHeight = min(max(0, height - 2 * gap), largeText ? 76 : 52)
        let reserved = historyExpanded ? min(height, barHeight + gap) : 0
        var result = contentLayout(size: CGSize(width: width, height: max(0, height - reserved)),
            browser: browser, inspector: inspector, railRight: railRight, largeText: largeText,
            minimumHeaderHeight: minimumHeaderHeight,
            minimumStatusHeight: historyExpanded ? minimumStatusHeight : max(minimumStatusHeight, barHeight + gap),
            compactChrome: compactChrome, pinnedBrowser: pinnedBrowser, railContentHeight: railContentHeight,
            inspectorContentHeight: inspectorContentHeight, navigationContentHeight: navigationContentHeight, leftBrowser: leftBrowser, inspectorPreferredWidth: inspectorPreferredWidth, viewButtonSize: viewButtonSize)
        let toggleWidth = historyToggleWidth.isFinite ? max(52, historyToggleWidth) : 112
        let barWidth = historyExpanded ? max(0, width - 2 * gap) : min(toggleWidth, max(0, width - 2 * gap))
        result.featureHistory = CGRect(x: historyExpanded ? gap : max(gap, width - gap - barWidth),
            y: max(gap, height - gap - barHeight), width: barWidth, height: barHeight)
        if !historyExpanded {
            if result.floating {
                // The bottom row belongs to the workspace, including when a
                // left browser leaves only a narrow strip for the side rails.
                result.history = CGRect(x: gap, y: result.featureHistory.minY,
                    width: min(100, max(0, result.featureHistory.minX - 2 * gap)), height: barHeight)
                result.status.origin = CGPoint(x: min(width, result.history.maxX + gap), y: result.featureHistory.minY)
                result.status.size.height = barHeight
                if var panel = result.browser {
                    panel.size.height = min(panel.height, max(0, result.featureHistory.minY - gap - panel.minY))
                    result.browser = panel
                }
            }
            result.status.size.width = min(result.status.width, max(0, result.featureHistory.minX - gap - result.status.minX))
        }
        if result.floating { result.canvas.size.height = height }
        return result
    }

    private static func contentLayout(size: CGSize, browser: Bool, inspector: Bool,
                        railRight: Bool, largeText: Bool,
                        minimumHeaderHeight: CGFloat, minimumStatusHeight: CGFloat,
                        compactChrome: Bool, pinnedBrowser: Bool, railContentHeight: CGFloat,
                        inspectorContentHeight: CGFloat, navigationContentHeight: CGFloat,
                        leftBrowser: Bool, inspectorPreferredWidth: CGFloat, viewButtonSize: CGSize) -> WorkspaceLayout {
        if compactChrome {
            return floatingLayout(size: size, browser: browser, inspector: inspector, railRight: railRight,
                                  largeText: largeText, header: minimumHeaderHeight, status: minimumStatusHeight,
                                  pinnedBrowser: pinnedBrowser, railContentHeight: railContentHeight, inspectorContentHeight: inspectorContentHeight,
                                  navigationContentHeight: navigationContentHeight, leftBrowser: leftBrowser, inspectorPreferredWidth: inspectorPreferredWidth, viewButtonSize: viewButtonSize)
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
    /// Preserve top/side system insets while allowing controls to use the bottom
    /// safe area. The keyboard remains the lower boundary in either mode.
    static func chromeBounds(safeArea: CGRect, viewport: CGRect, useBottomSafeArea: Bool = true) -> CGRect {
        let bottom = useBottomSafeArea ? viewport.maxY : min(safeArea.maxY, viewport.maxY)
        return CGRect(x: safeArea.minX, y: safeArea.minY, width: safeArea.width,
                      height: max(0, bottom - safeArea.minY))
    }

    /// Place controls in their available bounds while the floating viewport
    /// extends underneath indicators. A pinned browser reserves only its width.
    func placed(in chrome: CGRect, viewport: CGRect, useTopSafeArea: Bool = false) -> WorkspaceLayout {
        func offset(_ frame: CGRect) -> CGRect { frame.offsetBy(dx: chrome.minX, dy: chrome.minY) }
        var header = offset(topBar)
        let topInset = chrome.minY - viewport.minY
        // Leave room at both ends of iPad's status strip for system indicators.
        // Crowded windows and enlarged headers retain their safe-area position.
        let indicatorMargin: CGFloat = 240
        if floating && useTopSafeArea && (20...64).contains(topInset) && header.height <= 64 &&
            header.minX >= viewport.minX + indicatorMargin && header.maxX <= viewport.maxX - indicatorMargin {
            let candidate = CGRect(x: header.minX, y: viewport.minY + 4, width: header.width, height: header.height)
            let obstacles = [rail, navigation, browserAccess, viewAccess] + [browser, inspector].compactMap { $0 }
            if viewport.contains(candidate) && !obstacles.contains(where: { offset($0).intersects(candidate) }) {
                header = candidate
            }
        }
        let insetCanvas = offset(canvas)
        let left = floating && canvas.minX <= 0 ? viewport.minX : insetCanvas.minX
        let right = floating && canvas.maxX >= chrome.width ? viewport.maxX : insetCanvas.maxX
        let modelFrame = floating ? CGRect(x: left, y: viewport.minY, width: max(0, right - left), height: viewport.height) : insetCanvas
        var result = WorkspaceLayout(topBar: header, status: offset(status), rail: offset(rail),
            canvas: modelFrame, browser: browser.map(offset), inspector: inspector.map(offset), compact: compact)
        result.navigation = offset(navigation)
        result.history = offset(history)
        result.featureHistory = offset(featureHistory)
        result.browserAccess = offset(browserAccess)
        result.viewAccess = offset(viewAccess)
        result.floating = floating
        return result
    }

    /// Floating controls consume only their visible bounds. Unpinned panels do
    /// not move the Metal view or change its rendering/picking coordinate space.
    private static func floatingLayout(size: CGSize, browser: Bool, inspector: Bool,
                                       railRight: Bool, largeText: Bool, header: CGFloat, status: CGFloat,
                                       pinnedBrowser: Bool, railContentHeight: CGFloat, inspectorContentHeight: CGFloat,
                                       navigationContentHeight: CGFloat, leftBrowser: Bool, inspectorPreferredWidth: CGFloat, viewButtonSize: CGSize,
                                       viewSharesBrowserColumn: Bool = false) -> WorkspaceLayout {
        let w = size.width.isFinite ? max(0, size.width) : 0
        let h = size.height.isFinite ? max(0, size.height) : 0
        let gap = min(8, w / 8, h / 8)
        let railWidth = min(largeText ? 56 : 52, max(0, w - 2 * gap))
        if leftBrowser && browser {
            let panelWidth = min(largeText ? 380 : 320, max(0, w - railWidth - 3 * gap))
            let inset = min(w, panelWidth + gap)
            let remaining = floatingLayout(size: CGSize(width: max(0, w - inset), height: h), browser: false, inspector: inspector,
                railRight: railRight, largeText: largeText, header: header, status: status, pinnedBrowser: false,
                railContentHeight: railContentHeight, inspectorContentHeight: inspectorContentHeight,
                navigationContentHeight: navigationContentHeight, leftBrowser: false, inspectorPreferredWidth: inspectorPreferredWidth, viewButtonSize: viewButtonSize, viewSharesBrowserColumn: true)
            func shifted(_ frame: CGRect) -> CGRect { frame.offsetBy(dx: inset, dy: 0) }
            let dock = pinnedBrowser && w >= (largeText ? 1180 : 1000)
            var result = WorkspaceLayout(topBar: shifted(remaining.topBar), status: shifted(remaining.status), rail: shifted(remaining.rail),
                canvas: CGRect(x: dock ? inset : 0, y: 0, width: max(0, w - (dock ? inset : 0)), height: h),
                browser: CGRect(x: gap, y: gap, width: panelWidth, height: max(0, h - 2 * gap)),
                inspector: remaining.inspector.map(shifted), compact: false)
            result.navigation = shifted(remaining.navigation)
            result.viewAccess = shifted(remaining.viewAccess)
            result.history = shifted(remaining.history)
            result.browserAccess = CGRect(x: min(w, inset + gap), y: gap, width: railWidth, height: min(max(0, h - 2 * gap), 52))
            result.floating = true
            return result
        }
        let headerHeight = min(max(0, h - 2 * gap), max(largeText ? 76 : 52, header))
        let headerWidth = min(largeText ? 420 : 340, max(0, w - 2 * (railWidth + 2 * gap)))
        let top = CGRect(x: (w - headerWidth) / 2, y: gap, width: headerWidth, height: headerHeight)
        let bottomHeight = min(max(0, h - top.maxY - 2 * gap), max(48, status))
        let controlsY = min(h, top.maxY + gap)
        let controlsHeight = max(0, h - bottomHeight - 2 * gap - controlsY)
        let railX = railRight ? max(gap, w - gap - railWidth) : gap
        let navX = railRight ? gap : max(gap, w - gap - railWidth)
        let viewWidth = min(max(44, viewButtonSize.width.isFinite ? viewButtonSize.width : 96), max(0, w - 2 * gap))
        let viewHeight = min(max(44, viewButtonSize.height.isFinite ? viewButtonSize.height : 52), max(0, h - bottomHeight - 2 * gap))
        var viewAccess = CGRect(x: railRight ? gap : max(gap, w - gap - viewWidth), y: gap,
                                width: viewWidth, height: viewHeight)
        let browserAccess = CGRect(x: leftBrowser || viewSharesBrowserColumn ? gap : railX, y: gap, width: railWidth, height: min(headerHeight, 52))
        // Wide windows put View beside the header. Crowded windows and mirrored
        // chrome put it above its navigation group without covering the title.
        if viewAccess.intersects(top) || viewAccess.intersects(browserAccess) { viewAccess.origin.y = controlsY }
        viewAccess.origin.y = min(viewAccess.minY, max(gap, h - gap - viewAccess.height))
        let navigationY = min(h, max(controlsY, viewAccess.maxY + gap))
        let sharesRailColumn = railX < viewAccess.maxX && railX + railWidth > viewAccess.minX
        let railY = min(h, sharesRailColumn ? max(controlsY, viewAccess.maxY + gap) : controlsY)
        let rail = CGRect(x: railX, y: railY, width: railWidth,
                          height: min(controlsHeight, max(0, h - bottomHeight - 2 * gap - railY), max(0, railContentHeight)))
        let nav = CGRect(x: navX, y: navigationY, width: railWidth,
                         height: min(max(0, h - bottomHeight - 2 * gap - navigationY), max(0, navigationContentHeight)))
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
            let preferred = inspectorPreferredWidth.isFinite ? max(320, inspectorPreferredWidth) : 320
            let panelWidth = min(max(largeText ? 380 : 320, preferred), max(0, w - 2 * (railWidth + 2 * gap)))
            let x = railRight ? min(w, railWidth + 2 * gap) : max(0, w - 2 * gap - railWidth - panelWidth)
            let overlapsViewColumn = x < viewAccess.maxX && x + panelWidth > viewAccess.minX
            let inspectorY = min(h, overlapsViewColumn ? max(controlsY, viewAccess.maxY + gap) : controlsY)
            inspectorFrame = CGRect(x: x, y: inspectorY, width: panelWidth,
                                    height: min(panelHeight, max(0, h - bottomHeight - gap - inspectorY), max(180, inspectorContentHeight)))
        }
        let historyWidth = min(100, max(0, w - 2 * gap))
        let statusX = min(w, historyWidth + 2 * gap)
        let statusWidth = max(0, min(560, w - statusX - railWidth - 2 * gap))
        var result = WorkspaceLayout(topBar: top,
            status: CGRect(x: statusX, y: max(0, h - bottomHeight - gap), width: statusWidth, height: bottomHeight),
            rail: rail, canvas: canvas, browser: browserFrame, inspector: inspectorFrame, compact: false)
        result.navigation = nav
        result.viewAccess = viewAccess
        result.history = CGRect(x: gap, y: max(0, h - bottomHeight - gap), width: historyWidth, height: bottomHeight)
        result.browserAccess = browserAccess
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
    struct Feature: Decodable, Equatable {
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
    let position: Int?
    let parameters: [Parameter]
    var activeFeatureCount: Int { min(timeline.count, max(0, position ?? timeline.count)) }
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
    var sketch_annotation: String? = nil
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
            // Hover and its id-zero exit are visual feedback, never a contact boundary.
            // A busy renderer must not let hover traffic cancel a real tap.
            if let index = samples.firstIndex(where: hoverFeedback) {
                samples.remove(at: index)
            } else if hoverFeedback(sample) {
                return nil
            } else if let index = (1..<(samples.count - 1)).first(where: { redundantMotion(at: $0) }) {
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
    private func hoverFeedback(_ sample: InputSample) -> Bool {
        sample.phase == .hover || sample.phase == .cancelled && sample.id == 0
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

/// Pure presentation projection. IDs and parent links come from the read model;
/// filtering never changes document visibility, selection or expansion choices.
struct BrowserSearchNode {
    let id: String
    let parent: String?
    let name: String
}

struct BrowserSearchProjection {
    let ids: [String]
    let matches: Set<String>
    let searching: Bool

    static func make(nodes: [BrowserSearchNode], query: String, collapsed: Set<String>) -> Self {
        let needle = query.trimmingCharacters(in: .whitespacesAndNewlines)
            .folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current)
        var byID: [String: BrowserSearchNode] = [:]
        var children: [String: [String]] = [:]
        for node in nodes where byID[node.id] == nil {
            byID[node.id] = node
            children[node.parent ?? "", default: []].append(node.id)
        }
        let searching = !needle.isEmpty
        let matches = searching ? Set(nodes.filter {
            $0.name.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: .current).contains(needle)
        }.map(\.id)) : []
        var included = matches
        for id in matches {
            var parent = byID[id]?.parent
            // Each ancestor is visited at most once, including malformed cycles.
            while let id = parent, included.insert(id).inserted { parent = byID[id]?.parent }
        }
        var pending = Array((children[""] ?? []).reversed())
        var visited: Set<String> = []
        var ids: [String] = []
        while let id = pending.popLast() {
            guard visited.insert(id).inserted else { continue }
            guard !searching || included.contains(id) else { continue }
            ids.append(id)
            if searching || !collapsed.contains(id) {
                pending.append(contentsOf: (children[id] ?? []).reversed())
            }
        }
        return Self(ids: ids, matches: matches, searching: searching)
    }
}

/// Two fixed, separate hit columns for every browser row, including leaves and
/// reference rows. Missing controls occupy inert space, never another hit target.
enum BrowserRowColumns {
    static let touch: CGFloat = 44
    static let width: CGFloat = touch * 2
    static func eye(in bounds: CGRect) -> CGRect {
        CGRect(x: bounds.maxX - width, y: bounds.midY - touch / 2, width: touch, height: touch)
    }
    static func disclosure(in bounds: CGRect) -> CGRect {
        CGRect(x: bounds.maxX - touch, y: bounds.midY - touch / 2, width: touch, height: touch)
    }
}

struct BrowserScrollBookmark {
    let id: String?
    let displacement: CGFloat
    let fallback: CGFloat
    func restoredY(rowY: CGFloat?, maximum: CGFloat) -> CGFloat {
        min(maximum, max(0, rowY.map { $0 + displacement } ?? fallback))
    }
}

/// Keep View below its source and above the workspace's bottom controls. Long
/// pages and larger text scroll inside this viewport instead of growing it.
enum ViewPopoverLayout {
    static func size(contentHeight: CGFloat, container: CGRect, source: CGRect) -> CGSize {
        let width = min(340, max(0, container.width - 24))
        let availableHeight = max(0, container.maxY - max(container.minY, source.maxY) - 80)
        return CGSize(width: width, height: min(max(0, contentHeight), 560, availableHeight))
    }
}

/// Effective state is decoded from the same Rust viewport used by rendering.
/// Preferences describe overrides, never whether an automatic guide is visible.
struct ViewControlsPresentation: Decodable, Equatable {
    struct Origin: Decodable, Equatable { let visible: [Bool]; let choosing: Bool }
    struct Grid: Decodable, Equatable { let visible: Bool; let integrated_axes: Bool }
    struct Camera: Decodable, Equatable {
        let fov: Double
        let yaw: Double?
        let pitch: Double?
        let roll: Double?
        // Read-only matching of the existing renderer camera, never a camera
        // controller or a remembered last-command selection. Orbit clears it.
        var standardView: String? {
            guard let yaw, let pitch, let roll, [yaw, pitch, roll].allSatisfy(\.isFinite) else { return nil }
            func near(_ a: Double, _ b: Double) -> Bool { abs(atan2(sin(a - b), cos(a - b))) < 0.00001 }
            guard near(roll, 0) else { return nil }
            let orientations: [(String, Double, Double)] = [
                ("front", 0, 0), ("back", .pi, 0), ("left", .pi / 2, 0), ("right", -.pi / 2, 0),
                ("top", 0, .pi / 2 - 0.000001), ("bottom", 0, -.pi / 2 + 0.000001),
                ("isometric", -.pi / 4, 0.6154797086703874)
            ]
            return orientations.first { near(yaw, $0.1) && near(pitch, $0.2) }?.0
        }
    }
    enum Visibility { case none, some, all }
    let origin: Origin
    let grid: Grid
    let planar: Bool
    let camera: Camera
    var visibility: Visibility {
        Self.visibility(origin.visible)
    }
    static func visibility(_ values: [Bool]) -> Visibility {
        if !values.contains(true) { return .none }
        return values.allSatisfy { $0 } ? .all : .some
    }
    var masterTarget: Bool { visibility != .all }
    func originVisible(_ item: String) -> Bool {
        guard let index = ["O", "X", "Y", "Z", "XY", "XZ", "YZ"].firstIndex(of: item),
              origin.visible.indices.contains(index) else { return false }
        return origin.visible[index]
    }
    struct Envelope: Decodable { let result: ViewControlsPresentation }
}
