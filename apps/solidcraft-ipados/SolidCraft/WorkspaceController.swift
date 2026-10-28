import UIKit
#if DEBUG
import Darwin
#endif

/// One adaptive native workspace around the existing Metal view and Rust session.
/// Panels change the actual canvas frame; they never invent a second pick origin.
final class FoundationController: UIViewController, UITableViewDataSource, UITableViewDelegate {
    private let canvas = MetalCanvas()
    private let session = NativeSession()
    private let workspace = UIView()
    private let top = UIView()
    private let titleLabel = WorkspaceStyle.label(.headline)
    private let subtitle = WorkspaceStyle.label(.caption2, secondary: true)
    private let status = WorkspaceStyle.label(.caption1, secondary: true)
    private let topScroll = UIScrollView()
    private let topActions = UIStackView()
    private let railScroll = UIScrollView()
    private let rail = UIStackView()
    private let table = UITableView(frame: .zero, style: .plain)
    private let inspectorScroll = UIScrollView()
    private let properties = UIStackView()
    private let scrim = UIControl()
    private let browserMessage = WorkspaceStyle.label(.body, secondary: true)
    private lazy var browserPanel = WorkspacePanel(title: WorkspaceStyle.text("browser")) { [weak self] in self?.closePanels() }
    private lazy var inspectorPanel = WorkspacePanel(title: WorkspaceStyle.text("inspector")) { [weak self] in self?.closePanels() }
    private lazy var undoButton = WorkspaceButton(NativeText.value("ui.ipados.undo")) { [weak self] in self?.run("edit.undo") }
    private lazy var redoButton = WorkspaceButton(NativeText.value("ui.ipados.redo")) { [weak self] in self?.run("edit.redo") }
    private lazy var browserButton = WorkspaceButton(WorkspaceStyle.text("browser")) { [weak self] in self?.toggleBrowser() }
    private lazy var inspectorButton = WorkspaceButton(WorkspaceStyle.text("inspector")) { [weak self] in self?.toggleInspector() }
    private lazy var viewButton = WorkspaceButton(WorkspaceStyle.text("view"), action: {})
    private lazy var selectButton = WorkspaceButton(WorkspaceStyle.text("select")) { [weak self] in self?.setTool("select") }
    private lazy var radiusButton = WorkspaceButton(WorkspaceStyle.text("radius")) { [weak self] in self?.setTool("post_radius") }
    private lazy var moreButton = WorkspaceButton(WorkspaceStyle.text("tools"), action: {})
    private lazy var modellingButton = WorkspaceButton(ModellingText.text("tools"), action: {})
    private lazy var assemblyButton = WorkspaceButton(AssemblyText.text("tools"), action: {})
    private lazy var cancelToolButton = WorkspaceButton(SketchText.text("cancel_segment")) { [weak self] in self?.run("tool.cancel") }
    private lazy var createSketchButton = WorkspaceButton(SketchText.text("create")) { [weak self] in self?.chooseSketchPlane() }
    private lazy var sketchToolsButton = WorkspaceButton(SketchText.text("draw"), action: {})
    private lazy var finishSketchButton = WorkspaceButton(SketchText.text("finish"), prominent: true) { [weak self] in self?.run("sketch.finish") }
    private lazy var finishChainButton = WorkspaceButton(SketchText.text("finish_chain")) { [weak self] in self?.run("tool.finish_chain") }
    private var state: WorkspaceState?
    private var visibleRows: [WorkspaceRow] = []
    private var rowByID: [String: WorkspaceRow] = [:]
    private var parents: Set<String> = []
    private var collapsed: Set<String> = []
    private var browserOpen = false
    private var inspectorOpen = false
    private var multiple = false
    private var filter = "face"
    private var perspective = false
    private var active = false
    private var generation: UInt64 = 0
    private var failure: String?
    private var pending = ""
    private var previewStale = false
    private var developmentCheckStarted = false
#if DEBUG
    private var assemblyCheckRunning = false
    private var assemblyCheckTicket: UInt64 = 0
    private var assemblyCheckSignal: DispatchSourceSignal?
    private var assemblyCheckContinue: (() -> Void)?
    private var assemblyCheckWorkspace: ((WorkspaceState) -> Void)?
    private var assemblyCheckBegan: ((Result<ModellingState, BridgeError>) -> Void)?
    private var assemblyCheckEvaluated: ((Result<ModellingState, BridgeError>, Bool) -> Void)?
    private var assemblyCheckCancelled: ((Result<ModellingState, BridgeError>) -> Void)?
    private var assemblyCheckFrame: (document: String, revision: UInt64, preview: Bool, stage: String, next: () -> Void)?
    private var assemblyCheckLog: URL?
    private var assemblyCheckFile: URL?
    private var assemblyCheckFrameID: UInt64 = 0
    private var assemblyCheckArmID: UInt64 = 0
    private var assemblyCheckSlideID: UInt64 = 0
    private var assemblyCheckHingeID: UInt64 = 0
    private var assemblyCheckSliderID: UInt64 = 0
#endif
    private var layout: WorkspaceLayout?
    private var numericToken: UInt64?
    private var numericContext: (document: String, revision: UInt64)?
    private var editor: NumericExpressionEditor?
    private var numericStarting = false
    private var numericGeneration: UInt64 = 0
    private var sketchInput: SketchInputPresentation?
    private var planeDraft: (value: Any, label: String)?
    private var planeGeneration: UInt64 = 0
    private var planeConfirming = false
    private var showAllConstraints = false
    private var annotationLabels: [String: UILabel] = [:]
    private var constraintLabels: [String: UILabel] = [:]
    private struct TransformDraft {
        let entities: [String]
        let rotate: Bool
        let copy: Bool
    }
    private var transformDraft: TransformDraft?
    private var transformEditors: [(name: String, kind: String, editor: NumericExpressionEditor)] = []
    private var transformGeneration: UInt64 = 0
    private var transformMessage: WorkspaceMessage?
    private var transformBusy = false
    private var modellingStarting = false
    private var modellingToken: UInt64?
    private var modellingFeature: UInt64?
    private var modellingContext: (document: String, revision: UInt64)?
    private var modellingGeneration: UInt64 = 0
    private var modellingInspector: ModellingInspector?
    private var modellingInspectPending = false
    private var modellingCommitting = false
    private var modellingPreviewQueue = ModellingPreviewQueue()
    private var modellingFormVersion: UInt64 = 0
    private var modellingContextInspectInFlight = false
    private var pendingModellingContext: [String: PresentationJSON]?
    private var modellingContextNeedsRefresh = false
    private var previewAfterModellingContext = false
    private var selectedAssemblyRowID: String?
    private var lastOperationFeedback: [String] = []
    private var hasLocalSave = false
    private var localSaveLabel: String?
    private var documentStatus = WorkspaceStyle.text("unsaved_session")
    private var keyboardTop: CGFloat?
    private var railRight = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.railRight")
    private var appearance = UserDefaults.standard.integer(forKey: "SolidCraft.workspace.appearance")

    private var numericEditing: Bool { numericStarting || numericToken != nil || transformDraft != nil }
    private var modellingEditing: Bool { modellingStarting || modellingToken != nil }
    private var textEntryActive: Bool { editor?.field.isFirstResponder == true || transformEditors.contains { $0.editor.field.isFirstResponder } || modellingInspector?.textEntryActive == true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.background
        applyAppearance()
        view.addSubview(workspace)
        workspace.addSubview(canvas)
        workspace.addSubview(top)
        workspace.addSubview(status)
        workspace.addSubview(railScroll)
        workspace.addSubview(scrim)
        workspace.addSubview(browserPanel)
        workspace.addSubview(inspectorPanel)
        scrim.backgroundColor = WorkspaceStyle.scrim
        scrim.addAction(UIAction { [weak self] _ in self?.closePanels() }, for: .touchUpInside)
        scrim.accessibilityLabel = WorkspaceStyle.text("close")
        scrim.accessibilityIdentifier = "workspace-modal-scrim"
        top.addSubview(titleLabel); top.addSubview(subtitle); top.addSubview(topScroll)
        titleLabel.numberOfLines = 1
        titleLabel.accessibilityTraits = .header
        subtitle.numberOfLines = 2
        updateSubtitle()
        topActions.axis = .horizontal; topActions.spacing = 2
        [undoButton, redoButton, browserButton, inspectorButton, viewButton].forEach(topActions.addArrangedSubview)
        topScroll.addSubview(topActions)
        topScroll.showsHorizontalScrollIndicator = true
        topActions.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            topActions.leadingAnchor.constraint(equalTo: topScroll.contentLayoutGuide.leadingAnchor),
            topActions.trailingAnchor.constraint(equalTo: topScroll.contentLayoutGuide.trailingAnchor),
            topActions.topAnchor.constraint(equalTo: topScroll.contentLayoutGuide.topAnchor),
            topActions.bottomAnchor.constraint(equalTo: topScroll.contentLayoutGuide.bottomAnchor),
            topActions.heightAnchor.constraint(equalTo: topScroll.frameLayoutGuide.heightAnchor)
        ])
        rail.axis = .vertical; rail.spacing = 8; rail.alignment = .fill
        [selectButton, createSketchButton, sketchToolsButton, finishChainButton, finishSketchButton, modellingButton, assemblyButton, radiusButton, moreButton, cancelToolButton].forEach(rail.addArrangedSubview)
        railScroll.addSubview(rail)
        rail.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            rail.leadingAnchor.constraint(equalTo: railScroll.contentLayoutGuide.leadingAnchor),
            rail.trailingAnchor.constraint(equalTo: railScroll.contentLayoutGuide.trailingAnchor),
            rail.topAnchor.constraint(equalTo: railScroll.contentLayoutGuide.topAnchor),
            rail.bottomAnchor.constraint(equalTo: railScroll.contentLayoutGuide.bottomAnchor),
            rail.widthAnchor.constraint(equalTo: railScroll.frameLayoutGuide.widthAnchor)
        ])
        railScroll.showsVerticalScrollIndicator = false
        railScroll.backgroundColor = WorkspaceStyle.surface
        railScroll.layer.cornerRadius = WorkspaceStyle.radius
        for button in [selectButton, radiusButton, moreButton] {
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 60).isActive = true
        }
        canvas.accessibilityIdentifier = "native-cad-surface"
        canvas.isAccessibilityElement = true
        canvas.accessibilityLabel = WorkspaceStyle.text("viewport")
        canvas.accessibilityHint = WorkspaceStyle.text("select_hint")
        status.numberOfLines = 2
        status.accessibilityIdentifier = "workspace-status"
        status.text = WorkspaceStyle.text("loading")
        table.backgroundColor = .clear
        table.separatorStyle = .none
        table.rowHeight = UITableView.automaticDimension
        table.estimatedRowHeight = 56
        table.dataSource = self; table.delegate = self
        table.register(UITableViewCell.self, forCellReuseIdentifier: "model-row")
        table.accessibilityIdentifier = "workspace-model-browser"
        browserPanel.content.addSubview(table)
        browserMessage.textAlignment = .center
        browserMessage.text = WorkspaceStyle.text("loading")
        browserMessage.isUserInteractionEnabled = true
        browserMessage.addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(retryBrowser)))
        table.backgroundView = browserMessage
        inspectorPanel.content.addSubview(inspectorScroll)
        inspectorScroll.keyboardDismissMode = .interactive
        inspectorScroll.addSubview(properties)
        properties.axis = .vertical; properties.spacing = 18
        properties.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            properties.topAnchor.constraint(equalTo: inspectorScroll.contentLayoutGuide.topAnchor, constant: 12),
            properties.leadingAnchor.constraint(equalTo: inspectorScroll.contentLayoutGuide.leadingAnchor, constant: 16),
            properties.trailingAnchor.constraint(equalTo: inspectorScroll.contentLayoutGuide.trailingAnchor, constant: -16),
            properties.bottomAnchor.constraint(equalTo: inspectorScroll.contentLayoutGuide.bottomAnchor, constant: -20),
            properties.widthAnchor.constraint(equalTo: inspectorScroll.frameLayoutGuide.widthAnchor, constant: -32)
        ])
        for (button, id) in [(undoButton, "undo"), (redoButton, "redo"), (browserButton, "browser"),
                             (inspectorButton, "inspector"), (viewButton, "view"), (selectButton, "select"),
                             (radiusButton, "radius"), (moreButton, "tools")] {
            button.accessibilityIdentifier = "workspace-" + id
        }
        canvas.onResize = { [weak self] in self?.updateSurface() }
        canvas.historyGesturesEnabled = UserDefaults.standard.object(forKey: "SolidCraft.workspace.historyGestures") as? Bool ?? true
        canvas.onUndo = { [weak self] in
            guard let self, self.active, self.undoButton.isEnabled else { return }
            self.run("edit.undo")
        }
        canvas.onRedo = { [weak self] in
            guard let self, self.active, self.redoButton.isEnabled else { return }
            self.run("edit.redo")
        }
        canvas.onPreviousTool = { [weak self] in
            guard let self, !self.numericEditing, !self.modellingEditing else { return }
            self.session.command("tool.previous")
        }
        canvas.onCancel = { [weak self] in self?.session.cancelInput() }
        canvas.onInput = { [weak self] sample in self?.session.record(sample) }
        session.onFailure = { [weak self] error in self?.showFailure(error) }
        session.onWorkspace = { [weak self] value in self?.receive(value) }
        session.onSketchInput = { [weak self] value in
            guard let self, self.sketchInput != value else { return }
            self.sketchInput = value; self.updateStatus(); self.rebuildMenus()
        }
        session.onSketchAnnotations = { [weak self] annotations in self?.showSketchAnnotations(annotations) }
        session.onConstraintIndicators = { [weak self] indicators in self?.showConstraintIndicators(indicators) }
        session.onToolWorkCompleted = { [weak self] in self?.inspectModelling() }
        session.onStatus = { [weak self] message in
            guard let self else { return }
            self.pending = message; self.failure = nil; self.updateStatus(); self.updateAvailability()
        }
        session.onFrame = { [weak self] presented, revision, epoch, cameraEpoch, documentID, metrics, preview, previewStale, _ in
            guard let self, self.active else { return }
            if presented {
                guard let metrics, metrics.count == 3, let pixels = self.canvas.pixelSize,
                      abs(metrics[0] * metrics[2] - Double(pixels.width)) < 0.01,
                      abs(metrics[1] * metrics[2] - Double(pixels.height)) < 0.01,
                      abs(metrics[2] - Double(self.canvas.contentScaleFactor)) < 0.000001 else { return }
                self.canvas.geometryRevision = revision; self.canvas.viewportEpoch = epoch
                self.canvas.cameraEpoch = cameraEpoch; self.canvas.documentID = documentID
                self.canvas.displayingPreview = preview
                let staleChanged = self.previewStale != previewStale
                self.previewStale = previewStale
                if previewStale { self.modellingInspector?.invalidatePreview() }
                if staleChanged { self.updateStatus() }
            }
            #if DEBUG
            if presented, !self.developmentCheckStarted {
                if ProcessInfo.processInfo.arguments.contains("--input-check") {
                    self.developmentCheckStarted = true; self.session.developmentInputCheck()
                } else if ProcessInfo.processInfo.arguments.contains("--viewport-check") {
                    self.developmentCheckStarted = true; self.session.developmentCheck()
                } else if ProcessInfo.processInfo.arguments.contains("--workspace-check") {
                    self.developmentCheckStarted = true; self.session.developmentWorkspaceCheck()
                } else if ProcessInfo.processInfo.arguments.contains("--sketch-check") {
                    self.developmentCheckStarted = true
                    self.session.developmentSketchCheck { [weak self] workspace in
                        guard let self else { return }
                        self.receive(workspace); self.inspectorOpen = true; self.browserOpen = false
                        self.rebuildInspector(); self.relayoutPanels()
                    }
                } else if ProcessInfo.processInfo.arguments.contains("--assembly-check") {
                    self.developmentCheckStarted = true; self.startAssemblyCheck()
                }
            }
            if presented { self.assemblyCheckPresented(document: documentID, revision: revision, preview: preview) }
            #endif
        }
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
        let expected = generation
        session.start { [weak self] result in
            guard let self, self.generation == expected else { return }
            switch result {
            case .success: self.failure = nil; self.updateSurface()
            case .failure(let error): self.showFailure(error)
            }
        }
        rebuildMenus(); rebuildInspector(); updateAvailability()
    }

    private func receive(_ value: WorkspaceState) {
        if let context = modellingContext,
           context.document != value.document_id || context.revision != value.geometry_revision {
            clearModelling()
            failure = WorkspaceStyle.text("invalidated")
        }
        if let context = numericContext,
           context.document != value.document_id || context.revision != value.geometry_revision {
            // An external authoritative update invalidates the draft; never apply it
            // to a replacement document. Ordinary selection reports do not steal focus.
            numericGeneration &+= 1
            numericToken = nil; numericContext = nil; numericStarting = false
            editor?.field.resignFirstResponder(); editor = nil
            transformGeneration &+= 1; transformDraft = nil
            transformBusy = false
            transformEditors.forEach { $0.editor.field.resignFirstResponder() }; transformEditors.removeAll(); transformMessage = nil
            failure = WorkspaceStyle.text("invalidated")
        }
        let previous = state
        state = value
        if let label = localSaveLabel {
            documentStatus = label; localSaveLabel = nil; hasLocalSave = true
        } else if hasLocalSave {
            if value.dirty == true { documentStatus = ModellingText.text("unsaved_changes") }
            else if value.dirty == false { documentStatus = ModellingText.text("saved_local") }
            else if previous?.geometry_revision != value.geometry_revision || previous?.document_id != value.document_id { documentStatus = ModellingText.text("unsaved_changes") }
        }
        updateSubtitle()
        if value.sketch != nil { planeDraft = nil }
        titleLabel.text = value.title
        titleLabel.accessibilityLabel = value.title
        canvas.accessibilityValue = value.selection.isEmpty ? NativeText.value("ui.ipados.no_selection") : value.selection.map(\.readout).joined(separator: ", ")
        if previous?.document_id != value.document_id { collapsed.removeAll(); selectedAssemblyRowID = nil; lastOperationFeedback = [] }
        if previous?.selection != value.selection, previous?.geometry_revision == value.geometry_revision {
            if let id = selectedAssemblyRowID, let row = value.rows.first(where: { $0.id == id }),
               row.entity == nil || !value.selection.contains(where: { $0 == row.entity }) {
                selectedAssemblyRowID = nil
            }
            lastOperationFeedback = []
        }
        if previous?.geometry_revision != value.geometry_revision || previous?.document_id != value.document_id {
            rowByID = Dictionary(value.rows.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
            parents = Set(value.rows.compactMap(\.parent_id))
            if let id = selectedAssemblyRowID, rowByID[id] == nil { selectedAssemblyRowID = nil }
        }
        if previous?.selection != value.selection {
            for row in value.rows where row.selected {
                var parent = row.parent_id
                var visited: Set<String> = []
                while let id = parent, visited.insert(id).inserted {
                    collapsed.remove(id); parent = rowByID[id]?.parent_id
                }
            }
        }
        if previous?.selection != value.selection || previous?.geometry_revision != value.geometry_revision || previous?.document_id != value.document_id {
            updateRows()
        }
        if !numericEditing && !modellingEditing { rebuildInspector() }
        if modellingToken != nil && previous?.selection != value.selection { inspectModelling() }
        updateAvailability(); rebuildMenus(); updateStatus(); view.setNeedsLayout()
#if DEBUG
        assemblyCheckWorkspace?(value)
#endif
    }

    private func updateRows() {
        guard let state else { return }
        // The read model carries authoritative parent IDs. Arrange them in tree
        // order without inferring any feature/body ownership from display names.
        let children = Dictionary(grouping: state.rows, by: { $0.parent_id ?? "" })
        var pendingRows = Array((children[""] ?? []).reversed())
        var visited: Set<String> = []
        visibleRows.removeAll(keepingCapacity: true)
        while let row = pendingRows.popLast(), visited.insert(row.id).inserted {
            visibleRows.append(row)
            if !collapsed.contains(row.id) { pendingRows.append(contentsOf: (children[row.id] ?? []).reversed()) }
        }
        browserMessage.text = state.rows.contains(where: { $0.entity != nil }) ? nil : WorkspaceStyle.text("empty")
        table.reloadData()
    }

    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { visibleRows.count }
    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "model-row", for: indexPath)
        guard visibleRows.indices.contains(indexPath.row) else { return cell }
        let row = visibleRows[indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = row.label
        let feature = state?.modelling?.timeline.first(where: { row.id == "feature:" + String($0.id) })
        if let feature {
            content.secondaryText = [String(feature.order + 1) + " · " + ModellingText.label(feature.kind),
                                     feature.id == modellingFeature ? ModellingText.text("editing") : nil,
                                     feature.suppressed ? ModellingText.text("suppressed") : nil,
                                     feature.stale == true ? ModellingText.text("stale") : nil,
                                     feature.error, feature.warning].compactMap { $0 }.joined(separator: " · ")
        } else if let assembly = row.assembly {
            content.secondaryText = assemblyReadout(assembly).joined(separator: " · ")
        } else { content.secondaryText = WorkspaceStyle.text(row.kind) }
        content.textProperties.font = .preferredFont(forTextStyle: parents.contains(row.id) ? .headline : .body)
        content.textProperties.numberOfLines = 0
        content.secondaryTextProperties.color = WorkspaceStyle.secondary
        var depth = 0
        var parent = row.parent_id
        var visited: Set<String> = []
        while let id = parent, visited.insert(id).inserted { depth += 1; parent = rowByID[id]?.parent_id }
        content.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 10, leading: 16 + CGFloat(min(depth, 4)) * 12, bottom: 10, trailing: 8)
        cell.contentConfiguration = content
        var background = UIBackgroundConfiguration.listPlainCell()
        let selected = row.selected || selectedAssemblyRowID == row.id
        background.backgroundColor = selected ? WorkspaceStyle.selectionFill : .clear
        background.cornerRadius = 8
        cell.backgroundConfiguration = background
        cell.accessoryView = nil
        cell.accessoryType = selected ? .checkmark : .none
        if parents.contains(row.id) {
            let expand = WorkspaceButton(collapsed.contains(row.id) ? "+" : "−") { [weak self] in self?.toggleGroup(row.id) }
            expand.accessibilityLabel = WorkspaceStyle.text(collapsed.contains(row.id) ? "open_group" : "close_group") + ": " + row.label
            expand.frame = CGRect(x: 0, y: 0, width: 44, height: 44)
            cell.accessoryView = expand
        }
        cell.accessibilityLabel = row.label
        cell.accessibilityValue = content.secondaryText
        cell.accessibilityTraits = selected ? [.button, .selected] : .button
        cell.accessibilityHint = numericEditing ? WorkspaceStyle.text("editing_lock") : nil
        cell.selectionStyle = .none
        cell.accessibilityIdentifier = "workspace-row-" + row.id
        return cell
    }
    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        guard !numericEditing, visibleRows.indices.contains(indexPath.row) else { return }
        let row = visibleRows[indexPath.row]
        selectedAssemblyRowID = row.assembly == nil ? nil : row.id
        lastOperationFeedback = []
        if let entity = row.entity {
            canvas.cancelInput()
            session.selectEntity(entity.dictionary, additive: multiple) { [weak self] result in
                guard case .success = result, (row.kind == "sketch" || row.kind == "feature" || row.assembly != nil), let self, !self.multiple else { return }
                if self.modellingEditing { return }
                self.inspectorOpen = true; self.browserOpen = false
                self.rebuildInspector(); self.relayoutPanels()
            }
        } else if row.assembly != nil {
            inspectorOpen = true; browserOpen = false; rebuildInspector(); relayoutPanels(); table.reloadData()
        } else if parents.contains(row.id) { toggleGroup(row.id) }
    }
    private func toggleGroup(_ id: String) {
        if collapsed.contains(id) { collapsed.remove(id) } else { collapsed.insert(id) }
        updateRows()
    }

    private func rebuildInspector() {
        guard !numericEditing, !modellingEditing else { return }
        properties.arrangedSubviews.forEach { properties.removeArrangedSubview($0); $0.removeFromSuperview() }
        guard let state else {
            let loading = WorkspaceMessage(); loading.show(WorkspaceStyle.text("loading"), busy: true); properties.addArrangedSubview(loading); return
        }
        if !lastOperationFeedback.isEmpty {
            let message = WorkspaceMessage(); message.show(lastOperationFeedback.map(ModellingText.label).joined(separator: "\n")); properties.addArrangedSubview(message)
        }
        if let assembly = state.assembly { addAssemblyContext(assembly) }
        if let draft = planeDraft {
            properties.addArrangedSubview(WorkspacePropertyRow(label: SketchText.text("plane"), value: draft.label))
            let hint = WorkspaceStyle.label(.body, secondary: true); hint.text = SketchText.text("plane_preview"); properties.addArrangedSubview(hint)
            let confirm = WorkspaceButton(SketchText.text("confirm_plane"), prominent: true) { [weak self] in self?.confirmSketchPlane() }
            let cancel = WorkspaceButton(WorkspaceStyle.text("cancel")) { [weak self] in self?.cancelSketchPlane() }
            confirm.availability(enabled: !planeConfirming)
            cancel.availability(enabled: !planeConfirming)
            properties.addArrangedSubview(confirm)
            properties.addArrangedSubview(cancel)
            return
        }
        let selection = state.selection
        if let sketch = state.sketch {
            rebuildSketchInspector(sketch, selection: selection)
            return
        }
        if let id = selectedAssemblyRowID, let row = state.rows.first(where: { $0.id == id }), let assembly = row.assembly {
            addAssemblyInspector(row, assembly: assembly)
            return
        }
        if selection.isEmpty {
            let hint = WorkspaceStyle.label(.body, secondary: true); hint.text = WorkspaceStyle.text("no_properties"); properties.addArrangedSubview(hint)
        } else {
            properties.addArrangedSubview(WorkspacePropertyRow(label: WorkspaceStyle.text("selection"), value: selection.prefix(8).map(\.readout).joined(separator: "\n")))
            if selection.count > 1 { properties.addArrangedSubview(WorkspacePropertyRow(label: WorkspaceStyle.text("selected_count"), value: String(selection.count))) }
        }
        if let radius = state.radius {
            properties.addArrangedSubview(WorkspacePropertyRow(label: WorkspaceStyle.text("radius"), value: radius.expression + " · " + radius.units))
            let edit = WorkspaceButton(WorkspaceStyle.text("edit_radius"), prominent: true) { [weak self] in self?.beginNumeric() }
            edit.accessibilityIdentifier = "workspace-edit-radius"
            properties.addArrangedSubview(edit)
            let range = WorkspaceStyle.label(.footnote, secondary: true); range.text = WorkspaceStyle.text("numeric_range"); properties.addArrangedSubview(range)
        } else if let reason = state.tools.first(where: { $0.id == "post_radius" })?.reason {
            let explanation = WorkspaceStyle.label(.footnote, secondary: true)
            explanation.text = NativeText.value(reason)
            properties.addArrangedSubview(explanation)
        }
        if let selected = selection.first(where: { $0.type == "feature" }),
           let row = state.rows.first(where: { $0.kind == "sketch" && $0.entity == selected }), let id = selected.id {
            properties.addArrangedSubview(WorkspaceButton(SketchText.text("reopen"), prominent: true) { [weak self] in self?.run("sketch.edit", params: ["sketch": id.value]) })
            properties.addArrangedSubview(WorkspacePropertyRow(label: WorkspaceStyle.text("sketch"), value: row.label))
            properties.addArrangedSubview(WorkspaceButton(SketchText.text("delete_sketch")) { [weak self] in self?.confirmDeleteSketch(id: id.value) })
        }
        if let selected = selection.first(where: { $0.type == "feature" }),
           case .number(let id)? = selected.id,
           let feature = state.modelling?.timeline.first(where: { $0.id == id }) {
            addFeatureInspector(feature)
        }
        if let modelling = state.modelling, !modelling.parameters.isEmpty {
            let heading = WorkspaceStyle.label(.headline); heading.text = ModellingText.text("parameters"); properties.addArrangedSubview(heading)
            for parameter in modelling.parameters {
                let readout = [parameter.expression, parameter.value.map { String($0) + " " + parameter.unit }].compactMap { $0 }.joined(separator: " · ")
                properties.addArrangedSubview(WorkspacePropertyRow(label: parameter.name, value: readout))
                properties.addArrangedSubview(WorkspaceButton(ModellingText.text("edit_parameter") + ": " + parameter.name) { [weak self] in
                    self?.beginModelling(operation: "parameters.change", initialParams: ["name": .string(parameter.name), "expression": .string(parameter.expression)])
                })
            }
        }
        if state.modelling?.tools.contains(where: { $0.id == "parameters.add" }) == true {
            properties.addArrangedSubview(WorkspaceButton(ModellingText.text("add_parameter")) { [weak self] in self?.beginModelling(operation: "parameters.add") })
        }
        let guidance = WorkspaceStyle.label(.footnote, secondary: true)
        guidance.text = WorkspaceStyle.text(state.active_tool == "post_radius" ? "radius_hint" : "select_hint")
        properties.addArrangedSubview(guidance)
    }

    private func addFeatureInspector(_ feature: ModellingPresentation.Feature) {
        let heading = WorkspaceStyle.label(.headline); heading.text = feature.name; properties.addArrangedSubview(heading)
        properties.addArrangedSubview(WorkspacePropertyRow(label: ModellingText.text("feature_type"), value: ModellingText.label(feature.kind)))
        if let error = feature.error {
            let message = WorkspaceMessage(); message.show(error, error: true); properties.addArrangedSubview(message)
        }
        if let warning = feature.warning { let message = WorkspaceMessage(); message.show(warning); properties.addArrangedSubview(message) }
        if feature.stale == true { let message = WorkspaceMessage(); message.show(ModellingText.text("stale_geometry"), error: true); properties.addArrangedSubview(message) }
        if state?.rows.first(where: { $0.id == "feature:" + String(feature.id) })?.kind != "sketch" {
            properties.addArrangedSubview(WorkspaceButton(ModellingText.text("edit_feature"), prominent: true) { [weak self] in self?.beginModelling(feature: feature.id) })
        }
        properties.addArrangedSubview(WorkspaceButton(ModellingText.text("rename")) { [weak self] in self?.renameFeature(feature) })
        properties.addArrangedSubview(WorkspaceButton(ModellingText.text("dependencies")) { [weak self] in self?.showFeatureDependencies(feature) })
        properties.addArrangedSubview(WorkspaceButton(ModellingText.text(feature.suppressed ? "unsuppress" : "suppress")) { [weak self] in
            self?.run("timeline.suppress", params: ["feature": feature.id, "suppressed": !feature.suppressed])
        })
        properties.addArrangedSubview(WorkspaceButton(ModellingText.text("delete_feature")) { [weak self] in self?.deleteFeature(feature) })
    }

    private func assemblyReadout(_ assembly: AssemblyRowPresentation) -> [String] {
        var values: [String] = []
        if let type = assembly.joint_type { values.append(ModellingText.label(type)) }
        if let suppressed = assembly.suppressed { values.append(AssemblyText.text(suppressed ? "suppressed" : "enabled")) }
        if assembly.active == true { values.append(AssemblyText.text("active")) }
        if let grounded = assembly.grounded { values.append(AssemblyText.text(grounded ? "grounded" : "free")) }
        if assembly.visible == false { values.append(AssemblyText.text("hidden")) }
        if assembly.pending == true { values.append(AssemblyText.text("pending")) }
        if let diagnostic = assembly.diagnostic { values.append(ModellingText.label(diagnostic)) }
        return values
    }

    private func addAssemblyContext(_ assembly: AssemblyPresentation) {
        let name = assembly.active_name + (assembly.active_path.isEmpty ? "" : " · " + assembly.active_path.map(String.init).joined(separator: " / "))
        properties.addArrangedSubview(WorkspacePropertyRow(label: AssemblyText.text("authoring_context"), value: name))
        for diagnostic in assembly.diagnostics {
            let message = WorkspaceMessage(); message.show(ModellingText.label(diagnostic)); properties.addArrangedSubview(message)
        }
        addAssemblyActions(assembly.actions)
    }

    private func updateSubtitle() {
        if let assembly = state?.assembly {
            let path = assembly.active_path.isEmpty ? "" : " · " + assembly.active_path.map(String.init).joined(separator: " / ")
            subtitle.text = AssemblyText.text("authoring_context") + ": " + assembly.active_name + path + " · " + documentStatus
        } else { subtitle.text = documentStatus }
        subtitle.accessibilityLabel = subtitle.text
    }

    private func addAssemblyInspector(_ row: WorkspaceRow, assembly: AssemblyRowPresentation) {
        let title = WorkspaceStyle.label(.headline); title.text = row.label; properties.addArrangedSubview(title)
        let readout = assemblyReadout(assembly)
        if !readout.isEmpty { properties.addArrangedSubview(WorkspacePropertyRow(label: AssemblyText.text("state"), value: readout.joined(separator: "\n"))) }
        if let path = assembly.path { properties.addArrangedSubview(WorkspacePropertyRow(label: AssemblyText.text("placement_path"), value: path.map(String.init).joined(separator: " / "))) }
        addAssemblyActions(assembly.actions)
    }

    private func addAssemblyActions(_ actions: [AssemblyActionPresentation]) {
        for action in actions {
            let button = WorkspaceButton(ModellingText.label(action.label)) { [weak self] in self?.performAssemblyAction(action) }
            button.availability(enabled: action.command != nil || action.operation != nil)
            properties.addArrangedSubview(button)
        }
    }

    private func performAssemblyAction(_ action: AssemblyActionPresentation) {
        guard !numericEditing, !modellingEditing, let context = state else { return }
        let perform = { [weak self] in
            guard let self, self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else { return }
            if let operation = action.operation { self.beginModelling(operation: operation, initialParams: action.params) }
            else if let command = action.command { self.run(command, params: action.params.mapValues(\.value)) }
        }
        if let confirmation = action.confirm {
            canvas.cancelInput()
            let alert = UIAlertController(title: ModellingText.label(action.label), message: ModellingText.label(confirmation), preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
            alert.addAction(UIAlertAction(title: ModellingText.label(action.label), style: .default) { _ in perform() })
            present(alert, animated: true)
        } else { perform() }
    }

    private func renameFeature(_ feature: ModellingPresentation.Feature) {
        guard let context = state, !numericEditing, !modellingEditing else { return }
        let alert = UIAlertController(title: ModellingText.text("rename"), message: feature.name, preferredStyle: .alert)
        alert.addTextField { field in field.text = feature.name; field.autocorrectionType = .no; field.accessibilityLabel = ModellingText.text("name") }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("apply"), style: .default) { [weak self, weak alert] _ in
            guard let self, self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision,
                  let name = alert?.textFields?.first?.text else { return }
            self.run("timeline.rename", params: ["feature": feature.id, "name": name])
        })
        canvas.cancelInput(); present(alert, animated: true)
    }

    private func showFeatureDependencies(_ feature: ModellingPresentation.Feature) {
        guard let context = state, !numericEditing, !modellingEditing else { return }
        session.sketchDependents(feature.id) { [weak self] result in
            guard let self, !self.numericEditing, !self.modellingEditing, self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else { return }
            switch result {
            case .failure(let error): self.showFailure(error)
            case .success(let dependencies):
                var parts: [String] = []
                if !dependencies.deleted_with_it.isEmpty { parts.append(ModellingText.text("deleted_dependents") + "\n" + dependencies.deleted_with_it.joined(separator: "\n")) }
                if !dependencies.would_fail.isEmpty { parts.append(ModellingText.text("failing_dependents") + "\n" + dependencies.would_fail.joined(separator: "\n")) }
                let alert = UIAlertController(title: feature.name + " · " + ModellingText.text("dependencies"), message: parts.isEmpty ? ModellingText.text("no_dependencies") : parts.joined(separator: "\n\n"), preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: WorkspaceStyle.text("done"), style: .default))
                self.canvas.cancelInput(); self.present(alert, animated: true)
            }
        }
    }

    private func deleteFeature(_ feature: ModellingPresentation.Feature) {
        guard let context = state, !numericEditing, !modellingEditing else { return }
        session.sketchDependents(feature.id) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.showFailure(error)
            case .success(let dependencies):
                guard self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else {
                    self.showFailure(BridgeError(code: "stale_edit", message: WorkspaceStyle.text("invalidated"))); return
                }
                var message = ModellingText.text("delete_warning")
                if !dependencies.deleted_with_it.isEmpty { message += "\n\n" + ModellingText.text("deleted_dependents") + "\n" + dependencies.deleted_with_it.joined(separator: "\n") }
                if !dependencies.would_fail.isEmpty { message += "\n\n" + ModellingText.text("failing_dependents") + "\n" + dependencies.would_fail.joined(separator: "\n") }
                let alert = UIAlertController(title: ModellingText.text("delete_feature") + ": " + feature.name, message: message, preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
                alert.addAction(UIAlertAction(title: ModellingText.text("delete_feature"), style: .destructive) { [weak self] _ in
                    guard let self, self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else { return }
                    self.run("timeline.delete", params: ["feature": feature.id])
                })
                self.canvas.cancelInput(); self.present(alert, animated: true)
            }
        }
    }

    private func beginModelling(operation: String? = nil, feature: UInt64? = nil, initialParams: [String: PresentationJSON]? = nil) {
        guard !numericEditing, !modellingEditing, let context = state else { return }
        canvas.cancelInput(); modellingGeneration &+= 1; lastOperationFeedback = []
        let expected = modellingGeneration
        modellingStarting = true; modellingContext = (context.document_id, context.geometry_revision)
        updateAvailability()
        session.requestModelling(action: "begin", operation: operation, feature: feature, params: initialParams) { [weak self] result in
            guard let self, self.modellingGeneration == expected else { return }
            self.modellingStarting = false
            switch result {
            case .failure(let error): self.clearModelling(); self.showFailure(error)
            case .success(let model):
                guard let token = model.token else {
                    self.clearModelling(); self.showFailure(BridgeError(code: "stale_edit", message: WorkspaceStyle.text("invalidated"))); return
                }
                self.modellingToken = token
                self.modellingFeature = model.feature; self.table.reloadData()
                let form = ModellingInspector(state: model)
                self.modellingInspector = form
                self.properties.arrangedSubviews.forEach { self.properties.removeArrangedSubview($0); $0.removeFromSuperview() }
                self.properties.addArrangedSubview(form)
                form.onPreview = { [weak self] params in self?.evaluateModelling(params, commit: false) }
                form.onLivePreview = { [weak self] params in self?.evaluateModelling(params, commit: false, live: true) }
                form.onContextChange = { [weak self] params in self?.refreshModellingContext(params) }
                form.onApply = { [weak self] params in self?.evaluateModelling(params, commit: true) }
                form.onCancel = { [weak self] in self?.cancelModelling() }
                form.onFocus = { [weak self] in self?.canvas.cancelInput(); self?.revealField(); self?.updateAvailability() }
                form.onChange = { [weak self] in self?.failure = nil; self?.updateStatus() }
                form.onSelect = { [weak self, weak form] in
                    guard let self else { return }
                    form?.invalidatePreview()
                    self.session.requestModelling(action: "selection", token: token) { [weak self] result in
                        guard let self, self.modellingToken == token else { return }
                        if case .failure(let error) = result { self.showFailure(error); return }
                        self.inspectorOpen = false; self.browserOpen = false; self.relayoutPanels()
                    }
                }
                self.inspectorOpen = true; self.browserOpen = false; self.failure = nil
                self.relayoutPanels()
            }
            self.updateAvailability(); self.updateStatus()
#if DEBUG
            let check = self.assemblyCheckBegan; self.assemblyCheckBegan = nil; check?(result)
#endif
        }
    }

    private func evaluateModelling(_ params: [String: PresentationJSON], commit: Bool, live: Bool = false) {
        guard let token = modellingToken, let form = modellingInspector, !modellingCommitting else { return }
        if modellingContextNeedsRefresh || modellingContextInspectInFlight {
            if !commit { refreshModellingContext(params, previewAfter: true) }
            return
        }
        if commit {
            guard modellingPreviewQueue.active == nil else { return }
        } else if !modellingPreviewQueue.offer(params) {
            form.invalidatePreview()
            return
        }
        let expected = modellingGeneration
        let expectedForm = modellingFormVersion
        modellingCommitting = commit
        form.setBusy(committing: commit, allowInput: live); updateAvailability()
        session.requestModelling(action: commit ? "commit" : "preview", token: token, params: params) { [weak self, weak form] result in
            guard let self, self.modellingGeneration == expected, self.modellingToken == token else { return }
            self.modellingCommitting = false
            let queuedPreview = self.modellingPreviewQueue.complete()
            guard self.modellingFormVersion == expectedForm else { return }
            switch result {
            case .failure(let error): form?.failed(error); self.failure = error.message
            case .success(let model):
                if commit { self.lastOperationFeedback = model.feedback ?? []; self.clearModelling(); self.rebuildInspector() }
                else { form?.evaluated(model) }
                self.failure = nil
            }
            self.updateAvailability(); self.updateStatus()
            if let next = queuedPreview, !commit { self.evaluateModelling(next, commit: false, live: true) }
#if DEBUG
            let check = self.assemblyCheckEvaluated; self.assemblyCheckEvaluated = nil; check?(result, commit)
#endif
        }
    }

    private func inspectModelling() {
        guard let token = modellingToken, let form = modellingInspector, !modellingInspectPending, !modellingCommitting, modellingPreviewQueue.active == nil,
              !modellingContextInspectInFlight, !modellingContextNeedsRefresh else { return }
        modellingInspectPending = true; form.invalidatePreview()
        let expected = modellingGeneration
        let expectedForm = modellingFormVersion
        let expectedParams = form.params
        session.requestModelling(action: "inspect", token: token) { [weak self, weak form] result in
            guard let self, self.modellingGeneration == expected else { return }
            self.modellingInspectPending = false
            guard self.modellingFormVersion == expectedForm, form?.params == expectedParams else { return }
            switch result {
            case .success(let model): form?.inspected(model)
            case .failure(let error): form?.failed(error)
            }
        }
    }

    private func refreshModellingContext(_ params: [String: PresentationJSON], previewAfter: Bool = false) {
        guard modellingToken != nil, let form = modellingInspector, !modellingCommitting else { return }
        modellingFormVersion &+= 1
        modellingContextNeedsRefresh = true
        pendingModellingContext = params
        previewAfterModellingContext = previewAfter
        modellingPreviewQueue.discardPending()
        form.beginContextRefresh()
        drainModellingContext()
    }

    private func drainModellingContext() {
        guard let token = modellingToken, let form = modellingInspector, let params = pendingModellingContext,
              !modellingContextInspectInFlight, !modellingCommitting else { return }
        pendingModellingContext = nil
        modellingContextInspectInFlight = true
        let expected = modellingGeneration
        let expectedForm = modellingFormVersion
        session.requestModelling(action: "inspect", token: token, params: params) { [weak self, weak form] result in
            guard let self, self.modellingGeneration == expected, self.modellingToken == token else { return }
            self.modellingContextInspectInFlight = false
            if self.modellingFormVersion == expectedForm, form?.params == params {
                switch result {
                case .success(let model):
                    form?.contextRefreshed(model)
                    self.modellingContextNeedsRefresh = false
                    if self.previewAfterModellingContext, let current = form?.params {
                        self.previewAfterModellingContext = false
                        self.evaluateModelling(current, commit: false)
                    }
                case .failure(let error):
                    form?.failed(error); self.previewAfterModellingContext = false
                }
            } else if self.pendingModellingContext == nil {
                // Text typed after the context request remains provisional. The
                // next explicit Preview refreshes those complete current fields;
                // typing itself never sends a new engine request.
                form?.contextRefreshDiscarded(); self.previewAfterModellingContext = false
            }
            if self.pendingModellingContext != nil { self.drainModellingContext() }
        }
    }

    private func cancelModelling() {
        guard !modellingCommitting, modellingEditing else { return }
        let token = modellingToken
        clearModelling(); rebuildInspector(); updateAvailability(); updateStatus()
        session.requestModelling(action: "cancel", token: token) { [weak self] result in
            if case .failure(let error) = result, error.code != "stale_edit" { self?.showFailure(error) }
#if DEBUG
            guard let self else { return }
            let check = self.assemblyCheckCancelled; self.assemblyCheckCancelled = nil; check?(result)
#endif
        }
    }

    private func clearModelling() {
        modellingGeneration &+= 1
        modellingInspector?.endEditing(true); modellingInspector = nil
        previewStale = false
        modellingToken = nil; modellingFeature = nil; modellingContext = nil; modellingStarting = false
        modellingCommitting = false; modellingInspectPending = false
        modellingPreviewQueue.clear()
        modellingFormVersion &+= 1; modellingContextInspectInFlight = false
        pendingModellingContext = nil; modellingContextNeedsRefresh = false; previewAfterModellingContext = false
        table.reloadData()
    }

    private func localDocument(reopen: Bool) {
        guard !numericEditing, !modellingEditing else { return }
        canvas.cancelInput()
        if reopen {
            let alert = UIAlertController(title: ModellingText.text("reopen_local"), message: ModellingText.text("replace_warning"), preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
            alert.addAction(UIAlertAction(title: ModellingText.text("reopen_local"), style: .destructive) { [weak self] _ in self?.performLocalDocument(reopen: true) })
            present(alert, animated: true)
        } else { performLocalDocument(reopen: false) }
    }

    private func performLocalDocument(reopen: Bool) {
        guard !numericEditing, !modellingEditing else { return }
        session.localModellingDocument(reopen: reopen) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.showFailure(error)
            case .success:
                self.localSaveLabel = ModellingText.text(reopen ? "reopened_local" : "saved_local")
                if let label = self.localSaveLabel { self.documentStatus = label }
                self.updateSubtitle()
            }
        }
    }

    private func chooseSketchPlane() {
        guard !numericEditing, state?.sketch == nil, planeDraft == nil, !planeConfirming else { return }
        let chooser = UIAlertController(title: SketchText.text("plane"), message: SketchText.text("choose_plane"), preferredStyle: .actionSheet)
        for name in ["XY", "XZ", "YZ"] + (state?.planes?.map(\.name) ?? []) {
            chooser.addAction(UIAlertAction(title: name, style: .default) { [weak self] _ in self?.previewSketchPlane(name, label: name) })
        }
        if let face = state?.selection.first(where: { $0.type == "face" }), let point = face.point, let body = face.body {
            var plane: [String: Any] = ["face": point, "body": body]
            if let index = face.index { plane["index"] = index }
            if let path = face.path { plane["path"] = path }
            chooser.addAction(UIAlertAction(title: SketchText.text("selected_face"), style: .default) { [weak self] _ in self?.previewSketchPlane(plane, label: SketchText.text("selected_face")) })
        }
        chooser.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        chooser.popoverPresentationController?.sourceView = createSketchButton
        chooser.popoverPresentationController?.sourceRect = createSketchButton.bounds
        present(chooser, animated: true)
    }

    private func previewSketchPlane(_ plane: Any, label: String) {
        canvas.cancelInput(); planeGeneration &+= 1
        let expected = planeGeneration
        planeDraft = (plane, label)
        session.previewSketchPlane(plane) { [weak self] result in
            guard let self, self.planeGeneration == expected, self.planeDraft != nil else { return }
            switch result {
            case .success:
                self.inspectorOpen = true; self.browserOpen = false
                self.rebuildInspector(); self.relayoutPanels()
            case .failure(let error):
                self.planeDraft = nil; self.showFailure(error); self.rebuildInspector()
            }
        }
    }

    private func confirmSketchPlane() {
        guard let draft = planeDraft, !numericEditing, !planeConfirming else { return }
        planeConfirming = true
        rebuildInspector(); updateAvailability()
        let expected = planeGeneration
        session.command("sketch.create", params: ["plane": draft.value]) { [weak self] result in
            guard let self, self.planeGeneration == expected else { return }
            self.planeConfirming = false
            if case .success = result {
                self.planeDraft = nil; self.inspectorOpen = false; self.browserOpen = false
                self.rebuildInspector(); self.relayoutPanels()
            } else {
                self.rebuildInspector(); self.updateAvailability()
            }
        }
    }

    private func cancelSketchPlane() {
        guard !planeConfirming else { return }
        planeGeneration &+= 1; planeDraft = nil
        session.cancelSketchPlane()
        rebuildInspector(); updateAvailability()
    }

    private func sketchSolverText(_ sketch: WorkspaceSketch) -> String {
        if let oversized = sketch.report?.oversized { return SketchText.text("solve_limit") + ": " + String(oversized) }
        if let folded = sketch.report?.folded, !folded.isEmpty { return SketchText.text("orientation_failure") + ": " + folded.joined(separator: ", ") }
        guard sketch.solved else {
            return SketchText.text(sketch.failing.isEmpty ? "solve_failed" : "conflict") + (sketch.failing.isEmpty ? "" : ": " + sketch.failing.joined(separator: ", "))
        }
        return sketch.fully_constrained ? SketchText.text("fully_constrained") : SketchText.text("under_constrained") + " · " + String(sketch.dof) + " " + SketchText.text("dof")
    }

    private func showSketchAnnotations(_ annotations: [SketchAnnotationPresentation]) {
        let retained = Set(annotations.map(\.id))
        for id in Array(annotationLabels.keys) where !retained.contains(id) { annotationLabels.removeValue(forKey: id)?.removeFromSuperview() }
        let formatter = NumberFormatter()
        formatter.locale = .current; formatter.numberStyle = .decimal
        formatter.usesSignificantDigits = true; formatter.maximumSignificantDigits = 9
        for annotation in annotations where annotation.points.count == 2 && annotation.points.allSatisfy(\.isFinite) && annotation.value.isFinite {
            let label = annotationLabels[annotation.id] ?? WorkspaceStyle.label(.caption1)
            if annotationLabels[annotation.id] == nil {
                annotationLabels[annotation.id] = label
                label.isUserInteractionEnabled = false; label.numberOfLines = 1
                label.layer.cornerRadius = 4; label.clipsToBounds = true
                canvas.addSubview(label)
            }
            let number = formatter.string(from: NSNumber(value: annotation.value)) ?? String(annotation.value)
            label.text = " " + (annotation.driven ? "(" + number + ")" : number) + " " + annotation.units + " "
            label.textColor = annotation.selected ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
            label.backgroundColor = annotation.selected ? WorkspaceStyle.accent : WorkspaceStyle.surface.withAlphaComponent(0.92)
            label.sizeToFit()
            label.center = CGPoint(x: annotation.points[0], y: annotation.points[1])
            label.accessibilityIdentifier = "sketch-dimension-" + annotation.id
        }
    }

    private func showConstraintIndicators(_ indicators: [SketchConstraintIndicator]) {
        let retained = Set(indicators.map(\.id))
        for id in Array(constraintLabels.keys) where !retained.contains(id) { constraintLabels.removeValue(forKey: id)?.removeFromSuperview() }
        let symbols = ["horizontal": "H", "vertical": "V", "parallel": "∥", "perpendicular": "⊥", "equal": "=", "tangent": "T", "coincident": "•", "concentric": "◎", "fixed": "F", "midpoint": "M"]
        for indicator in indicators where indicator.points.count == 2 && indicator.points.allSatisfy(\.isFinite) {
            let label = constraintLabels[indicator.id] ?? WorkspaceStyle.label(.caption2)
            if constraintLabels[indicator.id] == nil { constraintLabels[indicator.id] = label; label.isUserInteractionEnabled = false; label.numberOfLines = 1; canvas.addSubview(label) }
            label.text = " " + (symbols[indicator.name.lowercased()] ?? String(indicator.name.prefix(2))) + " "
            label.accessibilityLabel = indicator.name
            label.backgroundColor = indicator.selected ? WorkspaceStyle.accent : WorkspaceStyle.surface.withAlphaComponent(0.92)
            label.textColor = indicator.selected ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
            label.sizeToFit(); label.center = CGPoint(x: indicator.points[0], y: indicator.points[1])
        }
    }

    private func rebuildSketchInspector(_ sketch: WorkspaceSketch, selection: [WorkspaceEntity]) {
        properties.addArrangedSubview(WorkspacePropertyRow(label: WorkspaceStyle.text("sketch"), value: sketch.name))
        properties.addArrangedSubview(WorkspacePropertyRow(label: SketchText.text("solver"), value: sketchSolverText(sketch)))
        properties.addArrangedSubview(WorkspacePropertyRow(label: SketchText.text("profiles"), value: String(sketch.profiles.count)))
        let selectedIDs = Set(selection.compactMap { $0.id?.readout })
        if !selection.isEmpty {
            properties.addArrangedSubview(WorkspacePropertyRow(label: WorkspaceStyle.text("selection"), value: selection.prefix(8).map(\.readout).joined(separator: "\n")))
        }
        if let dimension = sketch.constraints.first(where: { selectedIDs.contains($0.id) && $0.dimension }) {
            properties.addArrangedSubview(WorkspacePropertyRow(label: SketchText.text(dimension.driven ? "reference_dimension" : "driving_dimension"), value: dimension.expression ?? dimension.name))
            if !dimension.driven {
                let edit = WorkspaceButton(SketchText.text("edit_dimension"), prominent: true) { [weak self] in self?.beginNumeric() }
                edit.accessibilityIdentifier = "workspace-edit-dimension"
                properties.addArrangedSubview(edit)
            }
        } else if let dimensions = sketch.eligible_dimensions, !dimensions.isEmpty {
            let dimension = WorkspaceButton(SketchText.text("dimension"), prominent: true, action: {})
            dimension.menu = UIMenu(children: dimensions.map { type in
                UIAction(title: SketchText.text(type)) { [weak self] _ in self?.beginNumeric(dimension: type) }
            })
            dimension.showsMenuAsPrimaryAction = true
            dimension.accessibilityIdentifier = "workspace-create-dimension"
            properties.addArrangedSubview(dimension)
            let place = WorkspaceButton(SketchText.text("place_dimension"), action: {})
            place.menu = UIMenu(children: dimensions.map { type in
                UIAction(title: SketchText.text(type)) { [weak self] _ in
                    guard let self else { return }
                    self.run("tool.set", params: ["tool": "sketch.dimension", "dimension_type": type])
                    self.inspectorOpen = false; self.browserOpen = false; self.relayoutPanels()
                }
            })
            place.showsMenuAsPrimaryAction = true
            properties.addArrangedSubview(place)
        }
        for action in sketch.eligible_constraints ?? [] {
            let mode = action.params["mode"]?.value as? String
            let key = mode ?? String(action.command.split(separator: ".").last ?? "")
            let button = WorkspaceButton(SketchText.text(key)) { [weak self] in self?.run(action.command, params: action.dictionary) }
            button.accessibilityIdentifier = "workspace-constraint-" + key
            properties.addArrangedSubview(button)
        }
        if !selection.isEmpty {
            properties.addArrangedSubview(WorkspaceButton(SketchText.text("delete_selected")) { [weak self] in
                self?.run("sketch.delete", params: ["entities": selection.compactMap { entity -> String? in
                    guard ["sketch_curve", "sketch_point", "sketch_constraint"].contains(entity.type) else { return nil }
                    return entity.id?.readout
                }])
            })
            let curves = selection.filter { $0.type == "sketch_curve" }.compactMap { $0.id?.readout }
            if !curves.isEmpty {
                properties.addArrangedSubview(WorkspaceButton(SketchText.text("construction")) { [weak self] in self?.run("sketch.construction", params: ["curves": curves]) })
            }
            let entities = selection.filter { ["sketch_curve", "sketch_point"].contains($0.type) }.compactMap { $0.id?.readout }
            if !entities.isEmpty {
                let transform = WorkspaceButton(SketchText.text("transform"), action: {})
                transform.menu = UIMenu(children: [(false, false, "translate"), (false, true, "copy"), (true, false, "rotate"), (true, true, "rotate_copy")].map { rotate, copy, label in
                    UIAction(title: SketchText.text(label)) { [weak self] _ in self?.beginTransform(entities: entities, rotate: rotate, copy: copy) }
                })
                transform.showsMenuAsPrimaryAction = true; properties.addArrangedSubview(transform)
                let axes = sketch.curves.filter { $0.type == "line" && !entities.contains($0.id) }
                if !axes.isEmpty {
                    let mirror = WorkspaceButton(SketchText.text("mirror"), action: {})
                    mirror.menu = UIMenu(title: SketchText.text("mirror_axis"), children: axes.map { axis in
                        UIAction(title: axis.id) { [weak self] _ in self?.run("sketch.mirror", params: ["entities": entities, "line": axis.id]) }
                    })
                    mirror.showsMenuAsPrimaryAction = true; properties.addArrangedSubview(mirror)
                }
            }
        }
        let multipleButton = WorkspaceButton(WorkspaceStyle.text("multiple")) { [weak self] in
            guard let self else { return }; self.multiple.toggle()
            self.run("select.multiple", params: ["enabled": self.multiple]); self.rebuildInspector(); self.rebuildMenus()
        }
        multipleButton.active(multiple); properties.addArrangedSubview(multipleButton)
        let disclosure = WorkspaceButton(SketchText.text(showAllConstraints ? "related_constraints" : "all_constraints")) { [weak self] in
            guard let self else { return }; self.showAllConstraints.toggle(); self.rebuildInspector()
        }
        properties.addArrangedSubview(disclosure)
        let constraints = sketch.constraints.filter { showAllConstraints || selectedIDs.contains($0.id) || !$0.entities.filter(selectedIDs.contains).isEmpty }
        for constraint in constraints {
            let detail = constraint.expression.map { " · " + $0 } ?? ""
            let button = WorkspaceButton(constraint.name + " " + constraint.id + detail) { [weak self] in
                self?.canvas.cancelInput()
                self?.session.selectEntity(["type": "sketch_constraint", "id": constraint.id], additive: false)
            }
            button.active(selectedIDs.contains(constraint.id))
            button.accessibilityIdentifier = "workspace-sketch-constraint-" + constraint.id
            properties.addArrangedSubview(button)
        }
        let hint = WorkspaceStyle.label(.footnote, secondary: true); hint.text = SketchText.text("select_hint"); properties.addArrangedSubview(hint)
        if selection.contains(where: { $0.type == "sketch_constraint" }) {
            let placement = WorkspaceStyle.label(.footnote, secondary: true); placement.text = SketchText.text("annotation_hint"); properties.addArrangedSubview(placement)
        }
        properties.addArrangedSubview(WorkspaceButton(SketchText.text("delete_sketch")) { [weak self] in self?.confirmDeleteSketch(id: sketch.id) })
    }

    private func confirmDeleteSketch(id: Any) {
        guard !numericEditing, let context = state else { return }
        session.sketchDependents(id) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.showFailure(error)
            case .success(let dependents):
                guard self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else { self.showFailure(BridgeError(code: "stale_edit", message: WorkspaceStyle.text("invalidated"))); return }
                var message = SketchText.text("delete_sketch_dependents")
                if !dependents.deleted_with_it.isEmpty { message += "\n\n" + SketchText.text("deleted_with_it") + ": " + dependents.deleted_with_it.joined(separator: ", ") }
                if !dependents.would_fail.isEmpty { message += "\n\n" + SketchText.text("would_fail") + ": " + dependents.would_fail.joined(separator: ", ") }
                let alert = UIAlertController(title: SketchText.text("delete_sketch"), message: message, preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
                alert.addAction(UIAlertAction(title: SketchText.text("delete_sketch"), style: .destructive) { [weak self] _ in
                    guard let self, self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else { return }
                    self.run("timeline.delete", params: ["feature": id])
                })
                self.present(alert, animated: true)
            }
        }
    }

    private func beginTransform(entities: [String], rotate: Bool, copy: Bool) {
        guard !numericEditing, let state else { return }
        canvas.cancelInput(); transformGeneration &+= 1
        transformDraft = TransformDraft(entities: entities, rotate: rotate, copy: copy)
        numericContext = (state.document_id, state.geometry_revision)
        properties.arrangedSubviews.forEach { properties.removeArrangedSubview($0); $0.removeFromSuperview() }
        let title = WorkspaceStyle.label(.headline)
        title.text = SketchText.text(rotate ? (copy ? "rotate_copy" : "rotate") : (copy ? "copy" : "translate"))
        properties.addArrangedSubview(title)
        let fields = rotate ? [("center_x", "length", "mm", "0 mm"), ("center_y", "length", "mm", "0 mm"), ("angle", "angle", "deg", "90 deg")] : [("delta_x", "length", "mm", "0 mm"), ("delta_y", "length", "mm", "0 mm")]
        transformEditors = fields.map { name, kind, units, expression in
            let heading = WorkspaceStyle.label(.body); heading.text = SketchText.text(name); properties.addArrangedSubview(heading)
            let field = NumericExpressionEditor(expression: expression, units: units, showsActions: false)
            field.onFocus = { [weak self] in self?.updateAvailability(); DispatchQueue.main.async { [weak self] in self?.revealField() } }
            properties.addArrangedSubview(field)
            return (name, kind, field)
        }
        let message = WorkspaceMessage(); transformMessage = message; properties.addArrangedSubview(message)
        properties.addArrangedSubview(WorkspaceButton(WorkspaceStyle.text("preview")) { [weak self] in self?.evaluateTransform(commit: false) })
        properties.addArrangedSubview(WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true) { [weak self] in self?.evaluateTransform(commit: true) })
        properties.addArrangedSubview(WorkspaceButton(WorkspaceStyle.text("cancel")) { [weak self] in self?.cancelTransform() })
        inspectorOpen = true; browserOpen = false; updateAvailability(); view.setNeedsLayout()
    }

    private func evaluateTransform(commit: Bool) {
        guard let draft = transformDraft, !transformBusy else { return }
        transformBusy = true
        transformEditors.forEach { $0.editor.field.resignFirstResponder(); $0.editor.field.isEnabled = false }
        transformMessage?.show(NativeText.value("ui.ipados.updating"), busy: true)
        let expected = transformGeneration
        let fields = transformEditors.map { (name: $0.name, expression: $0.editor.field.text ?? "", kind: $0.kind) }
        session.evaluateTransformExpressions(fields) { [weak self] result in
            guard let self, self.transformGeneration == expected else { return }
            switch result {
            case .failure(let error): self.finishTransformEvaluation(error: error)
            case .success(let values):
                var params: [String: Any] = ["entities": draft.entities, "copy": draft.copy]
                if draft.rotate {
                    guard let x = values["center_x"], let y = values["center_y"], let angle = values["angle"] else { self.finishTransformEvaluation(error: BridgeError(code: "numeric_validation", message: WorkspaceStyle.text("numeric_required"))); return }
                    params["center"] = [x, y]; params["angle"] = angle * 180 / .pi
                } else {
                    guard let x = values["delta_x"], let y = values["delta_y"] else { self.finishTransformEvaluation(error: BridgeError(code: "numeric_validation", message: WorkspaceStyle.text("numeric_required"))); return }
                    params["translate"] = [x, y]
                }
                self.session.proposeSketchTransform(command: "sketch.move", params: params, commit: commit) { [weak self] result in
                    guard let self, self.transformGeneration == expected else { return }
                    switch result {
                    case .failure(let error): self.finishTransformEvaluation(error: error)
                    case .success:
                        if commit { self.clearTransform(); self.rebuildInspector(); self.updateAvailability() }
                        else { self.finishTransformEvaluation(error: nil) }
                    }
                }
            }
        }
    }

    private func finishTransformEvaluation(error: BridgeError?) {
        transformBusy = false; transformEditors.forEach { $0.editor.field.isEnabled = true }
        transformMessage?.show(error?.message ?? WorkspaceStyle.text("preview_ready"), error: error != nil)
    }
    private func clearTransform() {
        transformGeneration &+= 1; transformDraft = nil; numericContext = nil; transformBusy = false
        transformEditors.forEach { $0.editor.field.resignFirstResponder() }; transformEditors.removeAll(); transformMessage = nil
    }
    private func cancelTransform() {
        clearTransform(); session.command("tool.cancel"); rebuildInspector(); updateAvailability()
    }

    private func beginNumeric(dimension: String? = nil) {
        guard !numericEditing, state?.radius != nil || state?.sketch != nil else { return }
        canvas.cancelInput()
        numericStarting = true
        numericGeneration &+= 1
        let expected = numericGeneration
        updateAvailability()
        session.requestNumeric(action: "begin", dimension: dimension) { [weak self] result in
            guard let self, self.numericGeneration == expected else { return }
            self.numericStarting = false
            switch result {
            case .failure(let error): self.showFailure(error); self.updateAvailability()
            case .success(let value):
                guard let token = value.token, let expression = value.expression else {
                    self.showFailure(BridgeError(code: "numeric", message: WorkspaceStyle.text("invalidated"))); return
                }
                self.numericToken = token
                self.numericContext = (value.workspace.document_id, value.workspace.geometry_revision)
                self.installEditor(expression: expression, units: value.units)
                if let number = value.value { self.editor?.setEvaluation(value: number, units: value.units) }
            }
        }
    }

    private func installEditor(expression: String, units: String) {
        properties.arrangedSubviews.forEach { properties.removeArrangedSubview($0); $0.removeFromSuperview() }
        let editor = NumericExpressionEditor(expression: expression, units: units)
        self.editor = editor
        let heading = WorkspaceStyle.label(.headline)
        heading.text = state?.sketch == nil ? WorkspaceStyle.text("radius") : SketchText.text("dimension")
        heading.accessibilityTraits = .header
        properties.addArrangedSubview(heading)
        editor.onPreview = { [weak self] expression in self?.evaluateNumeric(expression, commit: false) }
        editor.onApply = { [weak self] expression in self?.evaluateNumeric(expression, commit: true) }
        editor.onCancel = { [weak self] in self?.cancelNumeric() }
        editor.onFocus = { [weak self] in
            guard let self else { return }
            self.canvas.cancelInput(); self.updateAvailability()
            DispatchQueue.main.async { [weak self] in self?.revealField() }
        }
        properties.addArrangedSubview(editor)
        inspectorOpen = true; browserOpen = false
        failure = nil
        updateAvailability(); updateStatus(); view.setNeedsLayout()
        // Opening an editor leaves the expression visible but does not summon a
        // keyboard until a person intentionally focuses the field.
    }

    private func evaluateNumeric(_ expression: String, commit: Bool) {
        guard let token = numericToken, let editor, !editor.busy else { return }
        let expected = numericGeneration
        editor.setBusy(true, committing: commit)
        session.requestNumeric(action: commit ? "commit" : "preview", token: token, expression: expression) { [weak self, weak editor] result in
            guard let self, self.numericGeneration == expected else { return }
            editor?.setBusy(false)
            switch result {
            case .failure(let error):
                editor?.message.show(error.message, error: true)
                UIAccessibility.post(notification: .announcement, argument: error.message)
            case .success(let value):
                if commit {
                    self.numericToken = nil; self.numericContext = nil
                    editor?.field.resignFirstResponder(); self.editor = nil
                    self.receive(value.workspace)
                    self.failure = nil; self.updateStatus()
                } else {
                    if let number = value.value { editor?.setEvaluation(value: number, units: value.units) }
                    editor?.message.show(WorkspaceStyle.text("preview_ready"))
                }
            }
            self.updateAvailability()
        }
    }

    private func cancelNumeric() {
        guard let token = numericToken else { return }
        numericGeneration &+= 1
        let expected = numericGeneration
        editor?.field.resignFirstResponder(); editor?.setBusy(true, committing: true)
        session.requestNumeric(action: "cancel", token: token) { [weak self] result in
            guard let self, self.numericGeneration == expected else { return }
            switch result {
            case .failure(let error):
                if error.code == "stale_edit" {
                    self.numericToken = nil; self.numericContext = nil; self.editor = nil
                    self.failure = error.message; self.rebuildInspector(); self.updateStatus()
                    self.session.refreshWorkspace()
                } else { self.editor?.setBusy(false); self.editor?.message.show(error.message, error: true) }
            case .success(let value):
                self.numericToken = nil; self.numericContext = nil; self.editor = nil
                self.failure = nil; self.receive(value.workspace)
            }
            self.updateAvailability()
        }
    }

    private func run(_ command: String, params: [String: Any] = [:]) {
        guard WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: textEntryActive, numericEditActive: numericEditing) else { return }
        guard !modellingEditing || command.hasPrefix("view.") || command.hasPrefix("select.") else { return }
        canvas.cancelInput(); failure = nil; lastOperationFeedback = []
        session.command(command, params: params)
    }
    private func setTool(_ tool: String) {
        guard !numericEditing, state?.tools.first(where: { $0.id == tool })?.enabled == true else { return }
        run("tool.set", params: ["tool": tool])
        if tool == "post_radius" { inspectorOpen = true; browserOpen = false; view.setNeedsLayout() }
    }
    private func toggleBrowser() {
        guard !numericEditing else { return }
        modellingInspector?.endEditing(true)
        canvas.cancelInput(); browserOpen.toggle(); inspectorOpen = false
        relayoutPanels()
    }
    private func toggleInspector() {
        guard !numericEditing else { return }
        modellingInspector?.endEditing(true)
        canvas.cancelInput(); inspectorOpen.toggle(); browserOpen = false
        relayoutPanels()
    }
    private func closePanels() {
        // A compact sheet's outside tap is always consumed, including while an
        // edit requires an explicit Apply/Cancel decision.
        guard !numericEditing else { editor?.message.show(WorkspaceStyle.text("editing_lock")); return }
        canvas.cancelInput(); browserOpen = false; inspectorOpen = false
        relayoutPanels()
    }
    private func relayoutPanels() {
        view.setNeedsLayout(); updateAvailability()
        UIAccessibility.post(notification: .layoutChanged, argument: browserOpen ? browserPanel : (inspectorOpen ? inspectorPanel : browserButton))
    }

    private func updateAvailability() {
        let locked = numericEditing || modellingEditing || !pending.isEmpty
        undoButton.availability(enabled: !locked && state?.can_undo == true, reason: numericEditing || modellingEditing ? WorkspaceStyle.text("editing_lock") : WorkspaceStyle.text("nothing_undo"))
        redoButton.availability(enabled: !locked && state?.can_redo == true, reason: numericEditing || modellingEditing ? WorkspaceStyle.text("editing_lock") : WorkspaceStyle.text("nothing_redo"))
        for button in [browserButton, inspectorButton, viewButton, moreButton] { button.availability(enabled: !numericEditing && !modellingCommitting && pending.isEmpty, reason: numericEditing ? WorkspaceStyle.text("editing_lock") : nil) }
        browserButton.active(browserOpen); inspectorButton.active(inspectorOpen)
        for (button, id) in [(selectButton, "select"), (radiusButton, "post_radius")] {
            let tool = state?.tools.first { $0.id == id }
            button.availability(enabled: !locked && tool?.enabled == true, reason: numericEditing ? WorkspaceStyle.text("editing_lock") : tool?.reason.map(NativeText.value))
            button.active(state?.active_tool == id)
        }
        let inSketch = state?.sketch != nil
        modellingButton.isHidden = inSketch
        modellingButton.availability(enabled: !locked && state?.modelling?.tools.isEmpty == false)
        modellingButton.active(modellingEditing)
        assemblyButton.isHidden = inSketch
        assemblyButton.availability(enabled: !locked && state?.assembly?.tools.isEmpty == false)
        createSketchButton.isHidden = inSketch
        createSketchButton.availability(enabled: !locked && planeDraft == nil)
        radiusButton.isHidden = inSketch
        sketchToolsButton.isHidden = !inSketch
        finishSketchButton.isHidden = !inSketch
        finishChainButton.isHidden = !inSketch || !(state?.active_tool == "sketch.line" || state?.active_tool == "sketch.spline.fit_point")
        for button in [sketchToolsButton, finishSketchButton, finishChainButton] { button.availability(enabled: !locked) }
        cancelToolButton.isHidden = numericEditing || !(inSketch || state?.active_tool == "post_radius")
        table.allowsSelection = !numericEditing && !modellingCommitting && pending.isEmpty
        let modal = layout?.compact == true && (browserOpen || inspectorOpen)
        canvas.isUserInteractionEnabled = !numericEditing && WorkspaceFocusPolicy.viewportInputEnabled(modalPresented: modal, textEntryActive: textEntryActive)
        canvas.accessibilityElementsHidden = modal || numericEditing
    }

    private func rebuildMenus() {
        assemblyButton.menu = UIMenu(children: (state?.assembly?.tools ?? []).map { tool in
            UIAction(title: ModellingText.label(tool.label)) { [weak self] _ in self?.beginModelling(operation: tool.id) }
        })
        assemblyButton.showsMenuAsPrimaryAction = true
        modellingButton.menu = UIMenu(children: (state?.modelling?.tools ?? []).map { tool in
            UIAction(title: ModellingText.label(tool.label)) { [weak self] _ in self?.beginModelling(operation: tool.id) }
        })
        modellingButton.showsMenuAsPrimaryAction = true
        var views = ["front", "back", "left", "right", "top", "bottom", "isometric"].map { name in
            UIAction(title: NativeText.value("ui.ipados." + name)) { [weak self] _ in self?.run("view." + name) }
        }
        views.insert(UIAction(title: WorkspaceStyle.text("fit")) { [weak self] _ in self?.run("view.fit") }, at: 0)
        views.append(UIAction(title: NativeText.value("ui.ipados.perspective"), state: perspective ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.perspective.toggle(); self.run("view.perspective", params: ["enabled": self.perspective]); self.rebuildMenus()
        })
        views.append(UIAction(title: NativeText.value("ui.ipados.exit_planar")) { [weak self] _ in self?.run("view.exit_planar") })
        viewButton.menu = UIMenu(children: views); viewButton.showsMenuAsPrimaryAction = true
        let filters = ["occurrence", "auto", "body", "face", "edge", "vertex"].map { name in
            UIAction(title: NativeText.value(name == "occurrence" ? "ui.ipados.assembly.occurrence" : "ui.ipados." + name), state: filter == name ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.filter = name; self.run("select.filter", params: ["filter": name]); self.rebuildMenus()
            }
        }
        let multiple = UIAction(title: WorkspaceStyle.text("multiple"), state: multiple ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.multiple.toggle(); self.run("select.multiple", params: ["enabled": self.multiple]); self.rebuildMenus()
        }
        let side = UIAction(title: WorkspaceStyle.text(railRight ? "rail_left" : "rail_right")) { [weak self] _ in
            guard let self else { return }
            self.canvas.cancelInput(); self.railRight.toggle()
            UserDefaults.standard.set(self.railRight, forKey: "SolidCraft.workspace.railRight")
            self.view.setNeedsLayout(); self.rebuildMenus()
        }
        let appearances = ["system", "light", "dark"].enumerated().map { index, key in
            UIAction(title: WorkspaceStyle.text(key), state: appearance == index ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.appearance = index
                UserDefaults.standard.set(index, forKey: "SolidCraft.workspace.appearance")
                self.applyAppearance(); self.rebuildMenus()
            }
        }
        let historyGestures = UIAction(title: WorkspaceStyle.text("history_gestures"), state: canvas.historyGesturesEnabled ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.canvas.cancelInput()
            self.canvas.historyGesturesEnabled.toggle()
            UserDefaults.standard.set(self.canvas.historyGesturesEnabled, forKey: "SolidCraft.workspace.historyGestures")
            self.rebuildMenus()
        }
        var moreActions: [UIMenuElement] = [
            UIAction(title: ModellingText.text("save_local"), attributes: numericEditing || modellingEditing ? .disabled : []) { [weak self] _ in self?.localDocument(reopen: false) },
            UIAction(title: ModellingText.text("reopen_local"), attributes: numericEditing || modellingEditing ? .disabled : []) { [weak self] _ in self?.localDocument(reopen: true) },
            UIMenu(title: WorkspaceStyle.text("filter"), children: filters), multiple,
            UIAction(title: WorkspaceStyle.text("clear")) { [weak self] _ in self?.run("select.clear") },
            historyGestures, side, UIMenu(title: WorkspaceStyle.text("appearance"), children: appearances)
        ]
        if state?.sketch != nil {
            moreActions.insert(UIAction(title: SketchText.text("snapping"), state: sketchInput?.snapping != false ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.run("tool.snapping", params: ["enabled": self.sketchInput?.snapping == false])
            }, at: 0)
            moreActions.insert(UIAction(title: SketchText.text("auto_constraints"), state: sketchInput?.auto_constraints != false ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.run("tool.auto_constraints", params: ["enabled": self.sketchInput?.auto_constraints == false])
            }, at: 1)
        }
        moreButton.menu = UIMenu(children: moreActions)
        moreButton.showsMenuAsPrimaryAction = true
        let tools = [("sketch.line", "line"), ("sketch.rectangle.two_point", "rectangle"), ("sketch.circle.center", "circle"),
                     ("sketch.arc.three_point", "arc"), ("sketch.spline.fit_point", "spline"), ("sketch.move", "move"),
                     ("sketch.trim", "trim"), ("sketch.extend", "extend")]
        sketchToolsButton.menu = UIMenu(children: tools.compactMap { id, label in
            guard state?.tools.contains(where: { $0.id == id && $0.enabled }) == true else { return nil }
            return UIAction(title: SketchText.text(label), state: state?.active_tool == id ? .on : .off) { [weak self] _ in self?.setTool(id) }
        })
        sketchToolsButton.showsMenuAsPrimaryAction = true
        sketchToolsButton.active(state?.active_tool.hasPrefix("sketch.") == true)
    }
    private func applyAppearance() { overrideUserInterfaceStyle = appearance == 1 ? .light : (appearance == 2 ? .dark : .unspecified) }

    private func updateStatus() {
        if let failure { status.textColor = WorkspaceStyle.error; status.text = failure; return }
        status.textColor = WorkspaceStyle.secondary
        if modellingEditing {
            if previewStale { status.text = ModellingText.text("preview_stale"); return }
            status.text = ModellingText.text("draft_hint") + "\n" + ModellingText.text("navigation_hint")
            return
        }
        if state?.sketch == nil, state?.modelling?.timeline.contains(where: { $0.error != nil || $0.stale == true }) == true {
            status.textColor = WorkspaceStyle.error
            status.text = ModellingText.text("repair_required")
            return
        }
        if !lastOperationFeedback.isEmpty {
            status.text = lastOperationFeedback.map(ModellingText.label).joined(separator: "\n")
            return
        }
        if let assembly = state?.assembly, state?.sketch == nil, !assembly.diagnostics.isEmpty {
            status.text = assembly.diagnostics.prefix(2).map(ModellingText.label).joined(separator: "\n")
            return
        }
        if !pending.isEmpty { status.text = pending; return }
        if let sketch = state?.sketch {
            let prompt = sketchInput.map { SketchText.text($0.prompt) } ?? SketchText.text("select_hint")
            let snap = sketchInput?.snap.map { SketchText.text($0.persistent ? "persistent_snap" : "temporary_snap") + ": " + SketchText.text($0.kind) } ?? ""
            var measurement = ""
            if let input = sketchInput, let feedback = input.feedback {
                let formatter = NumberFormatter(); formatter.locale = .current; formatter.numberStyle = .decimal; formatter.maximumFractionDigits = 3
                let number: (Double) -> String = { formatter.string(from: NSNumber(value: $0)) ?? String($0) }
                if input.tool == "sketch.rectangle.two_point" { measurement = number(feedback.width) + " × " + number(feedback.height) + " mm" }
                else if input.tool == "sketch.circle.center" { measurement = SketchText.text("radius") + " " + number(feedback.radius) + " mm" }
                else if input.tool == "sketch.line" { measurement = number(feedback.radius) + " mm" }
            }
            status.text = sketch.name + " · " + sketchSolverText(sketch) + "\n" + [prompt, measurement, snap].filter { !$0.isEmpty }.joined(separator: " · ")
            return
        }
        let summary = state?.selection.isEmpty == false ? state?.selection.prefix(2).map(\.readout).joined(separator: ", ") : NativeText.value("ui.ipados.no_selection")
        status.text = WorkspaceStyle.text(numericEditing ? "editing" : (state?.active_tool == "post_radius" ? "radius_hint" : "select_hint")) + "\n" + (summary ?? "")
    }
    private func showFailure(_ error: BridgeError) {
        failure = error.message; pending = ""
        if state == nil { browserMessage.text = error.message + "\n" + WorkspaceStyle.text("retry") }
        updateStatus(); updateAvailability()
        UIAccessibility.post(notification: .announcement, argument: error.message)
    }
    @objc private func retryBrowser() { session.refreshWorkspace() }

    override var canBecomeFirstResponder: Bool { true }
    override var keyCommands: [UIKeyCommand]? {
        var commands = [UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(cancelKey))]
        if WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: textEntryActive, numericEditActive: numericEditing || modellingEditing) {
            if state?.can_undo == true { commands.append(UIKeyCommand(input: "z", modifierFlags: .command, action: #selector(undoKey))) }
            if state?.can_redo == true { commands.append(UIKeyCommand(input: "z", modifierFlags: [.command, .shift], action: #selector(redoKey))) }
        }
        return commands
    }
    @objc private func undoKey() { run("edit.undo") }
    @objc private func redoKey() { run("edit.redo") }
    @objc private func cancelKey() {
        if modellingEditing { cancelModelling() }
        else if transformDraft != nil { cancelTransform() }
        else if numericEditing { cancelNumeric() }
        else if planeDraft != nil { cancelSketchPlane() }
        else if state?.sketch != nil { run("tool.cancel") }
        else { canvas.cancelInput(); closePanels() }
    }

    @objc private func keyboardChanged(_ notification: Notification) {
        if notification.name == UIResponder.keyboardWillHideNotification { keyboardTop = nil }
        else if let frame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect,
                let window = view.window {
            let local = view.convert(window.convert(frame, from: nil), from: window)
            keyboardTop = local.intersects(view.bounds) ? local.minY : nil
        }
        view.setNeedsLayout(); view.layoutIfNeeded(); revealField(); updateAvailability()
    }
    private func revealField() {
        guard let field = editor?.field ?? transformEditors.first(where: { $0.editor.field.isFirstResponder })?.editor.field ?? modellingInspector?.focusedField else { return }
        inspectorScroll.scrollRectToVisible(field.convert(field.bounds, to: inspectorScroll).insetBy(dx: 0, dy: -16), animated: false)
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        let safe = view.safeAreaLayoutGuide.layoutFrame
        workspace.frame = CGRect(x: safe.minX, y: safe.minY, width: safe.width, height: max(0, min(safe.maxY, keyboardTop ?? safe.maxY) - safe.minY))
        let largeText = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
        let compactWidth = workspace.bounds.width < (largeText ? 1120 : 1000)
        let titleHeight = ceil(titleLabel.font.lineHeight) + 2
        let subtitleHeight = 2 * ceil(subtitle.font.lineHeight) + 2
        let actionsHeight = max(44, ceil(UIFont.preferredFont(forTextStyle: .body).lineHeight) + 20)
        let headingHeight = compactWidth ? titleHeight + subtitleHeight + actionsHeight + 12 : max(titleHeight + subtitleHeight + 8, actionsHeight + 8)
        let resolved = WorkspaceLayout.resolve(size: workspace.bounds.size, browser: browserOpen, inspector: inspectorOpen,
                                               railRight: railRight, largeText: largeText,
                                               minimumHeaderHeight: headingHeight, minimumStatusHeight: 2 * ceil(status.font.lineHeight) + 4)
        if canvas.frame != resolved.canvas { canvas.cancelInput() }
        layout = resolved
        top.frame = resolved.topBar; status.frame = resolved.status; canvas.frame = resolved.canvas
        railScroll.frame = resolved.rail
        let compact = resolved.compact
        let titleWidth = compact ? top.bounds.width - 24 : max(140, top.bounds.width - min(520, topActions.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize).width) - 36)
        titleLabel.frame = CGRect(x: 12, y: 4, width: max(0, titleWidth), height: titleHeight)
        subtitle.frame = CGRect(x: 12, y: titleHeight + 4, width: max(0, titleWidth), height: subtitleHeight)
        let controlsY = titleHeight + subtitleHeight + 8
        topScroll.frame = compact ? CGRect(x: 8, y: controlsY, width: max(0, top.bounds.width - 16), height: max(44, top.bounds.height - controlsY)) : CGRect(x: titleWidth + 24, y: 4, width: max(0, top.bounds.width - titleWidth - 32), height: max(44, top.bounds.height - 8))
        browserPanel.isHidden = resolved.browser == nil
        inspectorPanel.isHidden = resolved.inspector == nil
        if let frame = resolved.browser { browserPanel.frame = frame }
        if let frame = resolved.inspector { inspectorPanel.frame = frame }
        scrim.frame = CGRect(x: 0, y: resolved.rail.minY, width: workspace.bounds.width, height: resolved.canvas.height)
        scrim.isHidden = !compact || resolved.browser == nil && resolved.inspector == nil
        browserPanel.layoutIfNeeded(); inspectorPanel.layoutIfNeeded()
        table.frame = browserPanel.content.bounds
        inspectorScroll.frame = inspectorPanel.content.bounds
        browserPanel.accessibilityViewIsModal = compact && browserOpen
        inspectorPanel.accessibilityViewIsModal = compact && inspectorOpen
        workspace.accessibilityElements = [top, railScroll, browserPanel, canvas, inspectorPanel, status]
        updateAvailability()
        canvas.updateDrawableSize()
    }
    func activate() { loadViewIfNeeded(); active = true; session.activate(); updateSurface() }
    func suspend() {
        guard active else { return }
        active = false
        // Suspension synchronously invalidates the Rust token. Drop local draft
        // ownership now and ignore queued callbacks, including a pending Begin.
        numericGeneration &+= 1
        planeGeneration &+= 1; planeDraft = nil; planeConfirming = false
        clearModelling()
        clearTransform()
        numericToken = nil; numericContext = nil; numericStarting = false
        editor?.field.resignFirstResponder(); editor = nil
        rebuildInspector(); updateAvailability()
        canvas.cancelInput(); session.suspend()
    }
    func shutdown() {
        generation &+= 1; numericGeneration &+= 1; active = false
        canvas.cancelInput(); session.stop()
    }
    private func updateSurface() {
        guard active, let layer = canvas.metalLayer, let size = canvas.pixelSize else { return }
        session.present(layer: layer, width: size.width, height: size.height, scale: Double(canvas.contentScaleFactor))
    }
}

#if DEBUG
/// Opt-in device evidence driver. It uses the ordinary controller forms and their
/// Preview/Apply/Cancel callbacks. It never synthesizes a hardware Pencil event.
private extension FoundationController {
    func startAssemblyCheck() {
        guard !assemblyCheckRunning else { return }
        assemblyCheckRunning = true
        do {
            let documents = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
            assemblyCheckLog = documents.appendingPathComponent("step8-assembly-check.log")
            assemblyCheckFile = documents.appendingPathComponent("Step8-Assembly-Runtime-\(UUID().uuidString).solidcraft")
            try Data().write(to: documents.appendingPathComponent("step8-assembly-check.log"), options: .atomic)
        } catch { assemblyCheckFail("Cannot create dedicated runtime evidence files: \(error)"); return }
        signal(SIGUSR1, SIG_IGN)
        let source = DispatchSource.makeSignalSource(signal: SIGUSR1, queue: .main)
        source.setEventHandler { [weak self] in
            guard let self, self.assemblyCheckRunning, let next = self.assemblyCheckContinue else { return }
            self.assemblyCheckContinue = nil
            self.assemblyCheckWrite("CONTINUE SIGUSR1")
            self.assemblyCheckArmWatchdog(); next()
        }
        assemblyCheckSignal = source; source.resume()
        assemblyCheckWrite("START actual UIKit controller / native CAD / Metal; synthetic controller actions, no physical Pencil claim")
        assemblyCheckWrite("LOG \(assemblyCheckLog?.path ?? "unavailable")")
        assemblyCheckArmWatchdog()
        // Only an explicit DEBUG launch flag replaces the disposable launch
        // fixture. The save path is unique and cannot overwrite a user's design.
        let commands: [(String, [String: Any])] = [
            ("file.new", ["name": "Step 8 native assembly runtime"]),
            ("component.create", ["name": "Runtime frame", "parent": 0, "activate": true, "grounded": true]),
            ("solid.box", ["length": 20, "width": 20, "height": 4, "corner": [-10, -10, -4], "body_name": "Frame plate"]),
            ("component.activate", ["component": 0]),
            ("component.create", ["name": "Runtime arm", "parent": 0, "activate": true, "grounded": false]),
            ("solid.box", ["length": 100, "width": 6, "height": 4, "corner": [0, -3, 0], "body_name": "Hinge arm"]),
            ("component.activate", ["component": 0]),
            ("component.create", ["name": "Runtime slider", "parent": 0, "activate": true, "grounded": false]),
            ("solid.box", ["length": 12, "width": 10, "height": 8, "corner": [0, -35, 0], "body_name": "Slide block"]),
            ("component.activate", ["component": 0]),
            ("solid.box", ["length": 110, "width": 4, "height": 3, "corner": [-5, -32, -3], "body_name": "Fixed guide"])
        ]
        assemblyCheckCommands(commands, index: 0) { [weak self] in self?.assemblyCheckStep(0) }
    }

    func assemblyCheckWrite(_ message: String) {
        let line = "SolidCraft Step 8 assembly check \(message)"
        NSLog("%@", line)
        guard let url = assemblyCheckLog, let bytes = (line + "\n").data(using: .utf8) else { return }
        do {
            let file = try FileHandle(forWritingTo: url)
            defer { try? file.close() }
            try file.seekToEnd(); try file.write(contentsOf: bytes)
        } catch { NSLog("SolidCraft Step 8 evidence log error: %@", String(describing: error)) }
    }

    func assemblyCheckArmWatchdog() {
        assemblyCheckTicket &+= 1
        let ticket = assemblyCheckTicket
        // A failure deadline, never a sequencing delay. CAD callbacks and exact
        // presented-frame revisions advance every stage. Pauses disable it.
        DispatchQueue.main.asyncAfter(deadline: .now() + 45) { [weak self] in
            guard let self, self.assemblyCheckRunning, self.assemblyCheckTicket == ticket,
                  self.assemblyCheckContinue == nil else { return }
            self.assemblyCheckFail("Timed out waiting for a native callback or matching presented frame")
        }
    }

    func assemblyCheckFail(_ message: String) {
        assemblyCheckWrite("FAIL \(message)")
        assemblyCheckRunning = false; assemblyCheckTicket &+= 1
        assemblyCheckWorkspace = nil; assemblyCheckBegan = nil; assemblyCheckEvaluated = nil
        assemblyCheckCancelled = nil; assemblyCheckFrame = nil; assemblyCheckContinue = nil
        assemblyCheckSignal?.cancel(); assemblyCheckSignal = nil
        showFailure(BridgeError(code: "assembly_check", message: message))
    }

    func assemblyCheckCommands(_ commands: [(String, [String: Any])], index: Int, next: @escaping () -> Void) {
        guard assemblyCheckRunning else { return }
        guard index < commands.count else { next(); return }
        let (command, params) = commands[index]
        assemblyCheckCommand(command, params: params) { [weak self] in self?.assemblyCheckCommands(commands, index: index + 1, next: next) }
    }

    func assemblyCheckCommand(_ command: String, params: [String: Any] = [:], next: @escaping () -> Void) {
        guard assemblyCheckRunning else { return }
        assemblyCheckArmWatchdog()
        // command() refreshes the authoritative workspace. Wait for that report
        // as well as command completion before beginning the next UI operation.
        var completed = false
        assemblyCheckWorkspace = { [weak self] _ in
            guard let self, completed, self.assemblyCheckRunning else { return }
            self.assemblyCheckWorkspace = nil; self.assemblyCheckWrite("PASS command \(command)"); next()
        }
        session.command(command, params: params) { [weak self] result in
            guard let self, self.assemblyCheckRunning else { return }
            switch result {
            case .failure(let error): self.assemblyCheckFail("\(command): \(error)")
            case .success: completed = true
            }
        }
    }

    func assemblyCheckBegin(_ operation: String, params: [String: Any], next: @escaping (ModellingState) -> Void) {
        guard assemblyCheckRunning else { return }
        assemblyCheckArmWatchdog()
        do {
            let data = try JSONSerialization.data(withJSONObject: params)
            let typed = try JSONDecoder().decode([String: PresentationJSON].self, from: data)
            assemblyCheckBegan = { [weak self] result in
                guard let self, self.assemblyCheckRunning else { return }
                switch result {
                case .failure(let error): self.assemblyCheckFail("begin \(operation): \(error)")
                case .success(let model):
                    self.receive(model.workspace)
                    guard self.modellingInspector != nil, model.token != nil else { self.assemblyCheckFail("No production modelling form/token for \(operation)"); return }
                    self.assemblyCheckWrite("PASS production form \(operation) token=\(model.token ?? 0)"); next(model)
                }
            }
            beginModelling(operation: operation, initialParams: typed)
        } catch { assemblyCheckFail("Invalid runtime fixture JSON: \(error)") }
    }

    func assemblyCheckEvaluate(commit: Bool, next: @escaping (ModellingState) -> Void) {
        guard assemblyCheckRunning, let form = modellingInspector else { assemblyCheckFail("Missing production form before evaluation"); return }
        assemblyCheckArmWatchdog()
        assemblyCheckEvaluated = { [weak self] result, actualCommit in
            guard let self, self.assemblyCheckRunning else { return }
            guard actualCommit == commit else { self.assemblyCheckFail("Unexpected evaluation callback"); return }
            switch result {
            case .failure(let error): self.assemblyCheckFail("\(commit ? "Apply" : "Preview"): \(error)")
            case .success(let model):
                self.receive(model.workspace)
                if !commit && model.valid != true { self.assemblyCheckFail("Preview was not validated by engine"); return }
                self.assemblyCheckWrite("PASS production \(commit ? "Apply" : "Preview") revision=\(model.workspace.geometry_revision) feedback=\(model.feedback ?? [])")
                next(model)
            }
        }
        // Invoke the same closures used by the native buttons. The form retains
        // expressions and the shared Rust session owns all speculative geometry.
        if commit { form.onApply?(form.params) } else { form.onPreview?(form.params) }
    }

    func assemblyCheckModel(_ operation: String, params: [String: Any], next: @escaping (ModellingState) -> Void) {
        assemblyCheckBegin(operation, params: params) { [weak self] _ in
            self?.assemblyCheckEvaluate(commit: false) { [weak self] _ in self?.assemblyCheckEvaluate(commit: true, next: next) }
        }
    }

    func assemblyCheckWaitForFrame(_ model: ModellingState, preview: Bool, stage: String, next: @escaping () -> Void) {
        assemblyCheckArmWatchdog()
        assemblyCheckFrame = (model.workspace.document_id, model.workspace.geometry_revision, preview, stage, next)
        view.setNeedsLayout(); view.layoutIfNeeded(); updateSurface()
    }

    func assemblyCheckPresented(document: String?, revision: UInt64?, preview: Bool) {
        guard assemblyCheckRunning, let waiting = assemblyCheckFrame,
              document == waiting.document, revision == waiting.revision, preview == waiting.preview else { return }
        assemblyCheckFrame = nil
        assemblyCheckWrite("READY \(waiting.stage) document=\(waiting.document) geometry_revision=\(waiting.revision) preview=\(preview) pixels=\(String(describing: canvas.pixelSize))")
        assemblyCheckWrite("LAYOUT window.frame=\(String(describing: view.window?.frame)) window.bounds=\(String(describing: view.window?.bounds)) window.transform=\(String(describing: view.window?.transform)) scene=\(String(describing: view.window?.windowScene?.coordinateSpace.bounds)) view.frame=\(view.frame) view.bounds=\(view.bounds) workspace=\(workspace.bounds) safeArea=\(view.safeAreaInsets) canvas=\(canvas.frame) scale=\(canvas.contentScaleFactor) nativeScale=\(String(describing: view.window?.screen.nativeScale)) orientation=\(String(describing: view.window?.windowScene?.interfaceOrientation))")
        if ProcessInfo.processInfo.arguments.contains("--assembly-check-pause") {
            assemblyCheckTicket &+= 1; assemblyCheckContinue = waiting.next
            assemblyCheckWrite("PAUSED \(waiting.stage); capture genuine device framebuffer, then send SIGUSR1 to pid \(ProcessInfo.processInfo.processIdentifier)")
        } else { waiting.next() }
    }

    func assemblyCheckAssertValue(_ model: ModellingState, expected: Double, units: String, moved: Bool? = nil) -> Bool {
        guard case .object(let reply) = model.reply, case .array(let values) = reply["values"],
              let first = values.first, case .object(let value) = first,
              let actual = value["value"]?.value as? NSNumber, value["unit"] == .string(units),
              abs(actual.doubleValue - expected) < 1e-8,
              moved == nil || reply["moved"] == .bool(moved ?? false) else {
            assemblyCheckFail("Native drive value/moved mismatch; expected \(expected) \(units), reply=\(String(describing: model.reply))"); return false
        }
        assemblyCheckWrite("PASS native actual=\(actual) \(units) expected=\(expected) moved=\(String(describing: reply["moved"])) limits=\(String(describing: value["limits"]))")
        return true
    }

    func assemblyCheckStep(_ step: Int) {
        guard assemblyCheckRunning else { return }
        assemblyCheckArmWatchdog()
        let advance: () -> Void = { [weak self] in self?.assemblyCheckStep(step + 1) }
        switch step {
        case 0:
            guard let rows = state?.rows,
                  let frame = rows.first(where: { $0.kind == "occurrence" && $0.label == "Runtime frame:1" })?.assembly?.path,
                  let arm = rows.first(where: { $0.kind == "occurrence" && $0.label == "Runtime arm:1" })?.assembly?.path,
                  let slide = rows.first(where: { $0.kind == "occurrence" && $0.label == "Runtime slider:1" })?.assembly?.path,
                  frame.count == 1, arm.count == 1, slide.count == 1,
                  let frameID = frame.first, let armID = arm.first, let slideID = slide.first else { assemblyCheckFail("Missing exact fixture occurrence paths"); return }
            assemblyCheckFrameID = frameID; assemblyCheckArmID = armID; assemblyCheckSlideID = slideID
            assemblyCheckWrite("PASS exact native paths frame=\(frame) arm=\(arm) slider=\(slide)")
            assemblyCheckCommand("select.entity", params: ["entity": ["type": "occurrence", "path": arm], "additive": false], next: advance)
        case 1, 2:
            let slider = step == 2
            let axis = slider ? [1, 0, 0] : [0, 0, 1]
            var params: [String: Any] = [
                "type": slider ? "slider" : "revolute", "name": slider ? "Runtime slide" : "Runtime hinge", "flip": true,
                "a_mode": "frame", "a_placement": ["occurrence": assemblyCheckFrameID, "path": [assemblyCheckFrameID]],
                "b_mode": "frame", "b_placement": ["occurrence": slider ? assemblyCheckSlideID : assemblyCheckArmID, "path": [slider ? assemblyCheckSlideID : assemblyCheckArmID]],
                "a_point_x": "0 mm", "a_point_y": slider ? "-30 mm" : "0 mm", "a_point_z": "0 mm", "a_z": axis,
                "b_point_x": "0 mm", "b_point_y": slider ? "-30 mm" : "0 mm", "b_point_z": "0 mm", "b_z": axis
            ]
            if slider { params["limits_enabled"] = true; params["lo_0"] = "0 mm"; params["hi_0"] = "50 mm" }
            assemblyCheckModel("joint.create", params: params) { [weak self] model in
                guard let self, case .object(let reply) = model.reply, let number = reply["joint"]?.value as? NSNumber else { self?.assemblyCheckFail("Created joint has no native identity"); return }
                if slider { self.assemblyCheckSliderID = number.uint64Value } else { self.assemblyCheckHingeID = number.uint64Value }
                self.assemblyCheckWrite("PASS created native joint id=\(number) type=\(slider ? "slider" : "revolute")"); advance()
            }
        case 3:
            session.command("view.top") { [weak self] result in
                switch result { case .failure(let error): self?.assemblyCheckFail(error.description)
                case .success: self?.session.command("view.fit") { result in
                    switch result { case .failure(let error): self?.assemblyCheckFail(error.description); case .success: advance() }
                } }
            }
        case 4:
            // Joint creation can clear native selection. Select the exact path
            // at this interaction boundary, as an ordinary browser row does.
            assemblyCheckCommand("select.entity", params: ["entity": ["type": "occurrence", "path": [assemblyCheckArmID]], "additive": false]) { [weak self] in
                guard let self else { return }
                self.assemblyCheckBegin("joint.drive", params: ["joint": self.assemblyCheckHingeID, "value": "90 deg"]) { [weak self] model in
                    guard let self, model.workspace.selection.contains(where: { $0.type == "occurrence" && $0.path == [self.assemblyCheckArmID] }) else { self?.assemblyCheckFail("Exact occurrence selection was lost"); return }
                    self.assemblyCheckEvaluate(commit: false) { [weak self] preview in
                        guard let self, self.assemblyCheckAssertValue(preview, expected: 90, units: "deg", moved: true) else { return }
                        self.assemblyCheckWaitForFrame(preview, preview: true, stage: "hinge-preview-90", next: advance)
                    }
                }
            }
        case 5:
            assemblyCheckCancelled = { [weak self] result in
                guard let self else { return }
                switch result {
                case .failure(let error): self.assemblyCheckFail("Cancel: \(error)")
                case .success(let model):
                    self.receive(model.workspace)
                    self.assemblyCheckBegin("joint.drive", params: ["joint": self.assemblyCheckHingeID]) { [weak self] inspected in
                        guard let self, let value = inspected.fields?.first(where: { $0.key == "value" })?.value, abs(value) < 1e-8 else { self?.assemblyCheckFail("Cancel changed the authoritative hinge value"); return }
                        self.assemblyCheckWrite("PASS production Cancel restored authoritative hinge value=\(value) deg")
                        self.assemblyCheckCancelled = { [weak self] result in
                            guard let self else { return }
                            switch result {
                            case .failure(let error): self.assemblyCheckFail(error.description)
                            case .success(let cancelled):
                                self.receive(cancelled.workspace)
                                self.assemblyCheckWaitForFrame(cancelled, preview: false, stage: "hinge-cancelled-zero", next: advance)
                            }
                        }
                        self.modellingInspector?.onCancel?()
                    }
                }
            }
            modellingInspector?.onCancel?()
        case 6, 7:
            let angle = step == 6 ? 90.0 : 30.0
            assemblyCheckModel("joint.drive", params: ["joint": assemblyCheckHingeID, "value": "\(angle) deg"]) { [weak self] model in
                guard let self, self.assemblyCheckAssertValue(model, expected: angle, units: "deg", moved: true) else { return }
                self.assemblyCheckWaitForFrame(model, preview: false, stage: "hinge-committed-\(Int(angle))", next: advance)
            }
        case 8:
            assemblyCheckCommand("edit.undo") { [weak self] in
                guard let self else { return }
                self.assemblyCheckBegin("joint.drive", params: ["joint": self.assemblyCheckHingeID]) { [weak self] model in
                    guard let self, let value = model.fields?.first(where: { $0.key == "value" })?.value, abs(value - 90) < 1e-8 else { self?.assemblyCheckFail("Undo did not restore authoritative hinge90"); return }
                    self.assemblyCheckWrite("PASS Undo restored authoritative hinge=\(value) deg")
                    self.assemblyCheckCancelled = { [weak self] result in
                        switch result { case .failure(let error): self?.assemblyCheckFail(error.description); case .success: advance() }
                    }
                    self.modellingInspector?.onCancel?()
                }
            }
        case 9: assemblyCheckCommand("edit.redo", next: advance)
        case 10:
            assemblyCheckBegin("joint.drive", params: ["joint": assemblyCheckHingeID]) { [weak self] model in
                guard let self, let field = model.fields?.first(where: { $0.key == "value" }), field.units == "deg", let value = field.value, abs(value - 30) < 1e-8 else { self?.assemblyCheckFail("Undo/redo did not restore hinge30 in production inspector"); return }
                self.assemblyCheckWrite("PASS undo/redo restored native hinge identity=\(self.assemblyCheckHingeID) value=\(value) deg")
                self.assemblyCheckCancelled = { [weak self] result in
                    switch result { case .failure(let error): self?.assemblyCheckFail(error.description); case .success: advance() }
                }
                self.modellingInspector?.onCancel?()
            }
        case 11:
            assemblyCheckModel("joint.drive", params: ["joint": assemblyCheckSliderID, "value": "75 mm"]) { [weak self] model in
                guard let self, self.assemblyCheckAssertValue(model, expected: 50, units: "mm", moved: true) else { return }
                self.assemblyCheckWaitForFrame(model, preview: false, stage: "slider-request75-actual50", next: advance)
            }
        case 12:
            guard let file = assemblyCheckFile else { assemblyCheckFail("Missing dedicated native file path"); return }
            assemblyCheckWrite("NATIVE_FILE \(file.path)")
            assemblyCheckCommand("file.save_as", params: ["path": file.path], next: advance)
        case 13:
            assemblyCheckModel("joint.drive", params: ["joint": assemblyCheckSliderID, "value": "20 mm"]) { [weak self] model in
                guard let self, self.assemblyCheckAssertValue(model, expected: 20, units: "mm", moved: true) else { return }; advance()
            }
        case 14:
            guard let file = assemblyCheckFile else { assemblyCheckFail("Missing dedicated native file path"); return }
            assemblyCheckCommand("doc.open", params: ["path": file.path], next: advance)
        case 15:
            assemblyCheckBegin("joint.drive", params: ["joint": assemblyCheckSliderID]) { [weak self] model in
                guard let self, let field = model.fields?.first(where: { $0.key == "value" }), field.units == "mm", let value = field.value, abs(value - 50) < 1e-8,
                      model.workspace.rows.filter({ $0.kind == "joint" }).count == 2,
                      model.workspace.rows.contains(where: { $0.assembly?.joint == self.assemblyCheckHingeID }),
                      model.workspace.rows.contains(where: { $0.assembly?.joint == self.assemblyCheckSliderID }),
                      model.workspace.rows.contains(where: { $0.assembly?.path == [self.assemblyCheckArmID] }) else { self?.assemblyCheckFail("Native reopen lost joint values, IDs or occurrence paths"); return }
                self.assemblyCheckWrite("PASS native file.save_as/doc.open restored slider50 after mutation20; hinge=\(self.assemblyCheckHingeID) slider=\(self.assemblyCheckSliderID)")
                self.assemblyCheckCancelled = { [weak self] result in
                    guard let self else { return }
                    switch result {
                    case .failure(let error): self.assemblyCheckFail(error.description)
                    case .success(let model):
                        self.receive(model.workspace); self.inspectorOpen = false; self.browserOpen = true; self.relayoutPanels()
                        self.assemblyCheckWaitForFrame(model, preview: false, stage: "reopened-history-final", next: advance)
                    }
                }
                self.modellingInspector?.onCancel?()
            }
        case 16 where ProcessInfo.processInfo.arguments.contains("--assembly-pencil-check"):
            assemblyCheckBegin("joint.drive", params: ["joint": assemblyCheckHingeID, "value": "30 deg"]) { [weak self] _ in
                self?.assemblyCheckEvaluate(commit: false) { [weak self] model in
                    guard let self, self.assemblyCheckAssertValue(model, expected: 30, units: "deg", moved: false) else { return }
                    self.inspectorOpen = false; self.browserOpen = false; self.relayoutPanels()
                    self.assemblyCheckWrite("HUMAN PENCIL READY: production hinge drive preview only; drag the ring, then open Properties to Apply or Cancel")
                    self.assemblyCheckWaitForFrame(model, preview: true, stage: "pencil-ready", next: advance)
                }
            }
        default:
            assemblyCheckWrite("PASS COMPLETE production forms, actual engine previews, cancel, Apply, limits, undo/redo, native persistence and matching Metal frames; physical hand input remains separate")
            assemblyCheckRunning = false; assemblyCheckTicket &+= 1
            assemblyCheckSignal?.cancel(); assemblyCheckSignal = nil
        }
    }
}
#endif

private enum SketchText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.sketch." + key) }
}
