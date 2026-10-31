import UIKit
import UniformTypeIdentifiers
#if DEBUG
import Darwin
#endif

/// One adaptive native workspace around the existing Metal view and Rust session.
/// Panels change the actual canvas frame; they never invent a second pick origin.
final class FoundationController: UIViewController, UITableViewDataSource, UITableViewDelegate, UIDocumentPickerDelegate, UIPopoverPresentationControllerDelegate {
    private let canvas = MetalCanvas()
    private let session = NativeSession()
    private let workspace = UIView()
    private let top = UIView()
    private let titleLabel = WorkspaceStyle.label(.headline)
    private let subtitle = WorkspaceStyle.label(.caption2, secondary: true)
    private let status = WorkspaceStatusLabel()
    private let topScroll = UIScrollView()
    private let topActions = UIStackView()
    private let railScroll = UIScrollView()
    private let rail = UIStackView()
    private let navigationScroll = UIScrollView()
    private let navigationActions = UIStackView()
    private let historyActions = UIStackView()
    private let featureHistory = FeatureHistoryBar()
    private var historyExpanded = false
    private let table = UITableView(frame: .zero, style: .plain)
    private let inspectorScroll = UIScrollView()
    private let properties = UIStackView()
    private let scrim = UIControl()
    private let browserSearchBar = UIStackView()
    private let browserSearchField = BrowserSearchField()
    private let browserMatchCount = WorkspaceStyle.label(.caption1, secondary: true)
    private lazy var browserFindButton = WorkspaceButton(BrowserText.text("find")) { [weak self] in self?.toggleBrowserSearch() }
    private lazy var browserSearchDismiss = WorkspaceButton(BrowserText.text("dismiss_find")) { [weak self] in self?.dismissBrowserSearch() }
    private var browserSearchOpen = false
    private var browserSearchBookmark: BrowserScrollBookmark?
    private var browserSearching = false
    private var visibleOriginRows: [String] = []
    private let browserOriginPrefix = "native-origin:"
    private let browserMessage = WorkspaceStyle.label(.body, secondary: true)
    private lazy var browserPanel = WorkspacePanel(title: WorkspaceStyle.text("browser")) { [weak self] in self?.closePanels() }
    private lazy var inspectorPanel = WorkspacePanel(title: WorkspaceStyle.text("inspector")) { [weak self] in self?.closePanels() }
    private lazy var undoButton = WorkspaceButton(NativeText.value("ui.ipados.undo")) { [weak self] in self?.run("edit.undo") }
    private lazy var redoButton = WorkspaceButton(NativeText.value("ui.ipados.redo")) { [weak self] in self?.run("edit.redo") }
    private lazy var browserButton = WorkspaceButton(WorkspaceStyle.text("browser")) { [weak self] in self?.toggleBrowser() }
    private lazy var inspectorButton = WorkspaceButton(WorkspaceStyle.text("inspector")) { [weak self] in self?.toggleInspector() }
    private lazy var viewButton = WorkspaceButton(WorkspaceStyle.text("view"), activationEvent: .primaryActionTriggered) { [weak self] in self?.showViewControls() }
    private lazy var selectButton = WorkspaceButton(WorkspaceStyle.text("select")) { [weak self] in self?.setTool("select") }
    private lazy var radiusButton = WorkspaceButton(WorkspaceStyle.text("radius")) { [weak self] in self?.setTool("post_radius") }
    private lazy var moreButton = WorkspaceButton(WorkspaceStyle.text("tools"), action: {})
    private lazy var modellingButton = WorkspaceButton(ModellingText.text("tools"), action: {})
    private lazy var assemblyButton = WorkspaceButton(AssemblyText.text("tools"), action: {})
    private lazy var cancelToolButton = WorkspaceButton(SketchText.text("cancel_segment")) { [weak self] in
        guard let self else { return }
        if self.planeChoosing || self.planeDraft != nil { self.cancelSketchPlane() }
        else { self.run("tool.cancel") }
    }
    private lazy var createSketchButton = WorkspaceButton(SketchText.text("create")) { [weak self] in self?.chooseSketchPlane() }
    private lazy var sketchToolsButton = WorkspaceButton(SketchText.text("draw"), action: {})
    private lazy var finishSketchButton = WorkspaceButton(SketchText.text("finish"), prominent: true) { [weak self] in self?.run("sketch.finish") }
    private lazy var finishChainButton = WorkspaceButton(SketchText.text("finish_chain")) { [weak self] in self?.run("tool.finish_chain") }
    private lazy var endpointFinishButton: WorkspaceButton = WorkspaceButton(SketchText.text("finish_chain"), prominent: true) { [weak self] in
        guard let self, !self.endpointFinishButton.isHidden, self.endpointFinishButton.isEnabled else { return }
        self.endpointFinishButton.isHidden = true
        self.run("tool.finish_chain")
    }
    private lazy var documentActionsButton = WorkspaceButton(WorkspaceStyle.text("document_actions"), action: {})
    private lazy var homeButton = WorkspaceButton(WorkspaceStyle.text("home")) { [weak self] in
        guard let self else { return }
        self.runViewCommand(self.homeToolbar ? "view.home" : "view.fit")
    }
    private lazy var projectionButton = WorkspaceButton(NativeText.value("ui.ipados.perspective")) { [weak self] in
        self?.togglePerspective()
    }
    private lazy var modifyButton = WorkspaceButton(WorkspaceStyle.text("modify"), action: {})
    private lazy var patternButton = WorkspaceButton(WorkspaceStyle.text("pattern"), action: {})
    private lazy var constraintsButton = WorkspaceButton(WorkspaceStyle.text("constraints"), activationEvent: .primaryActionTriggered) { [weak self] in self?.showConstraints() }
    private lazy var dimensionsButton = WorkspaceButton(SketchText.text("dimension"), action: {})
    private lazy var constructionButton = WorkspaceButton(SketchText.text("construction")) { [weak self] in
        guard let self, self.state?.sketch != nil else { return }
        let curves = self.state?.selection.filter { $0.type == "sketch_curve" }.compactMap { $0.id?.readout } ?? []
        guard !curves.isEmpty else { return }; self.run("sketch.construction", params: ["curves": curves])
    }
    private lazy var sketchQuickTools: [(String, WorkspaceButton)] = [
        ("sketch.line", "line"), ("sketch.rectangle.two_point", "rectangle"),
        ("sketch.circle.center", "circle"), ("sketch.arc.three_point", "arc"),
        ("sketch.spline.fit_point", "spline"), ("sketch.delete", "delete"), ("sketch.trim", "trim")
    ].map { id, label in
        (id, WorkspaceButton(SketchText.text(label)) { [weak self] in self?.setTool(id) })
    }
    private var state: WorkspaceState?
    private var selectionExtrudeTrigger = SelectionExtrudeTrigger<WorkspaceEntity>()
    private var historyActivation: (feature: UInt64, document: String)?
    private var visibleRows: [WorkspaceRow] = []
    private var rowByID: [String: WorkspaceRow] = [:]
    private var parents: Set<String> = []
    private var collapsed: Set<String> = []
    private var browserOpen = false
    private var browserAnimationGeneration: UInt64 = 0
    private var browserControlsEnabled: Bool?
    private var inspectorOpen = false
    private var multiple = UserDefaults.standard.object(forKey: "SolidCraft.workspace.multipleSelection") as? Bool ?? true
    private var filter = WorkspaceSelectionFilter(storedValue: UserDefaults.standard.string(forKey: "SolidCraft.workspace.selectionFilter"))
    private var perspective = false
    private weak var viewControlsPopover: WorkspaceViewPopover?
    private var viewPresentation: ViewControlsPresentation?
    private var viewControlPending = false
    private var active = false
    private var generation: UInt64 = 0
    private var failure: String?
    private var pending = ""
    private var previewStale = false
    private var developmentCheckStarted = false
#if DEBUG
    private var groupedInspectorFrameReady = false
    private var gridAxesCheckAfterFrame: (() -> Void)?
    private var assemblyCheckRunning = false
    private var assemblyCheckTicket: UInt64 = 0
    private var assemblyCheckSignal: DispatchSourceSignal?
    private var assemblyCheckContinue: (() -> Void)?
    private var assemblyCheckWorkspace: ((WorkspaceState) -> Void)?
    private var browserCheckWorkspace: ((WorkspaceState) -> Void)?
    private var historyCheckWorkspace: ((WorkspaceState) -> Void)?
    private var historyDeleteConfirmation: (() -> Void)?
    private var historyMenuDelete: (() -> Void)?
    private var constraintMenuActions: [String: () -> Void] = [:]
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
    private weak var inlineNumericEditor: NumericValueEditorController?
    private var numericStarting = false
    private var numericGeneration: UInt64 = 0
    private var sketchInput: SketchInputPresentation?
    private var planeDraft: (value: Any, label: String)?
    private var planeGeneration: UInt64 = 0
    private var planeConfirming = false
    private var planeChoosing = false
    private var referenceLabels: [String: WorkspaceButton] = [:]
    private var latestReferenceLabels: [ReferenceLabelPresentation] = []
    private var showAllConstraints = false
    private var annotationLabels: [String: SketchDimensionControl] = [:]
    private var sketchAnnotations: [String: SketchAnnotationPresentation] = [:]
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
    private var modellingStartedFromSelection = false
    private var modellingStarting = false
    private var modellingToken: UInt64?
    private var modellingFeature: UInt64?
    private var modellingContext: (document: String, revision: UInt64)?
    private var modellingGeneration: UInt64 = 0
    private var modellingInspector: ModellingInspector?
    private var modelValue: ModelValuePresentation?
    private var acceptsModelValueFrame = false
    private weak var modelValueEditor: NumericValueEditorController?
    private lazy var modelValueButton = WorkspaceButton(HandleText.text("edit_value")) { [weak self] in self?.editModelValue() }
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
    private weak var projectsBrowser: ProjectBrowserController?
    private var importingIntoProject = false
    private var pendingExport: URL?
    private var hasLocalSave = false
    private var localSaveLabel: String?
    private var documentStatus = WorkspaceStyle.text("unsaved_session")
    private var keyboardTop: CGFloat?
    private var railRight = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.railRight")
    private var appearance = (UserDefaults.standard.object(forKey: "SolidCraft.workspace.appearance") as? Int) ?? 2
    // The floating workspace is the only supported presentation; ignore saved legacy values.
    private let compactChrome = true
    private var homeToolbar = UserDefaults.standard.object(forKey: "SolidCraft.workspace.homeToolbar") as? Bool ?? true
    private var compactInspector = UserDefaults.standard.object(forKey: "SolidCraft.workspace.compactInspector") as? Bool ?? true
    private var useTopSafeArea = UserDefaults.standard.object(forKey: "SolidCraft.workspace.useTopSafeArea") as? Bool ?? true
    private var useBottomSafeArea = UserDefaults.standard.object(forKey: "SolidCraft.workspace.useBottomSafeArea") as? Bool ?? true
    private var pinnedBrowser = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.pinnedBrowser")
    private var leftBrowser = UserDefaults.standard.object(forKey: "SolidCraft.workspace.leftBrowser") as? Bool ?? true
    private var diagnostics = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.diagnostics")
    private var gridVisible = UserDefaults.standard.object(forKey: "SolidCraft.workspace.gridVisible") as? Bool ?? true
    private var integratedGridAxes = UserDefaults.standard.object(forKey: "SolidCraft.workspace.integratedGridAxes") as? Bool ?? true
    private var originInBrowser = UserDefaults.standard.object(forKey: "SolidCraft.workspace.originInBrowser") as? Bool ?? false
    private var resetChromeOffsets = false
    private var twoFingerTwist = UserDefaults.standard.object(forKey: "SolidCraft.workspace.twoFingerTwist") as? Bool ?? true
    private var viewpointOrbit = UserDefaults.standard.object(forKey: "SolidCraft.workspace.viewpointOrbit") as? Bool ?? true
    private var dragStepMM = DragResolution.current
    private var directHandles = UserDefaults.standard.object(forKey: "SolidCraft.workspace.directHandles") as? Bool ?? true
    private var automaticSubtract = UserDefaults.standard.object(forKey: "SolidCraft.workspace.automaticSubtract") as? Bool ?? true
    private var inlineDimensions = UserDefaults.standard.object(forKey: "SolidCraft.workspace.inlineDimensions") as? Bool ?? true
    private var freeDimensions = UserDefaults.standard.object(forKey: "SolidCraft.workspace.freeDimensions") as? Bool ?? true
    private var repeatRectangles = UserDefaults.standard.object(forKey: "SolidCraft.workspace.repeatRectangles") as? Bool ?? false
    private var cancelToolsOnDismiss = UserDefaults.standard.object(forKey: "SolidCraft.workspace.cancelToolsOnDismiss") as? Bool ?? true
    private var extrudeOnSketchSelection = UserDefaults.standard.object(forKey: "SolidCraft.workspace.extrudeOnSketchSelection") as? Bool ?? true
    private var extrudeOnFaceSelection = UserDefaults.standard.object(forKey: "SolidCraft.workspace.extrudeOnFaceSelection") as? Bool ?? true
    private var visualPlaneSelection = UserDefaults.standard.object(forKey: "SolidCraft.workspace.visualPlaneSelection") as? Bool ?? true
    private var legacyOriginPlanes = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.legacyOriginPlanes")
    private var axisLabelsVisible = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.axisLabels")
    private var originVisibility = UserDefaults.standard.dictionary(forKey: "SolidCraft.workspace.originVisibility") as? [String: Bool] ?? [:]
    private static let originItems = ["O", "X", "Y", "Z", "XY", "XZ", "YZ"]
    private static let originAxes = ["X", "Y", "Z"]
    private static let originPlanes = ["XY", "XZ", "YZ"]
    private var originExpanded = true

    private var numericEditing: Bool { numericStarting || numericToken != nil || transformDraft != nil }
    private var topStripHeaderEnabled: Bool {
        // Older iPadOS versions reserve the top centre for multitasking controls.
        if #available(iOS 26.0, *) { return useTopSafeArea && traitCollection.userInterfaceIdiom == .pad }
        return false
    }
    private var modellingEditing: Bool { modellingStarting || modellingToken != nil }
    private var textEntryActive: Bool { browserSearchField.isFirstResponder || modelValueEditor?.editor.field.isFirstResponder == true || editor?.field.isFirstResponder == true || transformEditors.contains { $0.editor.field.isFirstResponder } || modellingInspector?.textEntryActive == true }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.background
        applyAppearance()
        view.addSubview(workspace)
        workspace.addSubview(canvas)
        workspace.addSubview(top)
        workspace.addSubview(status)
        workspace.addSubview(railScroll)
        workspace.addSubview(featureHistory)
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
            button.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
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
        browserFindButton.systemArtwork("magnifyingglass", compact: true)
        browserFindButton.accessibilityIdentifier = "browser-find"
        browserPanel.setHeaderAction(browserFindButton)
        browserSearchBar.axis = .vertical; browserSearchBar.spacing = 4
        let searchEntry = UIStackView(arrangedSubviews: [browserSearchField, browserSearchDismiss])
        searchEntry.axis = .horizontal; searchEntry.spacing = 4
        browserSearchField.placeholder = BrowserText.text("find")
        browserSearchField.accessibilityLabel = BrowserText.text("find")
        browserSearchField.accessibilityIdentifier = "browser-search-field"
        browserSearchField.font = .preferredFont(forTextStyle: .body)
        browserSearchField.adjustsFontForContentSizeCategory = true
        browserSearchField.backgroundColor = WorkspaceStyle.fieldBackground
        browserSearchField.borderStyle = .roundedRect
        browserSearchField.clearButtonMode = .always
        browserSearchField.autocorrectionType = .no
        browserSearchField.spellCheckingType = .no
        browserSearchField.autocapitalizationType = .none
        browserSearchField.returnKeyType = .done
        browserSearchField.addAction(UIAction { [weak self] _ in self?.browserQueryChanged() }, for: .editingChanged)
        browserSearchField.addAction(UIAction { [weak self] _ in
            self?.browserSearchField.resignFirstResponder(); self?.updateAvailability()
        }, for: .editingDidEndOnExit)
        browserSearchField.addAction(UIAction { [weak self] _ in
            self?.canvas.cancelInput(); self?.updateAvailability()
        }, for: [.editingDidBegin, .editingDidEnd])
        browserSearchField.heightAnchor.constraint(greaterThanOrEqualToConstant: 44).isActive = true
        browserSearchField.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        browserSearchDismiss.artwork("close", compact: true)
        browserSearchDismiss.widthAnchor.constraint(equalToConstant: 44).isActive = true
        browserSearchDismiss.accessibilityIdentifier = "browser-dismiss-find"
        browserMatchCount.numberOfLines = 0
        browserMatchCount.accessibilityIdentifier = "browser-match-count"
        browserSearchBar.addArrangedSubview(searchEntry); browserSearchBar.addArrangedSubview(browserMatchCount)
        browserSearchBar.isHidden = true
        browserPanel.content.addSubview(browserSearchBar)
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
        configureChrome()
        featureHistory.onPreferredSizeChange = { [weak self] in self?.view.setNeedsLayout() }
        featureHistory.onToggle = { [weak self] in
            guard let self else { return }
            self.canvas.cancelInput()
            self.historyExpanded.toggle()
            self.featureHistory.setExpanded(self.historyExpanded)
            self.relayoutPanels()
        }
        featureHistory.onSelect = { [weak self] id in
            self?.activateHistoryFeature(id)
        }
        featureHistory.onShowActions = { [weak self] id, source in self?.showHistoryFeatureActions(id, source: source) }
        featureHistory.onMove = { [weak self] position in
            self?.historyActivation = nil; self?.moveHistory(to: position)
        }
        featureHistory.onScrubBegan = { [weak self] in self?.historyActivation = nil; self?.canvas.cancelInput() }
        featureHistory.onScrubMove = { [weak self] position, completion in
            guard let self else { completion(false); return }
            self.moveHistory(to: position) { result in
                if case .success = result { completion(true) } else { completion(false) }
            }
        }
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (controller: FoundationController, _: UITraitCollection) in
            controller.rebuildMenus()
            if controller.compactChrome {
                [controller.top, controller.railScroll, controller.navigationScroll, controller.historyActions,
                 controller.browserButton, controller.status].forEach(WorkspaceStyle.floatingSurface)
            }
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
        canvas.onInput = { [weak self] sample in
            if sample.phase == .began { self?.historyActivation = nil }
            self?.session.record(sample)
        }
        canvas.onAppearanceChange = { [weak self] in self?.updateCanvasAppearance() }
        session.onFailure = { [weak self] error in self?.showFailure(error) }
        session.onWorkspace = { [weak self] value in self?.receive(value) }
        session.onSketchInput = { [weak self] value in
            guard let self, self.sketchInput != value else { return }
            self.sketchInput = value; self.updateStatus(); self.rebuildMenus()
            self.layoutEndpointFinish()
        }
        session.onSketchAnnotations = { [weak self] annotations in self?.showSketchAnnotations(annotations) }
        canvas.dimensionAt = { [weak self] point in
            guard let self else { return nil }
            return self.annotationLabels.filter { id, label in
                guard let annotation = self.sketchAnnotations[id] else { return false }
                return DimensionTapTracker.hitFrame(label.frame, helper: annotation.dimension_type != nil).contains(point)
            }.min { a, b in
                hypot(a.value.center.x - point.x, a.value.center.y - point.y) < hypot(b.value.center.x - point.x, b.value.center.y - point.y)
            }?.key
        }
        canvas.onDimensionTap = { [weak self] id in self?.editSketchAnnotation(id) }
        session.onConstraintIndicators = { [weak self] indicators in self?.showConstraintIndicators(indicators) }
        session.onReferenceLabels = { [weak self] labels in self?.showReferenceLabels(labels) }
        session.onViewPresentation = { [weak self] value in
            guard let self, self.viewPresentation != value else { return }
            self.viewPresentation = value
            self.perspective = value.camera.fov > 0
            self.rebuildViewMenu()
            self.projectionButton.active(self.perspective)
            self.table.reloadSections(IndexSet(integer: 0), with: .none)
        }
        modelValueButton.accessibilityIdentifier = "workspace-model-value"
        modelValueButton.accessibilityLabel = HandleText.text("edit_value")
        modelValueButton.isHidden = true
        WorkspaceStyle.floatingSurface(modelValueButton)
        workspace.insertSubview(modelValueButton, aboveSubview: canvas)
        endpointFinishButton.artwork("finish", compact: true)
        endpointFinishButton.accessibilityIdentifier = "sketch-endpoint-finish"
        endpointFinishButton.isHidden = true
        workspace.insertSubview(endpointFinishButton, aboveSubview: canvas)
        session.onModelValue = { [weak self] value in
            guard let self else { return }
            self.modelValue = self.acceptsModelValueFrame ? value : nil
            self.layoutModelValue()
        }
        session.onToolWorkCompleted = { [weak self] in self?.inspectModelling() }
        session.onModelDismissed = { [weak self] token in
            guard let self, self.modellingToken == token else { return }
            // Rust already invalidated the token and any pending preview. Drop
            // the matching native form even if selection was already empty.
            self.cancelModelling(alreadyDismissed: true)
            self.inspectorOpen = false; self.browserOpen = false
            self.relayoutPanels(); self.session.refreshWorkspace()
        }
        session.onStatus = { [weak self] message in
            guard let self else { return }
            self.pending = message; self.failure = nil; self.updateStatus(); self.updateAvailability()
        }
        session.onFrame = { [weak self] presented, revision, epoch, cameraEpoch, documentID, metrics, preview, previewStale, _ in
            guard let self, self.active else { return }
            self.acceptsModelValueFrame = false
            if presented {
                guard let metrics, metrics.count == 3, let pixels = self.canvas.pixelSize,
                      abs(metrics[0] * metrics[2] - Double(pixels.width)) < 0.01,
                      abs(metrics[1] * metrics[2] - Double(pixels.height)) < 0.01,
                      abs(metrics[2] - Double(self.canvas.contentScaleFactor)) < 0.000001 else { return }
                self.canvas.geometryRevision = revision; self.canvas.viewportEpoch = epoch
                self.canvas.cameraEpoch = cameraEpoch; self.canvas.documentID = documentID
                self.canvas.displayingPreview = preview
                self.acceptsModelValueFrame = true
                let staleChanged = self.previewStale != previewStale
                self.previewStale = previewStale
                if previewStale { self.modellingInspector?.invalidatePreview() }
                if staleChanged { self.updateStatus() }
            }
            #if DEBUG
            if presented {
                self.groupedInspectorFrameReady = true
                if let next = self.gridAxesCheckAfterFrame {
                    self.gridAxesCheckAfterFrame = nil
                    next()
                }
            }
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
                } else if ProcessInfo.processInfo.arguments.contains("--numeric-entry-check") {
                    self.developmentCheckStarted = true; self.checkNumericEntryOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--inline-dimension-check") {
                    self.developmentCheckStarted = true; self.checkInlineDimensionsOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--sketch-toolbar-check") {
                    self.developmentCheckStarted = true; self.checkSketchToolbarOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--line-finish-check") {
                    self.developmentCheckStarted = true; self.checkLineFinishOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--constraints-menu-check") {
                    self.developmentCheckStarted = true; self.checkConstraintsMenuOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--assembly-check") || ProcessInfo.processInfo.arguments.contains("--face-extrude-check") || ProcessInfo.processInfo.arguments.contains("--compact-inspector-check") {
                    self.developmentCheckStarted = true; self.startAssemblyCheck()
                } else if ProcessInfo.processInfo.arguments.contains("--tool-dismissal-check") {
                    self.developmentCheckStarted = true; self.checkToolDismissalOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--selection-taps-check") {
                    self.developmentCheckStarted = true; self.checkSelectionTapsOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--grouped-inspector-check") {
                    self.startGroupedInspectorCheckIfReady()
                } else if ProcessInfo.processInfo.arguments.contains("--chrome-check") {
                    self.developmentCheckStarted = true; self.checkChromeOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--browser-check") {
                    self.developmentCheckStarted = true; self.checkBrowserOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--history-check") {
                    self.developmentCheckStarted = true; self.checkHistoryOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--deferred-changes-check") {
                    self.developmentCheckStarted = true; self.checkDeferredChangesOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--grid-axes-check") {
                    self.developmentCheckStarted = true; self.checkGridAxesOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--view-controls-check") {
                    self.developmentCheckStarted = true; self.checkViewControlsOnDevice()
                } else if ProcessInfo.processInfo.arguments.contains("--history-actions-check") {
                    self.developmentCheckStarted = true; self.checkHistoryActionsOnDevice()
                }
            }
            if presented { self.assemblyCheckPresented(document: documentID, revision: revision, preview: preview) }
            #endif
        }
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillChangeFrameNotification, object: nil)
        NotificationCenter.default.addObserver(self, selector: #selector(keyboardChanged(_:)), name: UIResponder.keyboardWillHideNotification, object: nil)
        let expected = generation
        session.onProjectChange = { [weak self] project in
            guard let self else { return }
            self.hasLocalSave = true
            self.documentStatus = ProjectText.text("saved")
            self.collapsed.removeAll(); self.selectedAssemblyRowID = nil
            self.titleLabel.text = project.name
            self.titleLabel.accessibilityLabel = project.name
            self.updateSubtitle()
        }
        session.start { [weak self] result in
            guard let self, self.generation == expected else { return }
            switch result {
            case .success:
                self.failure = nil
                self.updateCanvasAppearance()
                self.session.command("view.grid", params: ["visible": self.gridVisible, "integrated_axes": self.integratedGridAxes])
                self.session.command("view.touch_orbit", params: ["follows_viewpoint": self.viewpointOrbit])
                self.session.command("view.touch_twist", params: ["enabled": self.twoFingerTwist])
                self.session.command("view.drag_precision", params: ["step_mm": self.dragStepMM])
                self.session.command("view.tool_handles", params: ["enabled": self.directHandles])
                self.session.command("view.automatic_subtract", params: ["enabled": self.automaticSubtract])
                self.session.command("view.tool_dismissal", params: ["cancel_on_dismiss": self.cancelToolsOnDismiss])
                self.session.command("tool.free_dimensions", params: ["enabled": self.freeDimensions])
                self.session.command("tool.repeat_rectangles", params: ["enabled": self.repeatRectangles])
                self.session.command("select.filter", params: ["filter": self.filter.rawValue])
                self.session.command("select.multiple", params: ["enabled": self.multiple])
                self.session.command("view.origin_style", params: ["legacy": self.legacyOriginPlanes])
                for (item, visible) in self.originVisibility {
                    self.session.command("view.origin", params: ["item": item, "visible": visible])
                }
                self.updateSurface()
            case .failure(let error): self.showFailure(error)
            }
        }
        rebuildMenus(); rebuildInspector(); updateAvailability()
    }

    private func configureChrome() {
        resetChromeOffsets = true
        for stack in [topActions, rail, navigationActions, historyActions] {
            stack.arrangedSubviews.forEach { stack.removeArrangedSubview($0); $0.removeFromSuperview() }
        }
        documentActionsButton.removeFromSuperview()
        viewButton.removeFromSuperview()
        homeButton.updateTitle(WorkspaceStyle.text(homeToolbar ? "home" : "fit"))
        let chromeIcons: [(WorkspaceButton, String)] = [
            (undoButton, "undo"), (redoButton, "redo"), (browserButton, "list"),
            (inspectorButton, "inspect"), (selectButton, "select"),
            (radiusButton, "dimension"), (moreButton, "settings"), (modellingButton, "extrude"),
            (modifyButton, "fillet"), (patternButton, "pattern_rect"), (assemblyButton, "component"),
            (cancelToolButton, "sketch_cancel"), (createSketchButton, "sketch"),
            (sketchToolsButton, "sketch"), (finishSketchButton, "finish"), (finishChainButton, "finish"),
            (documentActionsButton, "folder"), (homeButton, homeToolbar ? "home" : "fit"), (projectionButton, "perspective"),
            (constraintsButton, "c_parallel"), (dimensionsButton, "dimension"), (constructionButton, "construction")
        ]
        chromeIcons.forEach { $0.0.artwork($0.1, compact: compactChrome) }
        viewButton.artwork("chevron")
        viewButton.configuration?.imagePlacement = .trailing
        viewButton.configuration?.baseForegroundColor = WorkspaceStyle.foreground
        viewButton.accessibilityIdentifier = "workspace-view-menu"
        viewButton.showsLargeContentViewer = false
        for interaction in viewButton.interactions where interaction is UILargeContentViewerInteraction { viewButton.removeInteraction(interaction) }
        for (id, button) in sketchQuickTools {
            button.commandArtwork(id, compact: true)
            button.accessibilityIdentifier = "workspace-tool-" + id
        }
        rail.spacing = compactChrome ? 2 : 8
        if compactChrome {
            // A single menu button needs no scroll view. In the top strip,
            // UIKit's scroll-edge treatment can obscure this control.
            documentActionsButton.translatesAutoresizingMaskIntoConstraints = true
            top.addSubview(documentActionsButton)
            // Completion must remain visible while the drawing tools scroll.
            finishSketchButton.artwork("finish")
            finishSketchButton.translatesAutoresizingMaskIntoConstraints = true
            top.addSubview(finishSketchButton)
            [selectButton, createSketchButton].forEach(rail.addArrangedSubview)
            sketchQuickTools.forEach { rail.addArrangedSubview($0.1) }
            [sketchToolsButton, finishChainButton, modellingButton, modifyButton,
             patternButton, assemblyButton, radiusButton, moreButton, cancelToolButton].forEach(rail.addArrangedSubview)
            navigationActions.axis = .vertical; navigationActions.spacing = 2
            [homeButton, projectionButton, inspectorButton, constraintsButton, dimensionsButton, constructionButton].forEach(navigationActions.addArrangedSubview)
            viewButton.translatesAutoresizingMaskIntoConstraints = true
            workspace.addSubview(viewButton)
            WorkspaceStyle.floatingSurface(viewButton)
            historyActions.axis = .horizontal; historyActions.spacing = 2; historyActions.distribution = .fillEqually
            [undoButton, redoButton].forEach(historyActions.addArrangedSubview)
            // UIStackView turns this off while it owns the browser button. Its
            // floating frame is manually laid out after leaving that stack.
            browserButton.translatesAutoresizingMaskIntoConstraints = true
            workspace.addSubview(browserButton); workspace.addSubview(historyActions); workspace.addSubview(navigationScroll)
            if navigationActions.superview == nil {
                navigationScroll.addSubview(navigationActions)
                navigationActions.translatesAutoresizingMaskIntoConstraints = false
                NSLayoutConstraint.activate([
                    navigationActions.leadingAnchor.constraint(equalTo: navigationScroll.contentLayoutGuide.leadingAnchor),
                    navigationActions.trailingAnchor.constraint(equalTo: navigationScroll.contentLayoutGuide.trailingAnchor),
                    navigationActions.topAnchor.constraint(equalTo: navigationScroll.contentLayoutGuide.topAnchor),
                    navigationActions.bottomAnchor.constraint(equalTo: navigationScroll.contentLayoutGuide.bottomAnchor),
                    navigationActions.widthAnchor.constraint(equalTo: navigationScroll.frameLayoutGuide.widthAnchor)
                ])
            }
            navigationScroll.showsVerticalScrollIndicator = false
            [top, railScroll, navigationScroll, historyActions, browserButton, status].forEach(WorkspaceStyle.floatingSurface)
            subtitle.numberOfLines = 1
        } else {
            [undoButton, redoButton, browserButton, inspectorButton, viewButton].forEach(topActions.addArrangedSubview)
            [selectButton, createSketchButton, sketchToolsButton, finishChainButton, finishSketchButton,
             modellingButton, assemblyButton, radiusButton, moreButton, cancelToolButton].forEach(rail.addArrangedSubview)
            top.backgroundColor = .clear; status.backgroundColor = .clear
            subtitle.numberOfLines = 2
        }
        topScroll.isHidden = compactChrome
        navigationScroll.isHidden = !compactChrome; historyActions.isHidden = !compactChrome
        for scroll in [topScroll, railScroll, navigationScroll] { scroll.contentInsetAdjustmentBehavior = .never }
        topScroll.showsHorizontalScrollIndicator = !compactChrome
        // Panels are above visible chrome. Their bounds alone own UI contacts;
        // there is no full-screen hit-testing overlay in the floating layout.
        workspace.bringSubviewToFront(scrim)
        workspace.bringSubviewToFront(browserPanel); workspace.bringSubviewToFront(inspectorPanel)
        homeButton.accessibilityIdentifier = homeToolbar ? "workspace-home" : "workspace-fit"
        projectionButton.accessibilityIdentifier = "workspace-projection"
        documentActionsButton.accessibilityIdentifier = "workspace-document-actions"
        constraintsButton.accessibilityIdentifier = "workspace-constraints"
        dimensionsButton.accessibilityIdentifier = "workspace-dimensions"
        constructionButton.accessibilityIdentifier = "workspace-construction"
        finishSketchButton.accessibilityIdentifier = "workspace-finish-sketch"
        cancelToolButton.accessibilityIdentifier = "workspace-cancel-tool"
        if state != nil { updateStatus() }
        view.setNeedsLayout()
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
            clearNumericEditor()
            transformGeneration &+= 1; transformDraft = nil
            transformBusy = false
            transformEditors.forEach { $0.editor.field.resignFirstResponder() }; transformEditors.removeAll(); transformMessage = nil
            failure = WorkspaceStyle.text("invalidated")
        }
        let previous = state
        state = value
        if SelectionExtrudeTrigger<WorkspaceEntity>.shouldDismiss(
            automatic: modellingStartedFromSelection, committing: modellingCommitting,
            selection: value.selection, source: { self.extrudeSelectionSource($0) }) {
            // Cancel the draft as well as its chrome, including a begin request
            // still in flight. The generation guards discard its late callback.
            cancelModelling()
            inspectorOpen = false
            relayoutPanels()
        }
        if let label = localSaveLabel {
            documentStatus = label; localSaveLabel = nil; hasLocalSave = true
        } else if hasLocalSave {
            if value.dirty == true { documentStatus = ModellingText.text("unsaved_changes") }
            else if value.dirty == false { documentStatus = ProjectText.text("saved") }
            else if previous?.geometry_revision != value.geometry_revision || previous?.document_id != value.document_id { documentStatus = ModellingText.text("unsaved_changes") }
        }
        updateSubtitle()
        if value.sketch != nil { planeDraft = nil; planeChoosing = false }
        if let previous, previous.document_id != value.document_id || previous.geometry_revision != value.geometry_revision,
           planeChoosing || planeDraft != nil {
            cancelSketchPlane()
        }
        if planeChoosing, planeDraft == nil, let selected = value.selection.first {
            if selected.type == "plane", let name = selected.name {
                previewSketchPlane(name, label: name)
            } else if selected.type == "face", let point = selected.point, let body = selected.body {
                var plane: [String: Any] = ["face": point, "body": body]
                if let index = selected.index { plane["index"] = index }
                if let path = selected.path { plane["path"] = path }
                previewSketchPlane(plane, label: SketchText.text("selected_face"))
            }
        }
        titleLabel.text = session.currentProject?.name ?? value.title
        titleLabel.accessibilityLabel = titleLabel.text
        canvas.accessibilityValue = value.selection.isEmpty ? NativeText.value("ui.ipados.no_selection") : value.selection.map(\.readout).joined(separator: ", ")
        if previous?.document_id != value.document_id {
            browserSearchField.resignFirstResponder(); browserSearchField.text = ""
            browserSearchOpen = false; browserSearching = false; browserSearchBookmark = nil
            browserSearchBar.isHidden = true; browserFindButton.active(false)
            collapsed.removeAll(); selectedAssemblyRowID = nil; lastOperationFeedback = []
        }
        if previous?.selection != value.selection, previous?.geometry_revision == value.geometry_revision {
            if let id = selectedAssemblyRowID, let row = value.rows.first(where: { $0.id == id }),
               row.entity == nil || !value.selection.contains(where: { $0 == row.entity }) {
                selectedAssemblyRowID = nil
            }
            lastOperationFeedback = []
        }
        do {
            rowByID = Dictionary(value.rows.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
            parents = Set(value.rows.compactMap(\.parent_id))
            if let id = selectedAssemblyRowID, rowByID[id] == nil { selectedAssemblyRowID = nil }
        }
        if previous?.selection != value.selection && !browserSearching {
            for row in value.rows where row.selected {
                var parent = row.parent_id
                var visited: Set<String> = []
                while let id = parent, visited.insert(id).inserted {
                    collapsed.remove(id); parent = rowByID[id]?.parent_id
                }
            }
        }
        updateRows()
        if !numericEditing && !modellingEditing { rebuildInspector() }
        if modellingToken != nil && previous?.selection != value.selection { inspectModelling() }
        updateAvailability(); rebuildMenus(); updateStatus(); view.setNeedsLayout()
#if DEBUG
        assemblyCheckWorkspace?(value)
        browserCheckWorkspace?(value)
        historyCheckWorkspace?(value)
        startGroupedInspectorCheckIfReady()
#endif
        openActivatedHistoryFeature()
        let idle = active && pending.isEmpty && !numericEditing && !modellingEditing &&
            !planeChoosing && planeDraft == nil && value.sketch == nil && value.active_tool == "select" &&
            !textEntryActive && presentedViewController == nil && !featureHistory.isScrubbing
        if selectionExtrudeTrigger.update(document: value.document_id, revision: value.geometry_revision,
                                     selection: value.selection, enabled: true, idle: idle,
                                     source: { self.extrudeSelectionSource($0, respectingPreferences: true) }) {
            beginModelling(operation: "solid.extrude", fromSelection: true)
        }
    }

    private func extrudeSelectionSource(_ entity: WorkspaceEntity, respectingPreferences: Bool = false) -> String? {
        switch entity.type {
        case "face" where !respectingPreferences || extrudeOnFaceSelection: return "face"
        case "profile" where !respectingPreferences || extrudeOnSketchSelection:
            // A draft can use multiple regions of one sketch, but cannot mix
            // sketches or combine sketch regions with solid faces.
            return entity.sketch.map { "profile:\($0)" }
        default: return nil
        }
    }

    private func updateRows() {
        guard let state else { return }
        let nodes = state.rows.map { BrowserSearchNode(id: $0.id, parent: $0.parent_id, name: browserName($0)) }
        var referenceNodes: [BrowserSearchNode] = []
        if originInBrowser {
            referenceNodes = [BrowserSearchNode(id: browserOriginPrefix + "all", parent: nil, name: SketchText.text("origin_references"))]
            referenceNodes += Self.originItems.map {
                BrowserSearchNode(id: browserOriginPrefix + $0, parent: browserOriginPrefix + "all",
                                  name: $0 == "O" ? SketchText.text("origin_point") : $0)
            }
        }
        var collapsedIDs = collapsed
        if !originExpanded { collapsedIDs.insert(browserOriginPrefix + "all") }
        let projection = BrowserSearchProjection.make(nodes: referenceNodes + nodes,
            query: browserSearchOpen ? browserSearchField.text ?? "" : "", collapsed: collapsedIDs)
        browserSearching = projection.searching
        visibleRows = projection.ids.compactMap { rowByID[$0] }
        visibleOriginRows = projection.ids.filter { $0.hasPrefix(browserOriginPrefix) }
            .map { String($0.dropFirst(browserOriginPrefix.count)) }
        browserMatchCount.text = BrowserText.text("match_count").replacingOccurrences(of: "{count}", with: String(projection.matches.count))
        browserMatchCount.isHidden = !projection.searching
        browserMessage.text = visibleRows.isEmpty && visibleOriginRows.isEmpty ?
            (projection.searching ? BrowserText.text("no_matches") : WorkspaceStyle.text("empty")) : nil
        table.reloadData()
    }

    private func browserBookmark() -> BrowserScrollBookmark {
        let first = table.indexPathsForVisibleRows?.sorted().first
        let id = first.flatMap { index -> String? in
            if index.section == 0, visibleOriginRows.indices.contains(index.row) { return browserOriginPrefix + visibleOriginRows[index.row] }
            return visibleRows.indices.contains(index.row) ? visibleRows[index.row].id : nil
        }
        return BrowserScrollBookmark(id: id, displacement: first.map { table.contentOffset.y - table.rectForRow(at: $0).minY } ?? 0,
                                     fallback: table.contentOffset.y)
    }

    private func restoreBrowserBookmark() {
        guard let bookmark = browserSearchBookmark else { return }
        browserSearchBookmark = nil
        table.layoutIfNeeded()
        var index: IndexPath?
        if let id = bookmark.id {
            if let row = visibleRows.firstIndex(where: { $0.id == id }) { index = IndexPath(row: row, section: 1) }
            else if let row = visibleOriginRows.firstIndex(where: { browserOriginPrefix + $0 == id }) { index = IndexPath(row: row, section: 0) }
        }
        let maximum = max(0, table.contentSize.height - table.bounds.height)
        table.setContentOffset(CGPoint(x: 0, y: bookmark.restoredY(rowY: index.map { table.rectForRow(at: $0).minY }, maximum: maximum)), animated: false)
    }

    private func browserQueryChanged() {
        let wasSearching = browserSearching
        if !wasSearching && browserSearchBookmark == nil { browserSearchBookmark = browserBookmark() }
        updateRows(); view.setNeedsLayout(); view.layoutIfNeeded()
        if wasSearching && !browserSearching { restoreBrowserBookmark() }
    }

    private func toggleBrowserSearch() {
        guard !numericEditing && !modellingEditing && !planeConfirming && pending.isEmpty else { return }
        if browserSearchOpen { dismissBrowserSearch(); return }
        browserSearchBookmark = browserBookmark()
        canvas.cancelInput(); browserSearchOpen = true; browserSearchBar.isHidden = false
        browserFindButton.active(true); updateRows(); view.setNeedsLayout(); view.layoutIfNeeded()
        browserSearchField.becomeFirstResponder(); updateAvailability()
        UIAccessibility.post(notification: .layoutChanged, argument: browserSearchField)
    }

    private func dismissBrowserSearch() {
        browserSearchField.resignFirstResponder(); browserSearchField.text = ""
        browserSearchOpen = false; browserSearchBar.isHidden = true; browserFindButton.active(false)
        updateRows(); view.setNeedsLayout(); view.layoutIfNeeded(); restoreBrowserBookmark(); updateAvailability()
        UIAccessibility.post(notification: .layoutChanged, argument: browserFindButton)
    }

    func numberOfSections(in tableView: UITableView) -> Int { 2 }
    func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        section == 0 ? visibleOriginRows.count : visibleRows.count
    }
    func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "model-row", for: indexPath)
        if indexPath.section == 0 {
            guard visibleOriginRows.indices.contains(indexPath.row) else { return cell }
            let id = visibleOriginRows[indexPath.row]
            return originCell(cell, row: id == "all" ? 0 : (Self.originItems.firstIndex(of: id).map { $0 + 1 } ?? 0))
        }
        guard visibleRows.indices.contains(indexPath.row) else { return cell }
        let row = visibleRows[indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = browserName(row)
        content.image = WorkspaceIcon.image(row.browser?.icon ?? "component", traits: traitCollection, size: 24)
        content.imageProperties.maximumSize = CGSize(width: 24, height: 24)
        content.imageProperties.reservedLayoutSize = CGSize(width: 24, height: 24)
        content.imageToTextPadding = 8
        content.secondaryText = nil
        content.textProperties.font = .preferredFont(forTextStyle: parents.contains(row.id) ? .headline : .body)
        content.textProperties.numberOfLines = 1
        content.textProperties.color = row.browser?.drawn == false ? .secondaryLabel : .label
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
        let controls = UIStackView()
        controls.axis = .horizontal
        let document = state?.document_id
        let revision = state?.geometry_revision
        if let browser = row.browser, browser.visibility != nil, let visible = browser.visible {
            let label = BrowserText.text(visible ? "hide" : "show") + ": " + browserName(row)
            let eye = WorkspaceButton(label) { [weak self] in
                guard let self, let current = self.currentBrowserRow(row.id, document: document, revision: revision) else { return }
                self.setBrowserVisibility(current)
            }
            eye.artwork("eye", compact: true)
            eye.active(visible)
            eye.alpha = visible ? 1 : 0.45
            eye.availability(enabled: browserCanChange)
            eye.accessibilityIdentifier = "browser-visibility-" + row.id
            eye.accessibilityValue = BrowserText.text(browser.drawn == false ? "hidden" : "visible")
            eye.widthAnchor.constraint(equalToConstant: 44).isActive = true
            controls.addArrangedSubview(eye)
        } else { controls.addArrangedSubview(browserEmptyControl()) }
        if row.browser?.container == true || parents.contains(row.id) {
            let expand = WorkspaceButton(WorkspaceStyle.text(collapsed.contains(row.id) ? "open_group" : "close_group") + ": " + browserName(row)) { [weak self] in self?.toggleGroup(row.id) }
            expand.artwork(!browserSearching && collapsed.contains(row.id) ? "chevron_right" : "chevron", compact: true)
            expand.availability(enabled: !browserSearching)
            expand.widthAnchor.constraint(equalToConstant: 44).isActive = true
            controls.addArrangedSubview(expand)
        } else { controls.addArrangedSubview(browserEmptyControl()) }
        controls.frame = CGRect(x: 0, y: 0, width: BrowserRowColumns.width, height: BrowserRowColumns.touch)
        cell.accessoryType = .none
        cell.accessoryView = controls.arrangedSubviews.isEmpty ? nil : controls
        cell.accessibilityLabel = browserName(row)
        cell.accessibilityValue = row.browser?.drawn.map { BrowserText.text($0 ? "visible" : "hidden") }
        cell.accessibilityTraits = selected ? [.button, .selected] : .button
        cell.accessibilityHint = numericEditing ? WorkspaceStyle.text("editing_lock") : BrowserText.text("hold_for_actions")
        cell.selectionStyle = .none
        cell.accessibilityIdentifier = "workspace-row-" + row.id
        return cell
    }
    func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        if indexPath.section == 0 {
            guard visibleOriginRows.indices.contains(indexPath.row) else { return }
            let reference = visibleOriginRows[indexPath.row]
            if reference == "all" { if !browserSearching { originExpanded.toggle(); updateRows() } }
            else if Self.originItems.contains(reference) {
                let item = reference
                setOriginVisibility(item: item, visible: !originVisible(item))
            }
            return
        }
        guard !numericEditing, visibleRows.indices.contains(indexPath.row) else { return }
        historyActivation = nil
        let row = visibleRows[indexPath.row]
        selectedAssemblyRowID = row.assembly == nil ? nil : row.id
        lastOperationFeedback = []
        if let entity = row.entity {
            let document = state?.document_id
            canvas.cancelInput()
            session.selectEntity(entity.dictionary, additive: multiple) { [weak self] result in
                guard case .success = result, (row.kind == "sketch" || row.kind == "feature" || row.assembly != nil),
                      let self, self.state?.document_id == document, self.rowByID[row.id] != nil, !self.multiple else { return }
                if self.modellingEditing { return }
                self.inspectorOpen = true; self.browserOpen = false
                self.rebuildInspector(); self.relayoutPanels()
            }
        } else if row.assembly != nil {
            inspectorOpen = true; browserOpen = false; rebuildInspector(); relayoutPanels(); table.reloadData()
        } else if parents.contains(row.id) { toggleGroup(row.id) }
    }
    private func browserEmptyControl() -> UIView {
        let space = UIView()
        space.isUserInteractionEnabled = false; space.isAccessibilityElement = false
        space.widthAnchor.constraint(equalToConstant: BrowserRowColumns.touch).isActive = true
        return space
    }

    private func currentBrowserRow(_ id: String, document: String?, revision: UInt64?) -> WorkspaceRow? {
        guard browserCanChange, state?.document_id == document, state?.geometry_revision == revision else { return nil }
        return rowByID[id]
    }

    private func toggleGroup(_ id: String) {
        guard !browserSearching else { return }
        if collapsed.contains(id) { collapsed.remove(id) } else { collapsed.insert(id) }
        updateRows()
    }

    private var browserCanChange: Bool {
        state != nil && !numericEditing && !modellingEditing && !planeChoosing && !planeConfirming && pending.isEmpty
    }

    private var historyContextAllowsNavigation: Bool {
        state != nil && !numericEditing && !modellingEditing && !planeChoosing && !planeConfirming &&
            state?.sketch == nil && planeDraft == nil && !textEntryActive
    }
    private var historyCanNavigate: Bool { historyContextAllowsNavigation && pending.isEmpty }

    private func moveHistory(to position: Int, completion: ((Result<Void, BridgeError>) -> Void)? = nil) {
        guard historyCanNavigate, let model = state?.modelling, (0...model.timeline.count).contains(position) else {
            completion?(.failure(BridgeError(code: "disabled", message: "ui.ipados.workspace.finish_edit")))
            return
        }
        canvas.cancelInput(); failure = nil; selectedAssemblyRowID = nil
        session.command("timeline.roll_to", params: ["position": position == model.timeline.count ? "end" as Any : position], completion: completion)
    }

    private func selectHistoryFeature(_ id: UInt64, completion: ((Result<Void, BridgeError>) -> Void)? = nil) {
        guard browserCanChange, let state, let model = state.modelling,
              let feature = model.timeline.first(where: { $0.id == id }) else {
            completion?(.failure(BridgeError(code: "disabled", message: "ui.ipados.workspace.finish_edit")))
            return
        }
        canvas.cancelInput(); selectedAssemblyRowID = nil
        if feature.order >= model.activeFeatureCount {
            let document = state.document_id
            moveHistory(to: feature.order + 1) { [weak self] result in
                guard let self else { return }
                guard case .success = result, self.state?.document_id == document else { completion?(result); return }
                self.session.selectEntity(["type": "feature", "id": id], additive: false, completion: completion)
            }
        } else { session.selectEntity(["type": "feature", "id": id], additive: false, completion: completion) }
    }

    private func showHistoryFeatureActions(_ id: UInt64, source: UIView) {
        guard browserCanChange, state?.sketch == nil, presentedViewController == nil,
              let context = state, let feature = context.modelling?.timeline.first(where: { $0.id == id }) else { return }
        historyActivation = nil
        canvas.cancelInput(); view.endEditing(true)
        let sheet = UIAlertController(title: feature.name, message: nil, preferredStyle: .actionSheet)
        let delete = { [weak self] in
            guard let self, self.browserCanChange, self.state?.document_id == context.document_id,
                  self.state?.geometry_revision == context.geometry_revision,
                  let current = self.state?.modelling?.timeline.first(where: { $0.id == id }) else { return }
            self.deleteFeature(current)
        }
#if DEBUG
        historyMenuDelete = delete
#endif
        sheet.addAction(UIAlertAction(title: ModellingText.text("delete_feature"), style: .destructive) { _ in delete() })
        sheet.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        sheet.popoverPresentationController?.sourceView = source
        sheet.popoverPresentationController?.sourceRect = source.bounds
        present(sheet, animated: true)
    }

    private func activateHistoryFeature(_ id: UInt64) {
        guard historyCanNavigate, presentedViewController == nil, let document = state?.document_id else { return }
        historyActivation = (id, document)
        selectHistoryFeature(id) { [weak self] result in
            guard let self, self.historyActivation?.feature == id, self.historyActivation?.document == document else { return }
            if case .failure = result { self.historyActivation = nil }
            else { self.session.refreshWorkspace() }
        }
    }

    private func openActivatedHistoryFeature() {
        guard let activation = historyActivation, let state else { return }
        guard state.document_id == activation.document,
              let feature = state.modelling?.timeline.first(where: { $0.id == activation.feature }) else { historyActivation = nil; return }
        guard historyCanNavigate, presentedViewController == nil,
              feature.order < (state.modelling?.activeFeatureCount ?? 0),
              state.selection.contains(where: { $0.type == "feature" && $0.id?.readout == String(activation.feature) }) else { return }
        historyActivation = nil
        if state.rows.first(where: { $0.id == "feature:" + String(feature.id) })?.kind == "sketch" {
            browserOpen = false; inspectorOpen = false
            run("sketch.edit", params: ["sketch": feature.id]); relayoutPanels()
        } else { beginModelling(feature: feature.id) }
    }

    private func browserName(_ row: WorkspaceRow) -> String {
        row.browser?.localised == true ? NativeText.value(row.label) : row.label
    }

    private func setBrowserVisibility(_ row: WorkspaceRow) {
        guard browserCanChange, let browser = row.browser, let visible = browser.visible,
              var params = browser.visibility?.mapValues(\.value) else { return }
        params["visible"] = !visible
        canvas.cancelInput()
        run("browser.visibility", params: params)
    }

    func tableView(_ tableView: UITableView, contextMenuConfigurationForRowAt indexPath: IndexPath, point: CGPoint) -> UIContextMenuConfiguration? {
        guard indexPath.section == 1, browserCanChange, visibleRows.indices.contains(indexPath.row) else { return nil }
        let row = visibleRows[indexPath.row]
        let document = state?.document_id, revision = state?.geometry_revision
        return UIContextMenuConfiguration(identifier: row.id as NSString, previewProvider: nil) { [weak self] _ in
            guard let self, let current = self.currentBrowserRow(row.id, document: document, revision: revision) else { return nil }
            return self.browserMenu(current)
        }
    }

    private func browserMenu(_ row: WorkspaceRow) -> UIMenu {
        guard let browser = row.browser else { return UIMenu(children: []) }
        let document = state?.document_id, revision = state?.geometry_revision
        func current(_ controller: FoundationController) -> WorkspaceRow? {
            controller.currentBrowserRow(row.id, document: document, revision: revision)
        }
        var actions: [UIMenuElement] = []
        if browser.rename != nil {
            actions.append(UIAction(title: BrowserText.text("rename"), image: WorkspaceIcon.image("rename", traits: traitCollection)) { [weak self] _ in
                guard let self, let row = current(self), let rename = row.browser?.rename else { return }
                self.askBrowserName(title: BrowserText.text("rename"), initial: row.label, command: rename.command,
                                    params: rename.params.mapValues(\.value), rowID: row.id)
            })
        }
        if let visible = browser.visible, browser.visibility != nil {
            actions.append(UIAction(title: BrowserText.text(visible ? "hide" : "show"), image: WorkspaceIcon.image("eye", traits: traitCollection)) { [weak self] _ in
                guard let self, let row = current(self) else { return }
                self.setBrowserVisibility(row)
            })
        }
        if let folder = browser.folder, let component = browser.component {
            var params: [String: Any] = ["component": component, "folder": folder]
            if let key = browser.key { params["items"] = [key] }
            let groupParams = params
            actions.append(UIAction(title: BrowserText.text(browser.key == nil ? "new_folder" : "new_folder_with_item"), image: WorkspaceIcon.image("folder", traits: traitCollection)) { [weak self] _ in
                guard let self, current(self) != nil else { return }
                self.askBrowserName(title: BrowserText.text("new_folder"), initial: "", command: "browser.group", params: groupParams, rowID: row.id)
            })
            if let key = browser.key {
                let groups = state?.rows.filter { $0.kind == "folder" && $0.browser?.component == component && $0.browser?.folder == folder && $0.browser?.group != nil } ?? []
                var seen: Set<UInt64> = []
                var destinations: [UIMenuElement] = [UIAction(title: BrowserText.text("outside_folders"), state: browser.group == nil ? .on : .off) { [weak self] _ in
                    guard let self, current(self) != nil else { return }
                    self.run("browser.move", params: ["component": component, "folder": folder, "items": [key]])
                }]
                for group in groups {
                    guard let id = group.browser?.group, seen.insert(id).inserted else { continue }
                    destinations.append(UIAction(title: browserName(group), state: browser.group == id ? .on : .off) { [weak self] _ in
                        guard let self, current(self) != nil, self.rowByID[group.id]?.browser?.group == id else { return }
                        self.run("browser.move", params: ["component": component, "folder": folder, "items": [key], "group": id])
                    })
                }
                actions.append(UIMenu(title: BrowserText.text("move_to_folder"), image: WorkspaceIcon.image("folder", traits: traitCollection), children: destinations))
            } else if let group = browser.group {
                actions.append(UIAction(title: BrowserText.text("remove_folder")) { [weak self] _ in
                    guard let self, current(self) != nil else { return }
                    self.run("browser.ungroup", params: ["group": group])
                })
            }
        }
        return UIMenu(title: browserName(row), children: actions)
    }

    private func askBrowserName(title: String, initial: String, command: String, params: [String: Any], rowID: String) {
        guard browserCanChange, presentedViewController == nil, let document = state?.document_id else { return }
        let revision = state?.geometry_revision
        browserSearchField.resignFirstResponder()
        canvas.cancelInput()
        let alert = UIAlertController(title: title, message: nil, preferredStyle: .alert)
        alert.addTextField { field in
            field.text = initial; field.placeholder = BrowserText.text("name")
            field.autocapitalizationType = .sentences
            field.accessibilityIdentifier = "browser-name"
        }
        func valid(_ text: String) -> Bool {
            let value = text.trimmingCharacters(in: .whitespacesAndNewlines)
            return !value.isEmpty && value.utf8.count <= 128
        }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        let save = UIAlertAction(title: BrowserText.text("save"), style: .default) { [weak self, weak alert] _ in
            guard let self, self.currentBrowserRow(rowID, document: document, revision: revision) != nil,
                  let name = alert?.textFields?.first?.text?.trimmingCharacters(in: .whitespacesAndNewlines), valid(name) else { return }
            var values = params; values["name"] = name
            self.run(command, params: values)
        }
        save.isEnabled = valid(initial)
        alert.textFields?.first?.addAction(UIAction { [weak save] action in
            guard let field = action.sender as? UITextField else { return }
            save?.isEnabled = valid(field.text ?? "")
        }, for: .editingChanged)
        alert.addAction(save)
        present(alert, animated: true)
    }

    private func originVisible(_ item: String) -> Bool {
        viewPresentation?.originVisible(item) ?? (originVisibility[item] ?? planeChoosing)
    }

    private var referencesLocked: Bool {
        numericEditing || modellingEditing || planeConfirming || !pending.isEmpty || viewControlPending || viewPresentation == nil
    }

    private func setOriginVisibility(item: String? = nil, group: String? = nil, visible: Bool?) {
        guard !referencesLocked else { return }
        var params: [String: Any] = ["visible": NSNull()]
        if let visible { params["visible"] = visible }
        if let item { params["item"] = item }
        if let group { params["group"] = group }
        let items = group == "axes" ? Self.originAxes : (group == "planes" ? Self.originPlanes : item.map { [$0] } ?? Self.originItems)
        viewControlPending = true; rebuildViewMenu()
        canvas.cancelInput()
        session.command("view.origin", params: params) { [weak self] result in
            guard let self else { return }
            self.viewControlPending = false
            if case .success = result {
                for item in items { self.originVisibility[item] = visible }
                UserDefaults.standard.set(self.originVisibility, forKey: "SolidCraft.workspace.originVisibility")
            }
            self.table.reloadData(); self.rebuildViewMenu()
        }
    }

    private func toggleOriginReferences() {
        guard let view = viewPresentation else { return }
        setOriginVisibility(visible: view.masterTarget)
    }

    private func toggleGrid() {
        guard !referencesLocked else { return }
        let visible = !(viewPresentation?.grid.visible ?? gridVisible)
        viewControlPending = true; rebuildViewMenu()
        session.command("view.grid", params: ["visible": visible]) { [weak self] result in
            guard let self else { return }
            self.viewControlPending = false
            if case .success = result {
                self.gridVisible = visible
                UserDefaults.standard.set(visible, forKey: "SolidCraft.workspace.gridVisible")
            }
            self.rebuildViewMenu()
        }
    }

    private func toggleIntegratedGridAxes() {
        guard !referencesLocked else { return }
        let enabled = !(viewPresentation?.grid.integrated_axes ?? integratedGridAxes)
        viewControlPending = true; rebuildViewMenu()
        canvas.cancelInput()
        session.command("view.grid", params: ["visible": viewPresentation?.grid.visible ?? gridVisible, "integrated_axes": enabled]) { [weak self] result in
            guard let self else { return }
            self.viewControlPending = false
            if case .success = result {
                self.integratedGridAxes = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.integratedGridAxes")
            }
            self.table.reloadData(); self.rebuildViewMenu()
        }
    }

    private func togglePerspective() {
        guard !numericEditing, !modellingCommitting, pending.isEmpty, !viewControlPending, state?.sketch == nil else { return }
        let enabled = !(viewPresentation.map { $0.camera.fov > 0 } ?? perspective)
        viewControlPending = true; rebuildViewMenu()
        session.command("view.perspective", params: ["enabled": enabled]) { [weak self] _ in
            guard let self else { return }
            self.viewControlPending = false; self.rebuildViewMenu()
        }
    }

    private func visibilityMenu() -> UIMenu {
        let attributes: UIMenuElement.Attributes = referencesLocked ? .disabled : []
        var actions: [UIMenuElement] = [true, false].map { visible in
            UIAction(title: SketchText.text(visible ? "show_origin" : "hide_origin"), attributes: attributes) { [weak self] _ in
                self?.setOriginVisibility(visible: visible)
            }
        }
        actions.append(UIAction(title: SketchText.text("automatic_origin"), attributes: attributes,
                                state: originVisibility.isEmpty ? .on : .off) { [weak self] _ in self?.setOriginVisibility(visible: nil) })
        func reference(_ item: String) -> UIAction {
            UIAction(title: item == "O" ? SketchText.text("origin_point") : item, attributes: attributes,
                     state: originVisible(item) ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.setOriginVisibility(item: item, visible: !self.originVisible(item))
            }
        }
        func references(_ group: String, items: [String]) -> UIAction {
            let allVisible = items.allSatisfy(originVisible)
            let state: UIMenuElement.State = allVisible ? .on : (items.contains(where: originVisible) ? .mixed : .off)
            return UIAction(title: WorkspaceStyle.text("all_" + group), attributes: attributes, state: state) { [weak self] _ in
                guard let self else { return }
                self.setOriginVisibility(group: group, visible: !items.allSatisfy(self.originVisible))
            }
        }
        let browser = UIAction(title: WorkspaceStyle.text("origin_in_model"), state: originInBrowser ? .on : .off) { [weak self] _ in
            self?.toggleOriginInModel()
        }
        return UIMenu(title: ViewText.text("individual_references"), children: [
            UIMenu(options: .displayInline, children: [reference("O")]),
            UIMenu(title: WorkspaceStyle.text("axes"), options: .displayInline,
                   children: [references("axes", items: Self.originAxes)] + Self.originAxes.map(reference)),
            UIMenu(title: WorkspaceStyle.text("planes"), options: .displayInline,
                   children: [references("planes", items: Self.originPlanes)] + Self.originPlanes.map(reference)),
            UIMenu(options: .displayInline, children: actions),
            UIMenu(options: .displayInline, children: [browser])
        ])
    }

    private func showViewControls(animated: Bool = true) {
        guard viewButton.isEnabled, presentedViewController == nil else { return }
        let panel = WorkspaceViewPopover()
        panel.overrideUserInterfaceStyle = workspaceInterfaceStyle
        panel.onCommand = { [weak self] command in self?.runViewCommand(command) }
        panel.onPerspective = { [weak self] in self?.togglePerspective() }
        panel.onGrid = { [weak self] in self?.toggleGrid() }
        panel.onIntegratedAxes = { [weak self] in self?.toggleIntegratedGridAxes() }
        panel.onOrigin = { [weak self] in self?.toggleOriginReferences() }
        panel.onReference = { [weak self] item, group, visible in self?.setOriginVisibility(item: item, group: group, visible: visible) }
        panel.onLegacyPlanes = { [weak self] in self?.toggleLegacyOriginPlanes() }
        panel.onAxisLabels = { [weak self] in
            guard let self else { return }
            self.axisLabelsVisible.toggle()
            UserDefaults.standard.set(self.axisLabelsVisible, forKey: "SolidCraft.workspace.axisLabels")
            self.showReferenceLabels(self.latestReferenceLabels)
            self.rebuildViewMenu()
        }
        panel.onOriginInModel = { [weak self] in self?.toggleOriginInModel() }
        if let popover = panel.popoverPresentationController {
            popover.sourceView = viewButton; popover.sourceRect = viewButton.bounds
            popover.permittedArrowDirections = .up; popover.delegate = self
        }
        viewControlsPopover = panel
        rebuildViewMenu()
        present(panel, animated: animated)
    }

    private func toggleLegacyOriginPlanes() {
        guard !referencesLocked else { return }
        let enabled = !legacyOriginPlanes
        viewControlPending = true; rebuildViewMenu()
        session.command("view.origin_style", params: ["legacy": enabled]) { [weak self] result in
            guard let self else { return }
            self.viewControlPending = false
            if case .success = result {
                self.legacyOriginPlanes = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.legacyOriginPlanes")
            }
            self.rebuildViewMenu()
        }
    }

    private func toggleOriginInModel() {
        originInBrowser.toggle()
        UserDefaults.standard.set(originInBrowser, forKey: "SolidCraft.workspace.originInBrowser")
        updateRows(); rebuildMenus()
    }

    private var viewCameraLocked: Bool {
        numericEditing || modellingCommitting || !pending.isEmpty || viewControlPending
    }

    private func runViewCommand(_ command: String) {
        guard !viewCameraLocked else { return }
        viewControlPending = true; rebuildViewMenu()
        session.command(command) { [weak self] _ in
            guard let self else { return }
            self.viewControlPending = false; self.rebuildViewMenu()
        }
    }

    private func rebuildViewMenu() {
        // Only View uses this panel. The native tool/browser menus are untouched.
        viewButton.menu = nil; viewButton.showsMenuAsPrimaryAction = false
        homeButton.availability(enabled: !viewCameraLocked && (!homeToolbar || (state?.sketch == nil && viewPresentation?.planar != true)))
        viewControlsPopover?.render(viewPresentation, cameraEnabled: !viewCameraLocked,
            referencesEnabled: !referencesLocked, inSketch: state?.sketch != nil,
            automatic: originVisibility.isEmpty, originInModel: originInBrowser, legacyPlanes: legacyOriginPlanes,
            axisLabels: axisLabelsVisible)
    }

    private func originCell(_ cell: UITableViewCell, row: Int) -> UITableViewCell {
        let item = Self.originItems.indices.contains(row - 1) ? Self.originItems[row - 1] : nil
        let visible = item.map(originVisible) ?? Self.originItems.contains(where: originVisible)
        var content = cell.defaultContentConfiguration()
        content.text = item.map { $0 == "O" ? SketchText.text("origin_point") : $0 } ?? SketchText.text("origin_references")
        content.textProperties.font = .preferredFont(forTextStyle: item == nil ? .headline : .body)
        content.textProperties.numberOfLines = 0
        content.secondaryTextProperties.color = WorkspaceStyle.secondary
        content.directionalLayoutMargins = NSDirectionalEdgeInsets(top: 10, leading: item == nil ? 16 : 40, bottom: 10, trailing: 8)
        content.image = WorkspaceIcon.image(item == nil ? "origin" : (item == "O" ? "point" : (item?.count == 2 ? "plane" : "axis")), traits: traitCollection, size: 24)
        content.secondaryText = nil
        cell.contentConfiguration = content
        var background = UIBackgroundConfiguration.listPlainCell()
        background.backgroundColor = .clear
        cell.backgroundConfiguration = background
        let label = item == nil ? SketchText.text("origin_references") : SketchText.text(visible ? "hide_reference" : "show_reference")
        let eye = WorkspaceButton(label) { [weak self] in
            guard let self, let item else { return }; self.setOriginVisibility(item: item, visible: !self.originVisible(item))
        }
        eye.artwork("eye", compact: true); eye.active(visible)
        eye.availability(enabled: !numericEditing && !modellingEditing && !planeConfirming && pending.isEmpty)
        eye.frame = CGRect(x: 0, y: 0, width: 44, height: 44)
        eye.accessibilityIdentifier = "origin-visibility-" + (item ?? "all")
        eye.accessibilityLabel = label + (item.map { ": " + ($0 == "O" ? SketchText.text("origin_point") : $0) } ?? "")
        let visibilityReadout = SketchText.text(visible ? "reference_visible" : "reference_hidden")
        eye.accessibilityValue = visibilityReadout
        if item == nil { eye.menu = visibilityMenu(); eye.showsMenuAsPrimaryAction = true }
        cell.accessoryType = .none
        let controls = UIStackView(arrangedSubviews: [eye])
        eye.widthAnchor.constraint(equalToConstant: BrowserRowColumns.touch).isActive = true
        if item == nil {
            let expand = WorkspaceButton(WorkspaceStyle.text(originExpanded ? "close_group" : "open_group")) { [weak self] in
                guard let self, !self.browserSearching else { return }; self.originExpanded.toggle(); self.updateRows()
            }
            expand.artwork(originExpanded || browserSearching ? "chevron" : "chevron_right", compact: true)
            expand.availability(enabled: !browserSearching)
            expand.widthAnchor.constraint(equalToConstant: BrowserRowColumns.touch).isActive = true
            controls.addArrangedSubview(expand)
        } else { controls.addArrangedSubview(browserEmptyControl()) }
        controls.frame = CGRect(x: 0, y: 0, width: BrowserRowColumns.width, height: BrowserRowColumns.touch)
        cell.accessoryView = controls
        cell.accessibilityLabel = content.text; cell.accessibilityValue = visibilityReadout
        cell.accessibilityHint = item == nil ? WorkspaceStyle.text(originExpanded ? "close_group" : "open_group") : nil
        cell.accessibilityTraits = .button; cell.selectionStyle = .none
        cell.accessibilityIdentifier = "workspace-origin-" + (item ?? "group")
        return cell
    }

    private func rebuildInspector() {
        guard !numericEditing, !modellingEditing else { return }
        inspectorPanel.setTitle(WorkspaceStyle.text("inspector")); inspectorPanel.setFooter(nil)
        properties.arrangedSubviews.forEach { properties.removeArrangedSubview($0); $0.removeFromSuperview() }
        guard let state else {
            let loading = WorkspaceMessage(); loading.show(WorkspaceStyle.text("loading"), busy: true); properties.addArrangedSubview(loading); return
        }
        if !lastOperationFeedback.isEmpty {
            let message = WorkspaceMessage(); message.show(lastOperationFeedback.map(ModellingText.label).joined(separator: "\n")); properties.addArrangedSubview(message)
        }
        if let assembly = state.assembly,
           !compactChrome || diagnostics || !assembly.active_path.isEmpty ||
            state.selection.contains(where: { $0.type == "occurrence" }) ||
            selectedAssemblyRowID.flatMap({ rowByID[$0]?.assembly }) != nil {
            addAssemblyContext(assembly)
        }
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
        if planeChoosing {
            let hint = WorkspaceStyle.label(.body, secondary: true)
            hint.text = SketchText.text("tap_plane")
            properties.addArrangedSubview(hint)
            properties.addArrangedSubview(WorkspaceButton(WorkspaceStyle.text("cancel")) { [weak self] in self?.cancelSketchPlane() })
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
        } else if !compactChrome || diagnostics, let reason = state.tools.first(where: { $0.id == "post_radius" })?.reason {
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
        if let modelling = state.modelling, !modelling.parameters.isEmpty, !compactChrome || diagnostics || selection.isEmpty {
            let heading = WorkspaceStyle.label(.headline); heading.text = ModellingText.text("parameters"); properties.addArrangedSubview(heading)
            for parameter in modelling.parameters {
                let readout = [parameter.expression, parameter.value.map { String($0) + " " + parameter.unit }].compactMap { $0 }.joined(separator: " · ")
                properties.addArrangedSubview(WorkspacePropertyRow(label: parameter.name, value: readout))
                properties.addArrangedSubview(WorkspaceButton(ModellingText.text("edit_parameter") + ": " + parameter.name) { [weak self] in
                    self?.beginModelling(operation: "parameters.change", initialParams: ["name": .string(parameter.name), "expression": .string(parameter.expression)])
                })
            }
        }
        if state.modelling?.tools.contains(where: { $0.id == "parameters.add" }) == true, !compactChrome || diagnostics || selection.isEmpty {
            properties.addArrangedSubview(WorkspaceButton(ModellingText.text("add_parameter")) { [weak self] in self?.beginModelling(operation: "parameters.add") })
        }
        let guidance = WorkspaceStyle.label(.footnote, secondary: true)
        guidance.text = WorkspaceStyle.text(state.active_tool == "post_radius" ? "radius_hint" : "select_hint")
        properties.addArrangedSubview(guidance)
    }

    private func addFeatureInspector(_ feature: ModellingPresentation.Feature) {
        func action(_ label: String, command: String, prominent: Bool = false, perform: @escaping () -> Void) {
            let button = WorkspaceButton(label, prominent: prominent, action: perform)
            button.commandArtwork(command)
            properties.addArrangedSubview(button)
        }
        let heading = WorkspaceStyle.label(.headline); heading.text = feature.name; properties.addArrangedSubview(heading)
        properties.addArrangedSubview(WorkspacePropertyRow(label: ModellingText.text("feature_type"), value: ModellingText.label(feature.kind)))
        if let error = feature.error {
            let message = WorkspaceMessage(); message.show(error, error: true); properties.addArrangedSubview(message)
        }
        if let warning = feature.warning { let message = WorkspaceMessage(); message.show(warning); properties.addArrangedSubview(message) }
        if feature.stale == true { let message = WorkspaceMessage(); message.show(ModellingText.text("stale_geometry"), error: true); properties.addArrangedSubview(message) }
        if state?.rows.first(where: { $0.id == "feature:" + String(feature.id) })?.kind != "sketch" {
            action(ModellingText.text("edit_feature"), command: "timeline.edit", prominent: true) { [weak self] in self?.beginModelling(feature: feature.id) }
        }
        action(ModellingText.text("rename"), command: "timeline.rename") { [weak self] in self?.renameFeature(feature) }
        action(ModellingText.text("dependencies"), command: "timeline.dependents") { [weak self] in self?.showFeatureDependencies(feature) }
        action(ModellingText.text(feature.suppressed ? "unsuppress" : "suppress"), command: "timeline.suppress") { [weak self] in
            self?.run("timeline.suppress", params: ["feature": feature.id, "suppressed": !feature.suppressed])
        }
        action(ModellingText.text("delete_feature"), command: "timeline.delete") { [weak self] in self?.deleteFeature(feature) }
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
        if compactChrome {
            subtitle.text = state?.sketch.map { SketchText.text("draw") + " · " + $0.name } ?? documentStatus
            subtitle.accessibilityLabel = subtitle.text
            return
        }
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
            if let command = action.command ?? action.operation { button.commandArtwork(command) }
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
                let confirm = { [weak self] in
                    guard let self, self.state?.document_id == context.document_id, self.state?.geometry_revision == context.geometry_revision else { return }
                    self.run("timeline.delete", params: ["feature": feature.id])
                }
#if DEBUG
                self.historyDeleteConfirmation = confirm
#endif
                alert.addAction(UIAlertAction(title: ModellingText.text("delete_feature"), style: .destructive) { _ in confirm() })
                self.canvas.cancelInput(); self.present(alert, animated: true)
            }
        }
    }

    private func beginModelling(operation: String? = nil, feature: UInt64? = nil, initialParams: [String: PresentationJSON]? = nil, fromSelection: Bool = false) {
        guard !numericEditing, !modellingEditing, let context = state else { return }
        historyActivation = nil
        canvas.cancelInput(); modellingGeneration &+= 1; lastOperationFeedback = []
        let expected = modellingGeneration
        modellingStartedFromSelection = fromSelection
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
                let form = ModellingInspector(state: model, compact: self.compactInspector)
                self.modellingInspector = form
                self.inspectorPanel.setTitle(form.compact ? form.title : WorkspaceStyle.text("inspector"))
                self.inspectorPanel.setFooter(form.compact ? form.footer : nil)
                self.inspectorScroll.setContentOffset(.zero, animated: false)
                self.properties.arrangedSubviews.forEach { self.properties.removeArrangedSubview($0); $0.removeFromSuperview() }
                self.properties.addArrangedSubview(form)
                form.onPreview = { [weak self] params in self?.evaluateModelling(params, commit: false) }
                form.onLivePreview = { [weak self] params in self?.evaluateModelling(params, commit: false, live: true) }
                form.onContextChange = { [weak self] params in self?.refreshModellingContext(params) }
                form.onInlineChoice = { [weak self] params in self?.refreshModellingContext(params, previewAfter: true) }
                form.onApply = { [weak self] params in self?.evaluateModelling(params, commit: true) }
                form.onCancel = { [weak self] in self?.cancelModelling() }
                form.onFocus = { [weak self] in self?.canvas.cancelInput(); self?.revealField(); self?.updateAvailability() }
                form.onEditValue = form.compact ? { [weak self] field, expression, source in
                    self?.presentModelValue(parameter: field.key, expression: expression, units: field.units ?? "",
                        name: ModellingText.label(field.label), source: source)
                } : nil
                form.onChange = { [weak self] in self?.failure = nil; self?.updateStatus(); self?.layoutModelValue() }
                form.onSelect = { [weak self, weak form] field in
                    guard let self, let form else { return }
                    form.beginContextRefresh()
                    self.session.requestModelling(action: "inspect", token: token, params: form.params) { [weak self, weak form] result in
                        guard let self, self.modellingToken == token else { return }
                        switch result {
                        case .failure(let error): form?.failed(error)
                        case .success(let model):
                            form?.contextRefreshed(model)
                            let selectionParams: [String: PresentationJSON] = field.map { ["field": .string($0)] } ?? [:]
                            // Explicit picking continues this draft after the adapter clears
                            // the original source selection. It is no longer an automatic popup.
                            let automatic = self.modellingStartedFromSelection
                            self.modellingStartedFromSelection = false
                            self.session.requestModelling(action: "selection", token: token, params: selectionParams) { [weak self] result in
                                guard let self, self.modellingToken == token else { return }
                                if case .failure(let error) = result {
                                    self.modellingStartedFromSelection = automatic
                                    self.showFailure(error); return
                                }
                                self.inspectorOpen = false; self.browserOpen = false; self.relayoutPanels()
                            }
                        }
                    }
                }
                self.inspectorOpen = true; self.browserOpen = false; self.failure = nil
                self.relayoutPanels()
#if DEBUG
                if ProcessInfo.processInfo.arguments.contains("--auto-face-extrude-check"),
                   operation == "solid.extrude", feature == nil {
                    NSLog("Automatic face extrude check PASS: selected face opened the production Extrude form; source=%@", String(describing: model.params?["profile"]))
                }
#endif
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
        let valueEditor = modelValueEditor?.editor
        let valueRevision = valueEditor?.draft.revision
        valueEditor?.setBusy(true, committing: commit)
        modellingCommitting = commit
        form.setBusy(committing: commit, allowInput: live || valueEditor != nil); updateAvailability()
        session.requestModelling(action: commit ? "commit" : "preview", token: token, params: params) { [weak self, weak form] result in
            guard let self, self.modellingGeneration == expected, self.modellingToken == token else { return }
            self.modellingCommitting = false
            let queuedPreview = self.modellingPreviewQueue.complete()
            guard self.modellingFormVersion == expectedForm else { return }
            switch result {
            case .failure(let error): form?.failed(error); self.failure = error.message
            case .success(let model):
                if commit { self.lastOperationFeedback = model.feedback ?? []; self.clearModelling(); self.rebuildInspector() }
                else {
                    form?.evaluated(model)
                    if let valueRevision, valueEditor === self.modelValueEditor?.editor {
                        valueEditor?.completePreview(revision: valueRevision,
                            valid: model.valid == true && model.params == form?.params && queuedPreview == nil,
                            value: model.handle?.distance, units: model.handle?.units ?? "",
                            feedback: ([WorkspaceStyle.text("preview_ready")] + (model.feedback ?? []).map(ModellingText.label)).joined(separator: "\n"))
                    }
                    self.updateModelValueChoices()
                }
                self.failure = nil
            }
            self.updateAvailability(); self.updateStatus()
            if case .failure(let error) = result, let valueRevision, valueEditor === self.modelValueEditor?.editor {
                valueEditor?.completePreview(revision: valueRevision, error: error.message)
                self.updateModelValueChoices()
            }
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
        modelValueEditor?.editor.setBusy(true)
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
                    } else {
                        self.modelValueEditor?.editor.setBusy(false)
                    }
                case .failure(let error):
                    form?.failed(error); self.previewAfterModellingContext = false
                    self.modelValueEditor?.editor.setBusy(false)
                    self.modelValueEditor?.editor.message.show(error.message, error: true)
                }
            } else if self.pendingModellingContext == nil {
                // Text typed after the context request remains provisional. The
                // next explicit Preview refreshes those complete current fields;
                // typing itself never sends a new engine request.
                form?.contextRefreshDiscarded(); self.previewAfterModellingContext = false
                self.modelValueEditor?.editor.setBusy(false)
            }
            if self.pendingModellingContext != nil { self.drainModellingContext() }
            self.updateAvailability()
        }
    }

    private func cancelModelling(alreadyDismissed: Bool = false) {
        guard !modellingCommitting || alreadyDismissed, modellingEditing else { return }
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
        modellingStartedFromSelection = false
        modellingGeneration &+= 1
        if let editor = modelValueEditor { editor.dismiss(animated: false) { [weak self] in self?.updateAvailability() } }
        modelValueEditor = nil; modelValue = nil; modelValueButton.isHidden = true
        modellingInspector?.endEditing(true); modellingInspector = nil
        inspectorPanel.setFooter(nil); inspectorPanel.setTitle(WorkspaceStyle.text("inspector"))
        previewStale = false
        modellingToken = nil; modellingFeature = nil; modellingContext = nil; modellingStarting = false
        modellingCommitting = false; modellingInspectPending = false
        modellingPreviewQueue.clear()
        modellingFormVersion &+= 1; modellingContextInspectInFlight = false
        pendingModellingContext = nil; modellingContextNeedsRefresh = false; previewAfterModellingContext = false
        table.reloadData()
    }

    private func layoutModelValue() {
        guard directHandles, active, let value = modelValue, value.token == modellingToken,
              value.position.count == 2, value.position.allSatisfy(\.isFinite), value.value.isFinite,
              let form = modellingInspector else { modelValueButton.isHidden = true; return }
        let formatter = NumberFormatter()
        formatter.numberStyle = .decimal; formatter.maximumFractionDigits = 4
        formatter.usesGroupingSeparator = false
        let text = (formatter.string(from: NSNumber(value: value.value)) ?? String(value.value)) + " " + value.units
        modelValueButton.updateTitle(text)
        var config = modelValueButton.configuration
        config?.titleLineBreakMode = .byTruncatingTail
        config?.baseForegroundColor = WorkspaceStyle.accent
        modelValueButton.configuration = config
        modelValueButton.accessibilityLabel = HandleText.text("edit_value")
        modelValueButton.accessibilityValue = value.expression
        modelValueButton.accessibilityHint = HandleText.text("entry_hint")
        let anchor = CGPoint(x: canvas.frame.minX + value.position[0], y: canvas.frame.minY + value.position[1])
        let fitting = modelValueButton.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize)
        let size = CGSize(width: min(220, max(80, fitting.width)), height: max(44, fitting.height))
        let obstacles = [top, railScroll, navigationScroll, historyActions, featureHistory, browserButton, status, browserPanel, inspectorPanel]
            .filter { !$0.isHidden }.map { $0.frame.insetBy(dx: -6, dy: -6) }
        let safe = workspace.convert(view.safeAreaLayoutGuide.layoutFrame, from: view)
        let bounds = WorkspaceLayout.chromeBounds(safeArea: safe, viewport: canvas.frame, useBottomSafeArea: useBottomSafeArea)
            .intersection(canvas.frame).insetBy(dx: 8, dy: 8)
        guard let frame = ModelValuePlacement.resolve(anchor: anchor, size: size, bounds: bounds, obstacles: obstacles) else {
            modelValueButton.isHidden = true; return
        }
        modelValueButton.frame = frame
        modelValueButton.isHidden = false
        modelValueButton.isEnabled = form.handleEntryEnabled && !form.hasLocalChanges && presentedViewController == nil
    }

    private func editModelValue() {
        guard let value = modelValue, value.token == modellingToken, let form = modellingInspector,
              form.handleEntryEnabled, !form.hasLocalChanges, presentedViewController == nil else { return }
        let name = form.fields.first(where: { $0.key == value.parameter }).map { ModellingText.label($0.label) } ?? HandleText.text("edit_value")
        presentModelValue(parameter: value.parameter, expression: value.expression, units: value.units, name: name, source: modelValueButton)
    }

    private func presentModelValue(parameter: String, expression: String, units: String, name: String, source: UIView) {
        guard let token = modellingToken, let form = modellingInspector, form.handleEntryEnabled,
              presentedViewController == nil else { return }
        canvas.cancelInput(); form.endEditing(true)
        let controller = NumericValueEditorController(expression: expression, units: units, name: name,
            context: form.numericContext, parameter: parameter)
        controller.overrideUserInterfaceStyle = workspaceInterfaceStyle
        let expected = modellingGeneration
        controller.editor.onChange = { [weak self, weak controller, weak form] in
            guard let self, self.modellingGeneration == expected, self.modellingToken == token,
                  let controller, let form else { return }
            form.editExpression(parameter, expression: controller.editor.field.text ?? "")
        }
        let submit: (String, Bool) -> Void = { [weak self, weak controller, weak form] expression, commit in
            guard let self, self.modellingGeneration == expected, self.modellingToken == token, let form else { return }
            form.editExpression(parameter, expression: expression)
            controller?.editor.setBusy(true, committing: commit)
            self.evaluateModelling(form.params, commit: commit)
        }
        controller.editor.onPreview = { submit($0, false) }
        controller.editor.onApply = { submit($0, true) }
        controller.editor.onCancel = { [weak self] in self?.cancelModelling() }
        if let popover = controller.popoverPresentationController {
            // The source field may be rebuilt as its form updates. Retain the
            // exact original on-screen anchor in the stable workspace instead.
            popover.sourceView = workspace; popover.sourceRect = source.convert(source.bounds, to: workspace)
            popover.permittedArrowDirections = [.up, .down, .left, .right]; popover.delegate = self
        }
        modelValueEditor = controller
        updateModelValueChoices()
        present(controller, animated: true) { [weak self] in self?.updateAvailability() }
    }

    private func updateModelValueChoices() {
        guard let controller = modelValueEditor, let form = modellingInspector else { return }
        let field = form.fields.first { $0.kind == "inline_choice" }
        let selected = field.flatMap { PresentationJSON.parameter($0.key, in: form.params) }
        controller.setInlineChoices(field, selected: selected, enabled: form.handleEntryEnabled) { [weak form, weak controller] value in
            guard let form, let controller, let field, form.handleEntryEnabled else { return }
            if let parameter = controller.parameter {
                form.editExpression(parameter, expression: controller.editor.field.text ?? "")
            }
            controller.editor.invalidatePreview()
            form.selectInlineChoice(field.key, value: value)
        }
    }

    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        if presentationController.presentedViewController === viewControlsPopover {
            viewControlsPopover = nil
            return
        }
        if presentationController.presentedViewController === inlineNumericEditor {
            inlineNumericEditor = nil
            cancelNumeric()
            return
        }
        modelValueEditor = nil; updateAvailability(); layoutModelValue()
    }

    func popoverPresentationControllerDidDismissPopover(_ popoverPresentationController: UIPopoverPresentationController) {
        presentationControllerDidDismiss(popoverPresentationController)
    }

    func presentationControllerShouldDismiss(_ presentationController: UIPresentationController) -> Bool {
        if presentationController.presentedViewController === modelValueEditor { return !modellingCommitting }
        return presentationController.presentedViewController !== inlineNumericEditor || editor?.busy != true
    }

    func popoverPresentationControllerShouldDismissPopover(_ popoverPresentationController: UIPopoverPresentationController) -> Bool {
        presentationControllerShouldDismiss(popoverPresentationController)
    }

    func adaptivePresentationStyle(for controller: UIPresentationController, traitCollection: UITraitCollection) -> UIModalPresentationStyle {
        if controller.presentedViewController is NumericValueEditorController,
           traitCollection.horizontalSizeClass == .compact { return .pageSheet }
        return .none
    }

    private func localDocument(reopen: Bool) {
        guard !numericEditing, !modellingEditing else { return }
        if reopen { showProjects(); return }
        canvas.cancelInput()
        session.saveProject { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.showProjectFailure(error)
            case .success: self.documentStatus = ProjectText.text("saved"); self.updateSubtitle()
            }
        }
    }

    private func projectMenuActions() -> [UIMenuElement] {
        let attributes: UIMenuElement.Attributes = numericEditing || modellingEditing || !pending.isEmpty ? .disabled : []
        return [
            UIAction(title: ProjectText.text("title"), image: WorkspaceIcon.image("folder", traits: traitCollection), attributes: attributes) { [weak self] _ in self?.showProjects() },
            UIAction(title: ProjectText.text("save"), image: WorkspaceIcon.image("save", traits: traitCollection), attributes: attributes) { [weak self] _ in self?.localDocument(reopen: false) },
            UIAction(title: ProjectText.text("import"), attributes: attributes) { [weak self] _ in self?.chooseProjectImport() },
            UIMenu(title: ProjectText.text("export"), children: ["step", "solidcraft", "stl", "obj", "3mf"].map { format in
                UIAction(title: ProjectText.text("export_" + (format == "solidcraft" ? "native" : format)), attributes: attributes) { [weak self] _ in self?.exportProject(format: format) }
            })
        ]
    }

    private func showProjects() {
        guard !numericEditing, !modellingEditing, presentedViewController == nil, pending.isEmpty else { return }
        canvas.cancelInput(); session.command("tool.cancel")
        if planeChoosing || planeDraft != nil { cancelSketchPlane() }
        let browser = ProjectBrowserController()
        let navigation = UINavigationController(rootViewController: browser)
        navigation.modalPresentationStyle = .formSheet
        navigation.preferredContentSize = CGSize(width: 680, height: 720)
        projectsBrowser = browser
        browser.onClose = { [weak browser] in browser?.dismiss(animated: true) }
        browser.onOpen = { [weak self, weak browser] project in
            guard let self, let browser else { return }; browser.setBusy(true)
            self.session.openProject(project) { [weak self, weak browser] result in self?.finishProjectChange(result, browser: browser) }
        }
        browser.onCreate = { [weak self, weak browser] name, source in
            guard let self, let browser else { return }; browser.setBusy(true)
            self.session.createProject(name: name, copying: source) { [weak self, weak browser] result in self?.finishProjectChange(result, browser: browser) }
        }
        browser.onDelete = { [weak self, weak browser] project in
            guard let self, let browser else { return }; browser.setBusy(true)
            self.session.deleteProject(project) { [weak self, weak browser] result in
                guard let self, let browser else { return }
                switch result {
                case .failure(let error): browser.showError(error)
                case .success: self.loadProjects(browser)
                }
            }
        }
        browser.onImport = { [weak self, weak browser] in
            browser?.dismiss(animated: true) { [weak self] in self?.chooseProjectImport() }
        }
        present(navigation, animated: true) { [weak self, weak browser] in if let browser { self?.loadProjects(browser) } }
    }

    private func loadProjects(_ browser: ProjectBrowserController) {
        browser.setBusy(true)
        session.listProjects { [weak self, weak browser] result in
            guard let self, let browser else { return }; browser.setBusy(false)
            switch result {
            case .success(let projects): browser.display(projects, current: self.session.currentProject?.id)
            case .failure(let error): browser.showError(error)
            }
        }
    }

    private func finishProjectChange(_ result: Result<Void, BridgeError>, browser: ProjectBrowserController?) {
        browser?.setBusy(false)
        switch result {
        case .failure(let error): if let browser { browser.showError(error) } else { showProjectFailure(error) }
        case .success:
            failure = nil; localSaveLabel = ProjectText.text("saved")
            browser?.dismiss(animated: true)
            session.command("view.fit")
        }
    }

    private func showProjectFailure(_ error: BridgeError) {
        showFailure(error)
        guard presentedViewController == nil else { return }
        let alert = UIAlertController(title: ProjectText.text("error"), message: error.description, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("close"), style: .default))
        present(alert, animated: true)
    }

    private func chooseProjectImport() {
        guard !numericEditing, !modellingEditing, presentedViewController == nil, pending.isEmpty else { return }
        canvas.cancelInput(); session.command("tool.cancel")
        let alert = UIAlertController(title: ProjectText.text("import"), message: ProjectText.text("import_choice"), preferredStyle: .alert)
        for insert in [false, true] {
            alert.addAction(UIAlertAction(title: ProjectText.text(insert ? "insert" : "import_new"), style: .default) { [weak self] _ in self?.presentImportPicker(insert: insert) })
        }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        present(alert, animated: true)
    }

    private func presentImportPicker(insert: Bool) {
        importingIntoProject = insert
        let extensions = ProjectLibrary.importExtensions.filter { !insert || !ProjectLibrary.nativeExtensions.contains($0) }.sorted()
        let types = extensions.compactMap { UTType(filenameExtension: $0) }
        let picker = UIDocumentPickerViewController(forOpeningContentTypes: types, asCopy: false)
        picker.delegate = self; picker.allowsMultipleSelection = false; picker.shouldShowFileExtensions = true
        picker.isModalInPresentation = true
        present(picker, animated: true)
    }

    private func exportProject(format: String) {
        guard !numericEditing, !modellingEditing, presentedViewController == nil, pending.isEmpty else { return }
        canvas.cancelInput()
        session.exportProject(format: format) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.showProjectFailure(error)
            case .success(let file):
                self.pendingExport = file
                let picker = UIDocumentPickerViewController(forExporting: [file], asCopy: true)
                picker.delegate = self; picker.shouldShowFileExtensions = true; picker.isModalInPresentation = true
                self.present(picker, animated: true)
            }
        }
    }

    func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
        if pendingExport != nil {
            clearPendingExport(); documentStatus = ProjectText.text("exported"); updateSubtitle(); return
        }
        guard let source = urls.first else { return }
        controller.dismiss(animated: true) { [weak self] in self?.performProjectImport(source) }
    }

    private func performProjectImport(_ source: URL) {
        session.importProject(from: source, insert: importingIntoProject) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): self.showProjectFailure(error)
            case .success(let warnings):
                self.failure = nil; self.session.command("view.fit")
                if !warnings.isEmpty {
                    let alert = UIAlertController(title: ProjectText.text("warnings"), message: warnings.joined(separator: "\n"), preferredStyle: .alert)
                    alert.addAction(UIAlertAction(title: WorkspaceStyle.text("close"), style: .default))
                    self.present(alert, animated: true)
                }
            }
        }
    }

    func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) { clearPendingExport() }

    private func clearPendingExport() {
        if let pendingExport { try? FileManager.default.removeItem(at: pendingExport.deletingLastPathComponent()) }
        pendingExport = nil
    }

    private func chooseSketchPlane() {
        guard !numericEditing, !modellingEditing, state?.sketch == nil, planeDraft == nil, !planeConfirming, !planeChoosing else { return }
        if visualPlaneSelection {
            if let selected = state?.selection.first, selected.type == "plane", let name = selected.name {
                previewSketchPlane(name, label: name)
                return
            }
            planeChoosing = true
            canvas.cancelInput(); inspectorOpen = false; browserOpen = false
            session.command("view.choose_sketch_plane") { [weak self] result in
                guard let self else { return }
                if case .failure = result { self.planeChoosing = false }
                self.rebuildInspector(); self.updateStatus(); self.updateAvailability()
            }
            rebuildInspector(); updateStatus(); relayoutPanels()
            return
        }
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
                self.planeChoosing = false
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
        planeChoosing = false
        session.cancelSketchPlane()
        rebuildInspector(); updateStatus(); updateAvailability()
    }

    private func showReferenceLabels(_ labels: [ReferenceLabelPresentation]) {
        latestReferenceLabels = labels
        let labels = labels.filter { axisLabelsVisible || !Self.originAxes.contains($0.name) }
        let retained = Set(labels.map(\.name))
        for name in Array(referenceLabels.keys) where !retained.contains(name) {
            referenceLabels.removeValue(forKey: name)?.removeFromSuperview()
        }
        for label in labels {
            guard label.position.count == 2, label.position.allSatisfy(\.isFinite) else { continue }
            let button: WorkspaceButton
            if let existing = referenceLabels[label.name] { button = existing }
            else {
                button = WorkspaceButton(label.name == "O" ? SketchText.text("origin_point") : label.name, viewportPoint: label.name == "O") { [weak self] in
                    guard let self, !self.numericEditing, !self.modellingEditing, !self.planeConfirming else { return }
                    self.canvas.cancelInput()
                    self.session.selectEntity(label.entity.dictionary, additive: false)
                }
                button.accessibilityIdentifier = "origin-reference-" + label.name
                button.accessibilityLabel = label.name == "O" ? SketchText.text("origin_point") : label.name
                workspace.insertSubview(button, aboveSubview: canvas); referenceLabels[label.name] = button
            }
            button.active(label.selected)
            let centre = CGPoint(x: canvas.frame.minX + label.position[0], y: canvas.frame.minY + label.position[1])
            button.isHidden = numericEditing || modellingEditing || planeDraft != nil || !canvas.frame.insetBy(dx: 22, dy: 22).contains(centre)
            button.frame = CGRect(x: centre.x - 22, y: centre.y - 22, width: 44, height: 44)
        }
    }

    private func layoutEndpointFinish() {
        guard active, state?.sketch != nil, state?.active_tool == "sketch.line",
              !numericEditing, !modellingEditing, pending.isEmpty, presentedViewController == nil,
              let position = sketchInput?.finish_chain_at, position.count == 2, position.allSatisfy(\.isFinite),
              canvas.isUserInteractionEnabled else { endpointFinishButton.isHidden = true; return }
        let anchor = CGPoint(x: canvas.frame.minX + position[0], y: canvas.frame.minY + position[1])
        let safe = workspace.convert(view.safeAreaLayoutGuide.layoutFrame, from: view)
        let bounds = canvas.frame.intersection(safe).insetBy(dx: 8, dy: 8)
        let obstacles = [top, railScroll, navigationScroll, historyActions, featureHistory, browserButton, viewButton, status, browserPanel, inspectorPanel]
            .filter { !$0.isHidden }.map { $0.frame.insetBy(dx: -6, dy: -6) }
        guard bounds.contains(anchor), let frame = ModelValuePlacement.resolve(anchor: anchor,
            size: CGSize(width: 44, height: 44), bounds: bounds, obstacles: obstacles) else {
            endpointFinishButton.isHidden = true; return
        }
        endpointFinishButton.frame = frame
        endpointFinishButton.isEnabled = true
        endpointFinishButton.isHidden = false
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
        sketchAnnotations = Dictionary(annotations.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first })
        let retained = Set(annotations.map(\.id))
        for id in Array(annotationLabels.keys) where !retained.contains(id) { annotationLabels.removeValue(forKey: id)?.removeFromSuperview() }
        let formatter = NumberFormatter()
        formatter.locale = .current; formatter.numberStyle = .decimal
        formatter.usesSignificantDigits = true; formatter.maximumSignificantDigits = 9
        for annotation in annotations where annotation.points.count == 2 && annotation.points.allSatisfy(\.isFinite) && annotation.value.isFinite {
            let label = annotationLabels[annotation.id] ?? SketchDimensionControl()
            if annotationLabels[annotation.id] == nil {
                annotationLabels[annotation.id] = label
                let id = annotation.id
                label.onEdit = { [weak self] in self?.editSketchAnnotation(id) }
                canvas.addSubview(label)
            }
            let number = formatter.string(from: NSNumber(value: annotation.value)) ?? String(annotation.value)
            let text = (annotation.driven ? "(" + number + ")" : number) + " " + annotation.units
            label.update(text: text, selected: annotation.selected, driven: annotation.driven, editable: annotationEditable(annotation))
            label.bounds.size = label.intrinsicContentSize
            label.center = CGPoint(x: annotation.points[0], y: annotation.points[1])
            label.accessibilityIdentifier = "sketch-dimension-" + annotation.id
        }
    }

    private func annotationEditable(_ annotation: SketchAnnotationPresentation) -> Bool {
        inlineDimensions && !annotation.driven && state?.sketch != nil && state?.active_tool == "select" &&
            !numericEditing && !modellingEditing && pending.isEmpty && presentedViewController == nil
    }

    private func editSketchAnnotation(_ id: String) {
        guard let annotation = sketchAnnotations[id], annotationEditable(annotation),
              let context = state, let source = annotationLabels[id] else { return }
        canvas.cancelInput()
        // Reserve the numeric lane while selection is delivered asynchronously.
        numericStarting = true; numericGeneration &+= 1
        let expected = numericGeneration
        updateAvailability()
        session.selectEntity(annotation.entity.dictionary, additive: false) { [weak self, weak source] result in
            guard let self, self.numericGeneration == expected else { return }
            self.numericStarting = false
            guard let source, self.state?.document_id == context.document_id,
                  self.state?.geometry_revision == context.geometry_revision else { self.updateAvailability(); return }
            switch result {
            case .failure(let error): self.showFailure(error); self.updateAvailability()
            case .success: self.beginNumeric(dimension: annotation.dimension_type, source: source)
            }
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
        // Constraint badges can share the default dimension lane. Keep the
        // editable number legible above those smaller glyphs.
        annotationLabels.values.forEach { canvas.bringSubviewToFront($0) }
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
                edit.commandArtwork("sketch.dimension")
                edit.accessibilityIdentifier = "workspace-edit-dimension"
                properties.addArrangedSubview(edit)
            }
        } else if let dimensions = sketch.eligible_dimensions, !dimensions.isEmpty {
            let dimension = WorkspaceButton(SketchText.text("dimension"), prominent: true, action: {})
            dimension.commandArtwork("sketch.dimension")
            dimension.menu = UIMenu(children: dimensions.map { type in
                UIAction(title: SketchText.text(type)) { [weak self] _ in self?.beginNumeric(dimension: type) }
            })
            dimension.showsMenuAsPrimaryAction = true
            dimension.accessibilityIdentifier = "workspace-create-dimension"
            properties.addArrangedSubview(dimension)
            let place = WorkspaceButton(SketchText.text("place_dimension"), action: {})
            place.commandArtwork("sketch.dimension")
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
            button.commandArtwork(action.command)
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
            self?.toggleMultipleSelection()
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

    private func beginNumeric(dimension: String? = nil, source: UIView? = nil) {
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
                self.installEditor(expression: expression, units: value.units, source: source)
                if let number = value.value { self.editor?.setEvaluation(value: number, units: value.units) }
            }
        }
    }

    private func installEditor(expression: String, units: String, source: UIView? = nil) {
        let name = state?.sketch == nil ? WorkspaceStyle.text("radius") : SketchText.text("dimension")
        let controller = NumericValueEditorController(expression: expression, units: units, name: name)
        controller.overrideUserInterfaceStyle = workspaceInterfaceStyle
        let editor = controller.editor
        self.editor = editor
        editor.requiresPreview = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.dimensionPreviewRequired")
        editor.onPreview = { [weak self] expression in self?.evaluateNumeric(expression, commit: false) }
        editor.onApply = { [weak self] expression in self?.evaluateNumeric(expression, commit: true) }
        editor.onCancel = { [weak self] in self?.cancelNumeric() }
        editor.onFocus = { [weak self] in self?.canvas.cancelInput(); self?.updateAvailability() }
        let anchor = source ?? inspectorButton
        if let popover = controller.popoverPresentationController {
            popover.sourceView = workspace; popover.sourceRect = anchor.convert(anchor.bounds, to: workspace)
            popover.permittedArrowDirections = [.up, .down, .left, .right]; popover.delegate = self
        }
        inlineNumericEditor = controller
        present(controller, animated: true)
        failure = nil
        updateAvailability(); updateStatus(); view.setNeedsLayout()
    }

    private func evaluateNumeric(_ expression: String, commit: Bool) {
        guard let token = numericToken, let editor, !editor.busy else { return }
        let expected = numericGeneration
        let revision = editor.draft.revision
        editor.setBusy(true, committing: commit)
        session.requestNumeric(action: commit ? "commit" : "preview", token: token, expression: expression) { [weak self, weak editor] result in
            guard let self, self.numericGeneration == expected else { return }
            switch result {
            case .failure(let error):
                editor?.completePreview(revision: revision, error: error.message)
                if editor?.draft.revision == revision { UIAccessibility.post(notification: .announcement, argument: error.message) }
            case .success(let value):
                if commit {
                    self.numericToken = nil; self.numericContext = nil
                    self.clearNumericEditor()
                    self.receive(value.workspace)
                    self.failure = nil; self.updateStatus()
                } else {
                    editor?.completePreview(revision: revision, value: value.value, units: value.units)
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
                    self.numericToken = nil; self.numericContext = nil; self.clearNumericEditor()
                    self.failure = error.message; self.rebuildInspector(); self.updateStatus()
                    self.session.refreshWorkspace()
                } else { self.editor?.setBusy(false); self.editor?.message.show(error.message, error: true) }
            case .success(let value):
                self.numericToken = nil; self.numericContext = nil; self.clearNumericEditor()
                self.failure = nil; self.receive(value.workspace)
            }
            self.updateAvailability()
        }
    }

    private func clearNumericEditor() {
        editor?.field.resignFirstResponder(); editor = nil
        let controller = inlineNumericEditor
        inlineNumericEditor = nil
        controller?.dismiss(animated: false) { [weak self] in self?.updateAvailability() }
    }

    private func run(_ command: String, params: [String: Any] = [:]) {
        guard WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: textEntryActive, numericEditActive: numericEditing) else { return }
        guard !modellingEditing || command.hasPrefix("view.") || command.hasPrefix("select.") else { return }
        historyActivation = nil
        canvas.cancelInput(); failure = nil; lastOperationFeedback = []
        session.command(command, params: params)
    }
    private func setTool(_ tool: String) {
        guard !numericEditing, state?.tools.first(where: { $0.id == tool })?.enabled == true else { return }
        run("tool.set", params: ["tool": tool])
        if tool == "post_radius" { inspectorOpen = true; browserOpen = false; view.setNeedsLayout() }
    }
    private func toggleMultipleSelection() {
        let enabled = !multiple
        canvas.cancelInput()
        session.command("select.multiple", params: ["enabled": enabled]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success:
                self.multiple = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.multipleSelection")
                self.rebuildInspector(); self.rebuildMenus()
            case .failure(let error): self.showFailure(error)
            }
        }
    }

    private func toggleBrowser() {
        guard !numericEditing, !modellingCommitting else { return }
        if inspectorOpen && cancelToolsOnDismiss { cancelModelling() }
        modellingInspector?.endEditing(true)
        canvas.cancelInput(); browserOpen.toggle(); inspectorOpen = false
        relayoutPanels()
    }
    private func toggleInspector() {
        guard !numericEditing, !modellingCommitting else { return }
        if inspectorOpen { closePanels(); return }
        modellingInspector?.endEditing(true)
        canvas.cancelInput(); inspectorOpen.toggle(); browserOpen = false
        relayoutPanels()
    }
    private func closePanels() {
        // Closing or tapping outside a form discards its provisional operation.
        // An Apply already being accepted must finish before dismissal.
        guard !numericEditing else { editor?.message.show(WorkspaceStyle.text("editing_lock")); return }
        guard !modellingCommitting else { return }
        if cancelToolsOnDismiss { cancelModelling() }
        canvas.cancelInput(); browserOpen = false; inspectorOpen = false
        relayoutPanels()
    }
    private func relayoutPanels() {
        let animateBrowser = compactChrome && leftBrowser && !UIAccessibility.isReduceMotionEnabled
        let wasOpen = !browserPanel.isHidden
        let oldFrame = browserPanel.frame
        let chrome: [UIView] = [top, status, railScroll, navigationScroll, historyActions, featureHistory, browserButton]
        let oldChrome = chrome.map(\.frame)
        browserAnimationGeneration &+= 1
        let animation = browserAnimationGeneration
        view.setNeedsLayout()
        view.layoutIfNeeded()
        if animateBrowser, wasOpen != browserOpen {
            let target = browserPanel.frame
            let targetChrome = chrome.map(\.frame)
            for (control, frame) in zip(chrome, oldChrome) { control.frame = frame }
            browserPanel.isHidden = false
            if browserOpen {
                browserPanel.frame = target.offsetBy(dx: -target.width - 16, dy: 0)
            } else { browserPanel.frame = oldFrame }
            UIView.animate(withDuration: 0.25, delay: 0, options: [.beginFromCurrentState, .curveEaseInOut]) {
                for (control, frame) in zip(chrome, targetChrome) { control.frame = frame }
                self.browserPanel.frame = self.browserOpen ? target : oldFrame.offsetBy(dx: -oldFrame.width - 16, dy: 0)
            } completion: { [weak self] _ in
                guard let self, self.browserAnimationGeneration == animation else { return }
                self.browserPanel.isHidden = !self.browserOpen
                self.view.setNeedsLayout()
            }
        }
        updateAvailability()
        UIAccessibility.post(notification: .layoutChanged, argument: browserOpen ? browserPanel : (inspectorOpen ? inspectorPanel : browserButton))
    }

    private func updateAvailability() {
        rebuildViewMenu()
        updateModelValueChoices()
        for (id, label) in annotationLabels {
            label.editable = sketchAnnotations[id].map(annotationEditable) ?? false
        }
        browserFindButton.availability(enabled: !numericEditing && !modellingEditing && !planeConfirming && pending.isEmpty)
        browserSearchField.isEnabled = !numericEditing && !modellingEditing && !planeConfirming && pending.isEmpty
        if browserControlsEnabled != browserCanChange {
            browserControlsEnabled = browserCanChange
            table.reloadData()
        } else { table.reloadSections(IndexSet(integer: 0), with: .none) }
        let locked = numericEditing || modellingEditing || !pending.isEmpty
        let features = state?.modelling?.timeline ?? []
        let featureIcons = Dictionary(features.map { feature in
            (feature.id, rowByID["feature:" + String(feature.id)]?.browser?.icon ?? "component")
        }, uniquingKeysWith: { first, _ in first })
        let selectedFeatures = Set((state?.selection ?? []).filter { $0.type == "feature" }.compactMap { entity in
            entity.id.flatMap { UInt64($0.readout) }
        })
        featureHistory.update(features: features, icons: featureIcons, selected: selectedFeatures,
            position: state?.modelling?.activeFeatureCount ?? 0, enabled: browserCanChange && state?.sketch == nil, navigationEnabled: historyCanNavigate,
            scrubbingEnabled: historyContextAllowsNavigation && (pending.isEmpty || featureHistory.scrubInFlight), documentID: state?.document_id)
        undoButton.availability(enabled: !locked && state?.can_undo == true, reason: numericEditing || modellingEditing ? WorkspaceStyle.text("editing_lock") : WorkspaceStyle.text("nothing_undo"))
        redoButton.availability(enabled: !locked && state?.can_redo == true, reason: numericEditing || modellingEditing ? WorkspaceStyle.text("editing_lock") : WorkspaceStyle.text("nothing_redo"))
        for button in [browserButton, inspectorButton, viewButton, moreButton] { button.availability(enabled: !numericEditing && !modellingCommitting && pending.isEmpty, reason: numericEditing ? WorkspaceStyle.text("editing_lock") : nil) }
        browserButton.active(browserOpen); inspectorButton.active(inspectorOpen)
        inspectorButton.isHidden = false
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
        createSketchButton.availability(enabled: !locked && planeDraft == nil && !planeChoosing)
        radiusButton.isHidden = inSketch || compactChrome && !diagnostics
        sketchToolsButton.isHidden = !inSketch
        finishSketchButton.isHidden = !inSketch
        finishChainButton.isHidden = !inSketch || !(state?.active_tool == "sketch.line" || state?.active_tool == "sketch.spline.fit_point")
        for button in [sketchToolsButton, finishSketchButton, finishChainButton] { button.availability(enabled: !locked) }
        cancelToolButton.isHidden = numericEditing || !(inSketch || state?.active_tool == "post_radius" || planeChoosing || planeDraft != nil)
        let cancelTitle = planeChoosing || planeDraft != nil ? WorkspaceStyle.text("cancel") : SketchText.text("cancel_segment")
        cancelToolButton.updateTitle(cancelTitle)
        table.allowsSelection = !numericEditing && !modellingCommitting && pending.isEmpty
        let modal = layout?.compact == true && (browserOpen || inspectorOpen)
        let textBlocksViewport = textEntryActive && !(cancelToolsOnDismiss && modellingEditing)
        canvas.isUserInteractionEnabled = !numericEditing && WorkspaceFocusPolicy.viewportInputEnabled(modalPresented: modal, textEntryActive: textBlocksViewport)
        canvas.accessibilityElementsHidden = modal || numericEditing
        layoutEndpointFinish()
        for button in referenceLabels.values {
            button.accessibilityElementsHidden = modal || numericEditing || modellingEditing
            button.isUserInteractionEnabled = !modal && !numericEditing && !modellingEditing
        }
        if compactChrome {
            let editing = numericEditing || modellingEditing
            for (id, button) in sketchQuickTools {
                let tool = state?.tools.first { $0.id == id }
                button.isHidden = !inSketch || editing
                button.active(state?.active_tool == id)
                button.availability(enabled: !locked && tool?.enabled == true, reason: tool?.reason.map(NativeText.value))
            }
            createSketchButton.isHidden = inSketch || editing
            modellingButton.isHidden = inSketch || editing
            assemblyButton.isHidden = inSketch || editing
            modifyButton.isHidden = inSketch || editing
            patternButton.isHidden = inSketch || editing
            for button in [modifyButton, patternButton] { button.availability(enabled: !locked && state?.modelling?.tools.isEmpty == false) }
            sketchToolsButton.isHidden = !inSketch || editing
            finishSketchButton.isHidden = !inSketch || editing
            finishChainButton.isHidden = finishChainButton.isHidden || editing
            [projectionButton, documentActionsButton].forEach {
                $0.availability(enabled: !numericEditing && !modellingCommitting && pending.isEmpty)
            }
            projectionButton.active(perspective)
            projectionButton.isHidden = inSketch
            constraintsButton.isHidden = !inSketch || editing
            dimensionsButton.isHidden = !inSketch || editing
            constructionButton.isHidden = !inSketch || editing
            constraintsButton.availability(enabled: !locked, reason: WorkspaceStyle.text("constraint_selection_hint"))
            dimensionsButton.availability(enabled: !locked && state?.sketch?.eligible_dimensions?.isEmpty == false,
                                           reason: WorkspaceStyle.text("dimension_selection_hint"))
            constructionButton.availability(enabled: !locked && state?.selection.contains(where: { $0.type == "sketch_curve" }) == true,
                                             reason: WorkspaceStyle.text("curve_selection_hint"))
        }
        layoutModelValue()
    }

    private func rebuildMenus() {
        assemblyButton.menu = UIMenu(children: (state?.assembly?.tools ?? []).map { tool in
            UIAction(title: ModellingText.label(tool.label), image: WorkspaceIcon.command(tool.id, traits: traitCollection)) { [weak self] _ in self?.beginModelling(operation: tool.id) }
        })
        if let assembly = state?.assembly {
            let contextual = assembly.actions.map { action in
                UIAction(title: ModellingText.label(action.label), image: action.command.flatMap { WorkspaceIcon.command($0, traits: traitCollection) }) { [weak self] _ in self?.performAssemblyAction(action) }
            }
            assemblyButton.menu = UIMenu(children: [UIMenu(options: .displayInline, children: assemblyButton.menu?.children ?? []), UIMenu(options: .displayInline, children: contextual)])
        }
        assemblyButton.showsMenuAsPrimaryAction = true
        let tools = state?.modelling?.tools ?? []
        func modelActions(_ included: (String) -> Bool) -> [UIMenuElement] {
            tools.filter { included($0.id) }.map { tool in
                UIAction(title: ModellingText.label(tool.label), image: WorkspaceIcon.command(tool.id, traits: traitCollection)) { [weak self] _ in self?.beginModelling(operation: tool.id) }
            }
        }
        // New native-supported operations remain reachable in this overflow
        // group and inherit their registry icon without another native mapping.
        modellingButton.menu = UIMenu(children: modelActions {
            !compactChrome || !$0.hasPrefix("parameters.") && !$0.hasPrefix("solid.pattern.") &&
                !["solid.fillet", "solid.chamfer", "solid.shell", "solid.hole", "solid.combine", "solid.mirror", "solid.offset_face"].contains($0)
        })
        modifyButton.menu = UIMenu(children: modelActions { ["solid.fillet", "solid.chamfer", "solid.shell", "solid.hole", "solid.combine", "solid.offset_face"].contains($0) })
        patternButton.menu = UIMenu(children: modelActions { $0.hasPrefix("solid.pattern.") || $0 == "solid.mirror" })
        modifyButton.showsMenuAsPrimaryAction = true; patternButton.showsMenuAsPrimaryAction = true
        modellingButton.showsMenuAsPrimaryAction = true
        rebuildViewMenu()
        documentActionsButton.menu = UIMenu(children: projectMenuActions())
        documentActionsButton.showsMenuAsPrimaryAction = true
        let filters = WorkspaceSelectionFilter.allCases.map { option in
            let name = option.rawValue
            return UIAction(title: NativeText.value(option == .occurrence ? "ui.ipados.assembly.occurrence" : "ui.ipados." + name), state: filter == option ? .on : .off) { [weak self] _ in
                guard let self, WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: self.textEntryActive, numericEditActive: self.numericEditing) else { return }
                self.canvas.cancelInput()
                self.session.command("select.filter", params: ["filter": name]) { [weak self] result in
                    guard let self else { return }
                    switch result {
                    case .success:
                        self.filter = option
                        UserDefaults.standard.set(name, forKey: "SolidCraft.workspace.selectionFilter")
                        self.rebuildMenus()
                    case .failure(let error): self.showFailure(error)
                    }
                }
            }
        }
        let multiple = UIAction(title: WorkspaceStyle.text("multiple"), state: multiple ? .on : .off) { [weak self] _ in
            self?.toggleMultipleSelection()
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
        let homeToolbar = UIAction(title: WorkspaceStyle.text("home_toolbar"), state: homeToolbar ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.homeToolbar.toggle()
            UserDefaults.standard.set(self.homeToolbar, forKey: "SolidCraft.workspace.homeToolbar")
            self.configureChrome(); self.rebuildMenus()
        }
        let historyGestures = UIAction(title: WorkspaceStyle.text("history_gestures"), state: canvas.historyGesturesEnabled ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.canvas.cancelInput()
            self.canvas.historyGesturesEnabled.toggle()
            UserDefaults.standard.set(self.canvas.historyGesturesEnabled, forKey: "SolidCraft.workspace.historyGestures")
            self.rebuildMenus()
        }
        let dimensionPreviewRequired = UserDefaults.standard.bool(forKey: "SolidCraft.workspace.dimensionPreviewRequired")
        let dimensionPreview = UIAction(title: NumericText.text("require_preview"), state: dimensionPreviewRequired ? .on : .off) { [weak self] _ in
            UserDefaults.standard.set(!dimensionPreviewRequired, forKey: "SolidCraft.workspace.dimensionPreviewRequired")
            self?.editor?.requiresPreview = !dimensionPreviewRequired
            self?.rebuildMenus()
        }
        let twist = UIAction(title: WorkspaceStyle.text("two_finger_twist"), attributes: numericEditing || modellingEditing ? .disabled : [], state: twoFingerTwist ? .on : .off) { [weak self] _ in
            guard let self else { return }
            let enabled = !self.twoFingerTwist
            self.canvas.cancelInput()
            self.session.command("view.touch_twist", params: ["enabled": enabled]) { [weak self] result in
                guard let self, case .success = result else { return }
                self.twoFingerTwist = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.twoFingerTwist")
                self.rebuildMenus()
            }
        }
        let orbit = UIAction(title: WorkspaceStyle.text("viewpoint_orbit"), attributes: numericEditing || modellingEditing ? .disabled : [], state: viewpointOrbit ? .on : .off) { [weak self] _ in
            guard let self else { return }
            let enabled = !self.viewpointOrbit
            self.canvas.cancelInput()
            self.session.command("view.touch_orbit", params: ["follows_viewpoint": enabled]) { [weak self] result in
                guard let self, case .success = result else { return }
                self.viewpointOrbit = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.viewpointOrbit")
                self.rebuildMenus()
            }
        }
        let handles = UIAction(title: HandleText.text("direct_handles"), attributes: numericEditing || modellingEditing ? .disabled : [], state: directHandles ? .on : .off) { [weak self] _ in
            guard let self else { return }
            let enabled = !self.directHandles
            self.canvas.cancelInput()
            self.session.command("view.tool_handles", params: ["enabled": enabled]) { [weak self] result in
                guard let self, case .success = result else { return }
                self.directHandles = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.directHandles")
                self.layoutModelValue(); self.rebuildMenus()
            }
        }
        let dragResolution = UIMenu(title: WorkspaceStyle.text("drag_resolution"), children: DragResolution.options.map { step in
            UIAction(title: step == 0 ? WorkspaceStyle.text("drag_continuous") : String(step) + " mm",
                     attributes: numericEditing || modellingEditing ? .disabled : [], state: dragStepMM == step ? .on : .off) { [weak self] _ in
                guard let self, !self.numericEditing, !self.modellingEditing else { return }
                self.canvas.cancelInput()
                self.session.command("view.drag_precision", params: ["step_mm": step]) { [weak self] result in
                    guard let self, case .success = result else { return }
                    self.dragStepMM = step
                    UserDefaults.standard.set(step, forKey: DragResolution.key)
                    self.rebuildMenus()
                }
            }
        })
        let dismissal = UIAction(title: WorkspaceStyle.text("cancel_tools_on_dismiss"), subtitle: WorkspaceStyle.text("cancel_tools_on_dismiss_help"), attributes: modellingEditing ? .disabled : [], state: cancelToolsOnDismiss ? .on : .off) { [weak self] _ in
            guard let self, !self.modellingEditing else { return }
            let enabled = !self.cancelToolsOnDismiss
            self.canvas.cancelInput()
            self.session.command("view.tool_dismissal", params: ["cancel_on_dismiss": enabled]) { [weak self] result in
                guard let self, case .success = result else { return }
                self.cancelToolsOnDismiss = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.cancelToolsOnDismiss")
                self.rebuildMenus()
            }
        }
        let faceExtrude = UIAction(title: WorkspaceStyle.text("extrude_on_face_selection"), subtitle: WorkspaceStyle.text("extrude_on_face_selection_help"), state: extrudeOnFaceSelection ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.extrudeOnFaceSelection.toggle()
            UserDefaults.standard.set(self.extrudeOnFaceSelection, forKey: "SolidCraft.workspace.extrudeOnFaceSelection")
            self.rebuildMenus()
        }
        let sketchExtrude = UIAction(title: WorkspaceStyle.text("extrude_on_sketch_selection"), subtitle: WorkspaceStyle.text("extrude_on_sketch_selection_help"), state: extrudeOnSketchSelection ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.extrudeOnSketchSelection.toggle()
            UserDefaults.standard.set(self.extrudeOnSketchSelection, forKey: "SolidCraft.workspace.extrudeOnSketchSelection")
            self.rebuildMenus()
        }
        let automaticCut = UIAction(title: NativeText.value("ui.ipados.extrude.automatic_subtract"), attributes: modellingEditing ? .disabled : [], state: automaticSubtract ? .on : .off) { [weak self] _ in
            guard let self, !self.modellingEditing else { return }
            let enabled = !self.automaticSubtract
            self.canvas.cancelInput()
            self.session.command("view.automatic_subtract", params: ["enabled": enabled]) { [weak self] result in
                guard let self, case .success = result else { return }
                self.automaticSubtract = enabled
                UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.automaticSubtract")
                self.rebuildMenus()
            }
        }
        var moreActions: [UIMenuElement] = [
            UIMenu(title: ProjectText.text("title"), children: projectMenuActions()),
            UIMenu(title: WorkspaceStyle.text("filter"), children: filters), multiple,
            UIAction(title: WorkspaceStyle.text("clear")) { [weak self] _ in self?.run("select.clear") },
            homeToolbar, historyGestures, dimensionPreview, orbit, twist, handles, dragResolution, automaticCut, dismissal, faceExtrude, sketchExtrude, side, UIMenu(title: WorkspaceStyle.text("appearance"), children: appearances)
        ]
        let restoreLastProject = UserDefaults.standard.object(forKey: "SolidCraft.projects.restoreLast") as? Bool ?? true
        moreActions.append(UIAction(title: ProjectText.text("restore_last"), state: restoreLastProject ? .on : .off) { [weak self] _ in
            UserDefaults.standard.set(!restoreLastProject, forKey: "SolidCraft.projects.restoreLast")
            self?.rebuildMenus()
        })
        moreActions.append(UIAction(title: SketchText.text("visual_plane_selection"), state: visualPlaneSelection ? .on : .off) { [weak self] _ in
            guard let self else { return }
            self.visualPlaneSelection.toggle()
            UserDefaults.standard.set(self.visualPlaneSelection, forKey: "SolidCraft.workspace.visualPlaneSelection")
            if self.planeChoosing { self.cancelSketchPlane() }
            self.rebuildMenus()
        })
        moreActions.append(UIMenu(title: ModellingText.text("parameters"), children: modelActions { $0.hasPrefix("parameters.") }))
        moreActions.append(UIAction(title: InspectorText.text("compact_inspector"),
                                    attributes: modellingEditing ? .disabled : [], state: compactInspector ? .on : .off) { [weak self] _ in
            guard let self, !self.modellingEditing else { return }
            self.compactInspector.toggle()
            UserDefaults.standard.set(self.compactInspector, forKey: "SolidCraft.workspace.compactInspector")
            self.rebuildMenus()
        })
        moreActions.append(UIAction(title: WorkspaceStyle.text("use_top_safe_area"),
                                    subtitle: WorkspaceStyle.text("use_top_safe_area_help"),
                                    state: useTopSafeArea ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.useTopSafeArea.toggle()
            UserDefaults.standard.set(self.useTopSafeArea, forKey: "SolidCraft.workspace.useTopSafeArea")
            self.view.setNeedsLayout(); self.rebuildMenus()
        })
        moreActions.append(UIAction(title: WorkspaceStyle.text("use_bottom_safe_area"),
                                    subtitle: WorkspaceStyle.text("use_bottom_safe_area_help"),
                                    state: useBottomSafeArea ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.canvas.cancelInput(); self.useBottomSafeArea.toggle()
            UserDefaults.standard.set(self.useBottomSafeArea, forKey: "SolidCraft.workspace.useBottomSafeArea")
            self.view.setNeedsLayout(); self.rebuildMenus()
        })
        moreActions.append(UIAction(title: WorkspaceStyle.text("pin_browser"), state: pinnedBrowser ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.canvas.cancelInput(); self.pinnedBrowser.toggle()
            UserDefaults.standard.set(self.pinnedBrowser, forKey: "SolidCraft.workspace.pinnedBrowser")
            self.view.setNeedsLayout(); self.rebuildMenus()
        })
        moreActions.append(UIAction(title: BrowserText.text("left_browser"), state: leftBrowser ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.canvas.cancelInput(); self.leftBrowser.toggle()
            UserDefaults.standard.set(self.leftBrowser, forKey: "SolidCraft.workspace.leftBrowser")
            self.view.setNeedsLayout(); self.rebuildMenus()
        })
        moreActions.append(UIAction(title: WorkspaceStyle.text("diagnostics"), state: diagnostics ? .on : .off) { [weak self] _ in
            guard let self else { return }; self.diagnostics.toggle()
            UserDefaults.standard.set(self.diagnostics, forKey: "SolidCraft.workspace.diagnostics")
            self.updateAvailability(); self.updateStatus(); self.view.setNeedsLayout(); self.rebuildMenus()
        })
        if state?.sketch != nil {
            moreActions.insert(UIAction(title: SketchText.text("inline_dimensions"), subtitle: SketchText.text("inline_dimensions_help"),
                                        attributes: numericEditing ? .disabled : [], state: inlineDimensions ? .on : .off) { [weak self] _ in
                guard let self, !self.numericEditing else { return }
                self.canvas.cancelInput(); self.inlineDimensions.toggle()
                UserDefaults.standard.set(self.inlineDimensions, forKey: "SolidCraft.workspace.inlineDimensions")
                self.updateAvailability(); self.rebuildMenus()
            }, at: 0)
            moreActions.insert(UIAction(title: SketchText.text("snapping"), state: sketchInput?.snapping != false ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.run("tool.snapping", params: ["enabled": self.sketchInput?.snapping == false])
            }, at: 0)
            moreActions.insert(UIAction(title: SketchText.text("auto_constraints"), state: sketchInput?.auto_constraints != false ? .on : .off) { [weak self] _ in
                guard let self else { return }; self.run("tool.auto_constraints", params: ["enabled": self.sketchInput?.auto_constraints == false])
            }, at: 1)
            moreActions.insert(UIAction(title: SketchText.text("free_dimensions"), subtitle: SketchText.text("free_dimensions_help"),
                                        attributes: numericEditing || modellingEditing ? .disabled : [], state: freeDimensions ? .on : .off) { [weak self] _ in
                guard let self, !self.numericEditing, !self.modellingEditing else { return }
                let enabled = !self.freeDimensions
                self.canvas.cancelInput()
                self.session.command("tool.free_dimensions", params: ["enabled": enabled]) { [weak self] result in
                    guard let self, case .success = result else { return }
                    self.freeDimensions = enabled
                    UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.freeDimensions")
                    self.rebuildMenus()
                }
            }, at: 2)
            moreActions.insert(UIAction(title: SketchText.text("repeat_rectangles"), subtitle: SketchText.text("repeat_rectangles_help"),
                                        attributes: numericEditing || modellingEditing ? .disabled : [], state: repeatRectangles ? .on : .off) { [weak self] _ in
                guard let self, !self.numericEditing, !self.modellingEditing else { return }
                let enabled = !self.repeatRectangles
                self.canvas.cancelInput()
                self.session.command("tool.repeat_rectangles", params: ["enabled": enabled]) { [weak self] result in
                    guard let self, case .success = result else { return }
                    self.repeatRectangles = enabled
                    UserDefaults.standard.set(enabled, forKey: "SolidCraft.workspace.repeatRectangles")
                    self.rebuildMenus()
                }
            }, at: 3)
        }
        moreButton.menu = UIMenu(children: moreActions)
        moreButton.showsMenuAsPrimaryAction = true
        let sketchTools = [("sketch.line", "line"), ("sketch.rectangle.two_point", "rectangle"), ("sketch.circle.center", "circle"),
                     ("sketch.arc.three_point", "arc"), ("sketch.spline.fit_point", "spline"), ("sketch.move", "move"),
                     ("sketch.delete", "delete"), ("sketch.trim", "trim"), ("sketch.extend", "extend")]
        sketchToolsButton.menu = UIMenu(children: sketchTools.compactMap { id, label in
            guard state?.tools.contains(where: { $0.id == id && $0.enabled }) == true else { return nil }
            return UIAction(title: SketchText.text(label), image: WorkspaceIcon.command(id, traits: traitCollection), state: state?.active_tool == id ? .on : .off) { [weak self] _ in self?.setTool(id) }
        })
        sketchToolsButton.showsMenuAsPrimaryAction = true
        sketchToolsButton.active(state?.active_tool.hasPrefix("sketch.") == true && state?.active_tool.hasPrefix("sketch.constraint.") != true)
        constraintsButton.active(state?.active_tool.hasPrefix("sketch.constraint.") == true)
        dimensionsButton.menu = UIMenu(children: (state?.sketch?.eligible_dimensions ?? []).map { type in
            UIAction(title: SketchText.text(type), image: WorkspaceIcon.image("dimension", traits: traitCollection)) { [weak self] _ in self?.beginNumeric(dimension: type) }
        })
        dimensionsButton.showsMenuAsPrimaryAction = true
    }

    private static let constraintChoices = ["horizontal", "vertical", "parallel", "perpendicular", "coincident", "tangent", "equal", "concentric", "midpoint", "fix"]

    private func showConstraints() {
        guard active, !numericEditing, !modellingEditing, pending.isEmpty,
              presentedViewController == nil, let context = state, context.sketch != nil else { return }
        canvas.cancelInput(); view.endEditing(true)
        // Native UIButton menu previews can raise the software keyboard on iPad.
        // Use the same text-free anchored sheet as the History actions.
        let sheet = UIAlertController(title: WorkspaceStyle.text("constraints"), message: nil, preferredStyle: .actionSheet)
#if DEBUG
        constraintMenuActions.removeAll()
#endif
        for key in Self.constraintChoices {
            let apply = { [weak self] in
                guard let self, self.active, !self.numericEditing, !self.modellingEditing, self.pending.isEmpty,
                      self.state?.document_id == context.document_id,
                      self.state?.geometry_revision == context.geometry_revision,
                      self.state?.sketch?.id == context.sketch?.id, self.state?.selection == context.selection,
                      self.state?.sketch != nil else { return }
                self.run("tool.constraint", params: ["constraint": key])
            }
#if DEBUG
            constraintMenuActions[key] = apply
#endif
            let choice = UIAlertAction(title: SketchText.text(key), style: .default) { _ in apply() }
            sheet.addAction(choice)
        }
        sheet.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        sheet.popoverPresentationController?.sourceView = constraintsButton
        sheet.popoverPresentationController?.sourceRect = constraintsButton.bounds
        present(sheet, animated: true)
    }
    private var workspaceInterfaceStyle: UIUserInterfaceStyle {
        appearance == 1 ? .light : (appearance == 2 ? .dark : .unspecified)
    }
    private func applyAppearance() {
        overrideUserInterfaceStyle = workspaceInterfaceStyle
        // UIKit presents popovers/sheets beside the root view hierarchy. Give
        // their presentation environment the saved preference, including System.
        viewIfLoaded?.window?.overrideUserInterfaceStyle = workspaceInterfaceStyle
        presentedViewController?.overrideUserInterfaceStyle = workspaceInterfaceStyle
    }
    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        applyAppearance()
    }

    private func updateCanvasAppearance() {
        guard canvas.traitCollection.userInterfaceStyle == .dark else {
            session.command("view.background", params: ["color": NSNull()])
            return
        }
        func components(_ color: UIColor) -> ([CGFloat], CGFloat)? {
            let resolved = color.resolvedColor(with: canvas.traitCollection)
            var red: CGFloat = 0, green: CGFloat = 0, blue: CGFloat = 0, alpha: CGFloat = 0
            guard resolved.getRed(&red, green: &green, blue: &blue, alpha: &alpha),
                  [red, green, blue, alpha].allSatisfy({ $0.isFinite }) else { return nil }
            return ([red, green, blue], min(1, max(0, alpha)))
        }
        func rgb(_ values: [CGFloat]) -> [Int] { values.map { Int((min(1, max(0, $0)) * 255).rounded()) } }
        guard let (surface, _) = components(WorkspaceStyle.surface),
              let (sketch, _) = components(WorkspaceStyle.darkSketchLine),
              let (separator, alpha) = components(WorkspaceStyle.separator) else { return }
        // Composite the semantic separator over the canvas; minor lines are subtler.
        let grid = [CGFloat(0.5), 1].map { strength in
            rgb(zip(surface, separator).map { background, line in background + (line - background) * alpha * strength })
        }
        session.command("view.background", params: ["color": rgb(surface), "grid": grid, "sketch": rgb(sketch)])
    }

    private func updateStatus() {
        if let failure { status.textColor = WorkspaceStyle.error; status.text = failure; return }
        status.textColor = WorkspaceStyle.secondary
        if planeChoosing { status.text = SketchText.text("tap_plane"); return }
        if modellingEditing {
            if previewStale { status.text = ModellingText.text("preview_stale"); return }
            status.text = ModellingText.text("draft_hint") + "\n" + (cancelToolsOnDismiss ? WorkspaceStyle.text("tool_dismiss_hint") : ModellingText.text("navigation_hint"))
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
        if let assembly = state?.assembly, state?.sketch == nil, !assembly.diagnostics.isEmpty, !compactChrome || diagnostics {
            status.text = assembly.diagnostics.prefix(2).map(ModellingText.label).joined(separator: "\n")
            return
        }
        if !pending.isEmpty { status.text = pending; return }
        if let sketch = state?.sketch {
            let prompt: String
            if let tool = sketchInput?.tool, tool.hasPrefix("sketch.constraint."), let key = tool.split(separator: ".").last {
                let hint = key == "horizontal" || key == "vertical" || key == "fix" ? "constraint_pick_one" : (state?.selection.isEmpty == false ? "constraint_pick_second" : "constraint_pick_pair")
                prompt = SketchText.text(String(key)) + ": " + SketchText.text(hint)
            } else { prompt = sketchInput.map { SketchText.text($0.prompt) } ?? SketchText.text("select_hint") }
            let snap = sketchInput?.snap.map { SketchText.text($0.persistent ? "persistent_snap" : "temporary_snap") + ": " + SketchText.text($0.kind) } ?? ""
            var measurement = ""
            if let input = sketchInput, let feedback = input.feedback {
                let formatter = NumberFormatter(); formatter.locale = .current; formatter.numberStyle = .decimal; formatter.maximumFractionDigits = 3
                let number: (Double) -> String = { formatter.string(from: NSNumber(value: $0)) ?? String($0) }
                if input.tool == "sketch.rectangle.two_point" { measurement = number(feedback.width) + " × " + number(feedback.height) + " mm" }
                else if input.tool == "sketch.circle.center" { measurement = SketchText.text("radius") + " " + number(feedback.radius) + " mm" }
                else if input.tool == "sketch.line" { measurement = number(feedback.radius) + " mm" }
            }
            if compactChrome && !diagnostics {
                let active = state?.active_tool == "select" ? WorkspaceStyle.text("select") : prompt
                status.text = sketchSolverText(sketch) + "\n" + [active, measurement, snap].filter { !$0.isEmpty }.joined(separator: " · ")
            } else { status.text = sketch.name + " · " + sketchSolverText(sketch) + "\n" + [prompt, measurement, snap].filter { !$0.isEmpty }.joined(separator: " · ") }
            return
        }
        if compactChrome && !diagnostics {
            let selection = state?.selection ?? []
            let mode = numericEditing ? WorkspaceStyle.text("editing") :
                (selection.contains(where: { $0.type == "occurrence" }) || state?.assembly?.active_path.isEmpty == false ? AssemblyText.text("tools") : ModellingText.text("tools"))
            let selected = selection.isEmpty ? NativeText.value("ui.ipados.no_selection") :
                String.localizedStringWithFormat(WorkspaceStyle.text("selection_count"), selection.count)
            status.text = mode + " · " + selected
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
        let escape = UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(cancelKey))
        escape.wantsPriorityOverSystemBehavior = browserSearchOpen && browserOpen
        var commands = [escape]
        if WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: textEntryActive, numericEditActive: numericEditing || modellingEditing) {
            if state?.can_undo == true { commands.append(UIKeyCommand(input: "z", modifierFlags: .command, action: #selector(undoKey))) }
            if state?.can_redo == true { commands.append(UIKeyCommand(input: "z", modifierFlags: [.command, .shift], action: #selector(redoKey))) }
        }
        return commands
    }
    @objc private func undoKey() { run("edit.undo") }
    @objc private func redoKey() { run("edit.redo") }
    @objc private func cancelKey() {
        if browserSearchOpen && browserOpen { dismissBrowserSearch() }
        else if modellingEditing { cancelModelling() }
        else if transformDraft != nil { cancelTransform() }
        else if numericEditing { cancelNumeric() }
        else if planeDraft != nil || planeChoosing { cancelSketchPlane() }
        else if state?.sketch != nil { run("tool.cancel") }
        else { canvas.cancelInput(); closePanels() }
    }

    @objc private func keyboardChanged(_ notification: Notification) {
        if inlineNumericEditor?.editor.panelKeypadVisible == true || modelValueEditor?.editor.panelKeypadVisible == true {
            keyboardTop = nil
        } else if notification.name == UIResponder.keyboardWillHideNotification { keyboardTop = nil }
        else if let frame = notification.userInfo?[UIResponder.keyboardFrameEndUserInfoKey] as? CGRect,
                let window = view.window {
            let local = view.convert(window.convert(frame, from: nil), from: window)
            keyboardTop = local.intersects(view.bounds) ? local.minY : nil
        }
        view.setNeedsLayout(); view.layoutIfNeeded(); revealField(); updateAvailability()
    }
    private func revealField() {
        guard inlineNumericEditor == nil else { return }
        guard let field = editor?.field ?? transformEditors.first(where: { $0.editor.field.isFirstResponder })?.editor.field ?? modellingInspector?.focusedField else { return }
        inspectorScroll.scrollRectToVisible(field.convert(field.bounds, to: inspectorScroll).insetBy(dx: 0, dy: -16), animated: false)
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        workspace.frame = view.bounds
        let safe = workspace.convert(view.safeAreaLayoutGuide.layoutFrame, from: view)
        let keyboardY = keyboardTop.map { workspace.convert(CGPoint(x: 0, y: $0), from: view).y } ?? workspace.bounds.maxY
        let viewport = CGRect(x: 0, y: 0, width: workspace.bounds.width, height: max(0, min(workspace.bounds.maxY, keyboardY)))
        let chrome = WorkspaceLayout.chromeBounds(safeArea: safe, viewport: viewport, useBottomSafeArea: useBottomSafeArea)
        let largeText = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
        let compactWidth = chrome.width < (largeText ? 1120 : 1000)
        let titleHeight = ceil(titleLabel.font.lineHeight) + 2
        let subtitleHeight = 2 * ceil(subtitle.font.lineHeight) + 2
        let actionsHeight = max(44, ceil(UIFont.preferredFont(forTextStyle: .body).lineHeight) + 20)
        let compactHeadingHeight = max(52, titleHeight + ceil(subtitle.font.lineHeight) + 8)
        let completionHeight = finishSketchButton.isHidden ? 0 : actionsHeight + 8
        let headingHeight = compactChrome ? compactHeadingHeight + completionHeight :
            (compactWidth ? titleHeight + subtitleHeight + actionsHeight + 12 : max(titleHeight + subtitleHeight + 8, actionsHeight + 8))
        let inspectorHeight: CGFloat = largeText ? 760 : (numericEditing ? 500 :
            (modellingEditing ? (modellingInspector?.compact == true ? 620 : min(640, 240 + CGFloat(modellingInspector?.fields.count ?? 4) * 84)) : 640))
        func resolve(railHeight: CGFloat, navHeight: CGFloat) -> WorkspaceLayout {
            WorkspaceLayout.resolve(size: chrome.size, browser: browserOpen, inspector: inspectorOpen,
                                               railRight: railRight, largeText: largeText,
                                               minimumHeaderHeight: headingHeight, minimumStatusHeight: 2 * ceil(status.font.lineHeight) + 8,
                                               compactChrome: compactChrome, pinnedBrowser: pinnedBrowser, railContentHeight: railHeight,
                                               inspectorContentHeight: inspectorHeight, navigationContentHeight: navHeight, leftBrowser: leftBrowser,
                                               historyExpanded: historyExpanded,
                                               inspectorPreferredWidth: modellingInspector?.compact == true ? 380 : 320,
                                               historyToggleWidth: featureHistory.preferredToggleWidth,
                                               viewButtonSize: viewButton.intrinsicContentSize)
                .placed(in: chrome, viewport: viewport, useTopSafeArea: topStripHeaderEnabled)
        }
        let available = resolve(railHeight: chrome.height, navHeight: chrome.height)
        func contentHeight(_ stack: UIStackView, width: CGFloat) -> CGFloat {
            stack.systemLayoutSizeFitting(CGSize(width: width, height: 0),
                withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel).height
        }
        let resolved = resolve(railHeight: contentHeight(rail, width: available.rail.width),
                               navHeight: contentHeight(navigationActions, width: available.navigation.width))
        if canvas.frame != resolved.canvas {
            canvas.cancelInput(); modelValue = nil; modelValueButton.isHidden = true
            sketchInput = nil; endpointFinishButton.isHidden = true
        }
        layout = resolved
        top.frame = resolved.topBar; status.frame = resolved.status; canvas.frame = resolved.canvas
        railScroll.frame = resolved.rail
        featureHistory.frame = resolved.featureHistory
        let compact = resolved.compact
        if resolved.floating {
            let statusSize = status.sizeThatFits(resolved.status.size)
            status.frame.size.width = min(resolved.status.width, statusSize.width)
            let titleWidth = max(0, top.bounds.width - 64)
            titleLabel.frame = CGRect(x: 12, y: 5, width: titleWidth, height: titleHeight)
            subtitle.frame = CGRect(x: 12, y: titleHeight + 5, width: titleWidth, height: ceil(subtitle.font.lineHeight) + 2)
            documentActionsButton.frame = CGRect(x: max(0, top.bounds.width - 48), y: 4, width: min(44, top.bounds.width), height: max(0, compactHeadingHeight - 8))
            finishSketchButton.frame = CGRect(x: 8, y: compactHeadingHeight, width: max(0, top.bounds.width - 16),
                height: max(0, top.bounds.height - compactHeadingHeight - 8))
            browserButton.frame = resolved.browserAccess
            navigationScroll.frame = resolved.navigation
            viewButton.frame = resolved.viewAccess
            historyActions.frame = resolved.history
        } else {
            let titleWidth = compact ? top.bounds.width - 24 : max(140, top.bounds.width - min(520, topActions.systemLayoutSizeFitting(UIView.layoutFittingCompressedSize).width) - 36)
            titleLabel.frame = CGRect(x: 12, y: 4, width: max(0, titleWidth), height: titleHeight)
            subtitle.frame = CGRect(x: 12, y: titleHeight + 4, width: max(0, titleWidth), height: subtitleHeight)
            let controlsY = titleHeight + subtitleHeight + 8
            topScroll.frame = compact ? CGRect(x: 8, y: controlsY, width: max(0, top.bounds.width - 16), height: max(44, top.bounds.height - controlsY)) : CGRect(x: titleWidth + 24, y: 4, width: max(0, top.bounds.width - titleWidth - 32), height: max(44, top.bounds.height - 8))
        }
        browserPanel.isHidden = resolved.browser == nil
        if resolved.browser == nil { browserSearchField.resignFirstResponder() }
        inspectorPanel.isHidden = resolved.inspector == nil
        if let frame = resolved.browser { browserPanel.frame = frame }
        if let frame = resolved.inspector { inspectorPanel.frame = frame }
        scrim.frame = CGRect(x: 0, y: resolved.rail.minY, width: workspace.bounds.width, height: resolved.canvas.height)
        scrim.isHidden = !compact || resolved.browser == nil && resolved.inspector == nil
        browserPanel.layoutIfNeeded(); inspectorPanel.layoutIfNeeded()
        let browserBounds = browserPanel.content.bounds
        let searchHeight = browserSearchOpen ? min(browserBounds.height, browserSearchBar.systemLayoutSizeFitting(
            CGSize(width: max(0, browserBounds.width - 24), height: 0),
            withHorizontalFittingPriority: .required, verticalFittingPriority: .fittingSizeLevel).height + 12) : 0
        browserSearchBar.frame = CGRect(x: 12, y: 0, width: max(0, browserBounds.width - 24), height: max(0, searchHeight - 12))
        table.frame = CGRect(x: 0, y: searchHeight, width: browserBounds.width, height: max(0, browserBounds.height - searchHeight))
        inspectorScroll.frame = inspectorPanel.content.bounds
        browserPanel.accessibilityViewIsModal = compact && browserOpen
        inspectorPanel.accessibilityViewIsModal = compact && inspectorOpen
        workspace.accessibilityElements = resolved.floating ? [top, browserButton, railScroll, browserPanel, navigationScroll, inspectorPanel, modelValueButton, endpointFinishButton, canvas, historyActions, featureHistory, status] : [top, railScroll, browserPanel, modelValueButton, endpointFinishButton, canvas, inspectorPanel, featureHistory, status]
        if resetChromeOffsets {
            // Reparenting the same controls changes scroll content dimensions.
            // Discard the old text-toolbar offsets once, after the new frames
            // exist; normal user scrolling remains untouched thereafter.
            [topScroll, railScroll, navigationScroll].forEach { $0.setContentOffset(.zero, animated: false) }
            resetChromeOffsets = false
        }
        updateAvailability()
        canvas.updateDrawableSize()
    }
    func activate() { loadViewIfNeeded(); active = true; session.activate(); updateSurface() }
    func suspend() {
        guard active else { return }
        viewControlsPopover?.dismiss(animated: false); viewControlsPopover = nil
        historyActivation = nil
        featureHistory.cancelScrub()
        active = false
        endpointFinishButton.isHidden = true
#if DEBUG
        groupedInspectorFrameReady = false
#endif
        // Suspension synchronously invalidates the Rust token. Drop local draft
        // ownership now and ignore queued callbacks, including a pending Begin.
        numericGeneration &+= 1
        planeGeneration &+= 1; planeDraft = nil; planeConfirming = false; planeChoosing = false
        clearModelling()
        clearTransform()
        numericToken = nil; numericContext = nil; numericStarting = false
        clearNumericEditor()
        rebuildInspector(); updateAvailability()
        canvas.cancelInput(); session.suspend()
    }
    func shutdown() {
        historyActivation = nil
        featureHistory.cancelScrub()
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
    func checkLineFinishOnDevice() {
        let expected = generation
        var finished = false
        var issues: [String] = []
        func report(_ problem: String? = nil) {
            guard !finished else { return }; finished = true
            if let problem { issues.append(problem) }
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let data = try JSONSerialization.data(withJSONObject: ["passed": issues.isEmpty, "issues": issues,
                    "candidate": Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped",
                    "physical_contacts": false], options: [.prettyPrinted, .sortedKeys])
                try data.write(to: directory.appendingPathComponent("line-finish-check.json"), options: .atomic)
            } catch { print("Line finish report failed: \(error)") }
            print("Line finish check \(issues.isEmpty ? "PASS" : "FAIL"): \(issues)")
        }
        func wait(_ ready: @escaping () -> Bool, remaining: Int = 100, then next: @escaping () -> Void) {
            guard active, generation == expected, !finished else { return }
            if ready() { next(); return }
            guard remaining > 0 else { report("Timed out waiting for native line completion"); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { wait(ready, remaining: remaining - 1, then: next) }
        }
        func capture(_ name: String) {
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let surface: UIView = view.window ?? view
                let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
                guard let png = image.pngData() else { report("Screenshot encoding failed"); return }
                try png.write(to: directory.appendingPathComponent("line-finish-" + name + ".png"), options: .atomic)
            } catch { report(error.localizedDescription) }
        }
        func tap(_ point: [Double], then next: @escaping () -> Void) {
            session.developmentSketchTap(point) { result in
                if case .failure(let error) = result { report(error.description); return }
                self.session.command("view.grid", params: ["visible": self.gridVisible]) { result in
                    if case .failure(let error) = result { report(error.description); return }
                    self.gridAxesCheckAfterFrame = { DispatchQueue.main.async { next() } }
                }
            }
        }
        let commands: [(String, [String: Any])] = [("file.new", [:]), ("sketch.create", ["plane": "XY"]), ("tool.set", ["tool": "sketch.line"])]
        func step(_ index: Int) {
            guard !finished else { return }
            if index < commands.count {
                session.command(commands[index].0, params: commands[index].1) { result in
                    if case .failure(let error) = result { report(error.description); return }; step(index + 1)
                }
                return
            }
            tap([-20, -10, 0]) {
                if !self.endpointFinishButton.isHidden { issues.append("Button appeared before the first line") }
                tap([0, 10, 0]) {
                    tap([25, 10, 0]) {
                        wait({ !self.endpointFinishButton.isHidden && self.pending.isEmpty && self.state?.sketch?.curves.count == 2 }) {
                            let button = self.endpointFinishButton
                            if self.state?.sketch?.curves.count != 2 { issues.append("Expected two committed lines") }
                            if button.bounds.width < 44 || button.bounds.height < 44 || button.configuration?.image == nil { issues.append("Missing artwork or touch target") }
                            if self.workspace.hitTest(button.center, with: nil) !== button { issues.append("Endpoint button cannot receive taps") }
                            if let position = self.sketchInput?.finish_chain_at {
                                let endpoint = CGPoint(x: self.canvas.frame.minX + position[0], y: self.canvas.frame.minY + position[1])
                                if hypot(button.center.x - endpoint.x, button.center.y - endpoint.y) > 90 { issues.append("Button too far from endpoint") }
                                if button.frame.contains(endpoint) { issues.append("Button covers endpoint") }
                            } else { issues.append("Missing endpoint projection") }
                            capture("ready")
                            button.sendActions(for: .touchUpInside)
                            wait({ self.sketchInput?.finish_chain_at == nil && self.pending.isEmpty }) {
                                if !button.isHidden || self.state?.sketch?.curves.count != 2 || self.state?.sketch == nil { issues.append("Finish changed geometry or closed the sketch") }
                                capture("finished"); report()
                            }
                        }
                    }
                }
            }
        }
        browserOpen = false; inspectorOpen = false; relayoutPanels()
        DispatchQueue.main.asyncAfter(deadline: .now() + 35) { report("Diagnostic timed out") }
        step(0)
    }

    func startGroupedInspectorCheckIfReady() {
        guard active, groupedInspectorFrameReady, state != nil, !developmentCheckStarted,
              ProcessInfo.processInfo.arguments.contains("--grouped-inspector-check") else { return }
        developmentCheckStarted = true
        let expected = generation
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            guard self.active, self.generation == expected, self.groupedInspectorFrameReady, self.state != nil else {
                self.developmentCheckStarted = false
                return
            }
            self.checkGroupedInspectorOnDevice()
        }
    }

    /// Real UIKit sizing, scrolling and completion on a disposable native sketch.
    func checkSketchToolbarOnDevice() {
        let expected = generation
        let savedAppearance = appearance
        let variants = [(2, "select"), (2, "sketch.line"),
                        (1, "sketch.spline.fit_point"), (1, "sketch.delete"), (2, "sketch.delete"), (1, "select"), (2, "select")]
        var issues: [String] = []
        var finished = false
        func report() {
            guard !finished else { return }; finished = true
            appearance = savedAppearance
            applyAppearance(); configureChrome(); updateAvailability(); view.layoutIfNeeded()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let data = try JSONSerialization.data(withJSONObject: ["passed": issues.isEmpty, "issues": issues, "variants": variants.count], options: [.prettyPrinted, .sortedKeys])
                try data.write(to: directory.appendingPathComponent("sketch-toolbar-check.json"), options: .atomic)
            } catch { print("Sketch toolbar report failed: \(error)") }
            print("Sketch toolbar check \(issues.isEmpty ? "PASS" : "FAIL"): \(issues)")
        }
        func waitForFinish(_ remaining: Int = 100) {
            guard active, generation == expected, !finished else { return }
            if state?.sketch == nil && pending.isEmpty { report(); return }
            guard remaining > 0 else { issues.append("Finish sketch button did not exit the sketch"); report(); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { waitForFinish(remaining - 1) }
        }
        func waitForTool(_ tool: String, remaining: Int = 100, then action: @escaping () -> Void) {
            guard active, generation == expected, !finished else { return }
            if state?.sketch != nil && state?.active_tool == tool && pending.isEmpty { action(); return }
            guard remaining > 0 else { issues.append("workspace did not publish tool: \(tool)"); report(); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { waitForTool(tool, remaining: remaining - 1, then: action) }
        }
        func check(_ index: Int) {
            guard active, generation == expected, !finished else { return }
            guard index < variants.count else {
                finishSketchButton.sendActions(for: .touchUpInside); waitForFinish(); return
            }
            let (style, tool) = variants[index]
            appearance = style
            applyAppearance(); configureChrome()
            session.command("tool.set", params: ["tool": tool]) { result in
                guard case .success = result else { issues.append("tool selection failed: \(tool)"); report(); return }
                waitForTool(tool) {
                    self.updateAvailability(); self.view.setNeedsLayout(); self.view.layoutIfNeeded()
                    self.railScroll.layoutIfNeeded()
                    if self.finishSketchButton.isHidden || !self.finishSketchButton.isEnabled {
                        issues.append("completion unavailable: \(index)")
                    }
                    if self.compactChrome {
                        self.railScroll.setContentOffset(CGPoint(x: 0, y: max(0, self.railScroll.contentSize.height - self.railScroll.bounds.height)), animated: false)
                        let frame = self.finishSketchButton.convert(self.finishSketchButton.bounds, to: self.top)
                        if !self.top.bounds.contains(frame) || frame.height < 44 || self.finishSketchButton.configuration?.title != SketchText.text("finish") {
                            issues.append("labelled completion is outside header: \(index)")
                        }
                        for button in self.rail.arrangedSubviews.compactMap({ $0 as? WorkspaceButton }) where !button.isHidden {
                            if button.configuration?.image == nil || button.configuration?.title != nil || button.bounds.height < 44 || button.bounds.height > 52 {
                                issues.append("icon rail has a missing icon or wrapped title: \(button.accessibilityLabel ?? "")")
                            }
                        }
                        let point = self.finishSketchButton.convert(CGPoint(x: self.finishSketchButton.bounds.midX, y: self.finishSketchButton.bounds.midY), to: self.workspace)
                        if self.workspace.hitTest(point, with: nil) !== self.finishSketchButton { issues.append("completion cannot be tapped: \(index)") }
                    }
                    do {
                        let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                        let surface: UIView = self.view.window ?? self.view
                        let capture = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
                        if let png = capture.pngData() { try png.write(to: directory.appendingPathComponent("sketch-toolbar-\(index).png"), options: .atomic) }
                    } catch { issues.append("capture failed: \(error)") }
                    check(index + 1)
                }
            }
        }
        let fixture: [(String, [String: Any])] = [("file.new", [:]), ("sketch.create", ["plane": "XY"]),
            ("sketch.rectangle.two_point", ["p0": [0, 0], "p1": [20, 20]]), ("view.fit", [:])]
        func prepare(_ index: Int) {
            guard active, generation == expected else { return }
            guard index < fixture.count else { browserOpen = false; inspectorOpen = false; check(0); return }
            session.command(fixture[index].0, params: fixture[index].1) { result in
                guard case .success = result else { issues.append("fixture failed"); report(); return }
                prepare(index + 1)
            }
        }
        prepare(0)
    }

    /// Disposable real-engine fixture and production UIKit constraint actions.
    /// Actual Pencil contact is separate from these synthetic selections.
    private func checkConstraintsMenuOnDevice() {
        let expected = generation
        var issues: [String] = []
        var finished = false
        func check(_ value: Bool, _ message: String) { if !value { issues.append(message) } }
        func capture(_ name: String) {
            view.layoutIfNeeded()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let surface: UIView = view.window ?? view
                let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
                if let png = image.pngData() { try png.write(to: directory.appendingPathComponent("constraints-menu-" + name + ".png"), options: .atomic) }
            } catch { issues.append("capture: \(error)") }
        }
        func finish() {
            guard !finished else { return }; finished = true
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let receipt: [String: Any] = ["passed": issues.isEmpty, "issues": issues,
                    "candidate": Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped",
                    "at": ISO8601DateFormatter().string(from: Date())]
                try JSONSerialization.data(withJSONObject: receipt, options: [.prettyPrinted, .sortedKeys])
                    .write(to: directory.appendingPathComponent("constraints-menu-check.json"), options: .atomic)
            } catch { print("Constraints menu report failed: \(error)") }
            print("Constraints menu check " + (issues.isEmpty ? "PASS" : "FAIL: " + issues.joined(separator: "; ")))
        }
        func wait(_ stage: String, remaining: Int = 100, until condition: @escaping () -> Bool, then next: @escaping () -> Void) {
            guard active, generation == expected, !finished else { return }
            if condition() { DispatchQueue.main.asyncAfter(deadline: .now() + 0.3, execute: next); return }
            guard remaining > 0 else { issues.append("timed out: " + stage); finish(); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { wait(stage, remaining: remaining - 1, until: condition, then: next) }
        }
        func verifyConstraintFirst() {
            self.run("tool.set", params: ["tool": "select"])
            self.session.command("select.clear") { result in
                guard case .success = result else { issues.append("clear selection failed"); finish(); return }
                wait("empty selection", until: { self.state?.selection.isEmpty == true && self.pending.isEmpty }) {
                    guard let original = self.state, let sketch = original.sketch, sketch.curves.count == 2 else {
                        issues.append("missing constraint-first fixture"); finish(); return
                    }
                    self.constraintsButton.sendActions(for: .primaryActionTriggered)
                    wait("empty selection sheet", until: { self.presentedViewController is UIAlertController && self.presentedViewController?.isBeingPresented == false }) {
                        guard let sheet = self.presentedViewController as? UIAlertController,
                              let apply = self.constraintMenuActions["parallel"] else { issues.append("constraint-first menu missing"); finish(); return }
                        check(sheet.actions.filter { $0.style != .cancel }.allSatisfy(\.isEnabled), "empty selection disabled constraint choices")
                        capture("empty-selection")
                        sheet.dismiss(animated: false) {
                            apply()
                            wait("armed constraint", until: { self.state?.active_tool == "sketch.constraint.parallel" && self.pending.isEmpty }) {
                                check(self.state?.revision == original.revision && self.state?.sketch?.constraints.count == sketch.constraints.count, "arming changed the document")
                                capture("armed")
                                self.session.selectEntity(["type": "sketch_curve", "id": sketch.curves[0].id, "sketch": sketch.id], additive: false) { result in
                                    guard case .success = result else { issues.append("constraint-first first pick failed"); finish(); return }
                                    wait("first constraint pick", until: { self.state?.selection.count == 1 && self.pending.isEmpty }) {
                                        check(self.state?.sketch?.constraints.count == sketch.constraints.count, "first pick applied a two-entity constraint")
                                        capture("first-pick")
                                        self.session.selectEntity(["type": "sketch_curve", "id": sketch.curves[1].id, "sketch": sketch.id], additive: false) { result in
                                            guard case .success = result else { issues.append("constraint-first second pick failed"); finish(); return }
                                            wait("constraint-first applied", until: { self.pending.isEmpty && self.state?.sketch?.constraints.count == sketch.constraints.count + 1 }) {
                                                check(self.state?.selection.isEmpty == true && self.state?.active_tool == "sketch.constraint.parallel", "constraint did not remain armed for repeat use")
                                                check(self.keyboardTop == nil && !self.textEntryActive, "constraint-first raised text input")
                                                capture("constraint-first-applied")
                                                self.run("edit.undo")
                                                wait("constraint-first undo", until: { self.pending.isEmpty && self.state?.sketch?.constraints.map(\.id) == sketch.constraints.map(\.id) }) {
                                                    self.setTool("select")
                                                    wait("select exits constraint", until: { self.state?.active_tool == "select" }) { capture("constraint-first-undo"); finish() }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        func verify() {
            guard let original = state else { issues.append("missing workspace"); finish(); return }
            let frame = canvas.frame
            let constraintIDs = original.sketch?.constraints.map(\.id)
            func unchanged() {
                check(self.state?.document_id == original.document_id && self.state?.revision == original.revision &&
                      self.state?.geometry_revision == original.geometry_revision && self.state?.selection == original.selection &&
                      self.state?.can_undo == original.can_undo && self.state?.can_redo == original.can_redo,
                      "opening/dismissing changed document, history or selection")
                check(self.canvas.frame == frame && self.keyboardTop == nil && !self.textEntryActive && !self.numericEditing,
                      "opening/dismissing changed viewport or raised text input")
            }
            func open(_ next: @escaping (UIAlertController) -> Void) {
                self.constraintsButton.sendActions(for: .primaryActionTriggered)
                wait("constraint sheet", until: { self.presentedViewController is UIAlertController && self.presentedViewController?.isBeingPresented == false }) {
                    guard let sheet = self.presentedViewController as? UIAlertController else { return }
                    check(sheet.preferredStyle == .actionSheet && sheet.textFields?.isEmpty != false,
                          "constraint choices requested text entry (style: \(sheet.preferredStyle.rawValue), fields: \(sheet.textFields?.count ?? 0))")
                    check(self.constraintsButton.menu == nil && !self.constraintsButton.showsMenuAsPrimaryAction, "native menu preview remains installed")
                    check(sheet.actions.count == Self.constraintChoices.count + 1, "missing constraint choices or Cancel")
                    for key in Self.constraintChoices {
                        check(sheet.actions.first(where: { $0.title == SketchText.text(key) })?.isEnabled == true, "constraint cannot be armed: " + key)
                    }
                    unchanged(); next(sheet)
                }
            }
            capture("selected")
            open { sheet in
                capture("opened")
                sheet.dismiss(animated: false) {
                    unchanged()
                    open { reopened in
                        capture("reopened")
                        guard reopened.actions.first(where: { $0.title == SketchText.text("parallel") })?.isEnabled == true,
                              let apply = self.constraintMenuActions["parallel"] else { issues.append("Parallel unavailable for two edges"); finish(); return }
                        reopened.dismiss(animated: false) {
                            apply()
                            wait("parallel applied", until: { self.pending.isEmpty && self.state?.sketch?.constraints.count == (constraintIDs?.count ?? 0) + 1 }) {
                                check(self.state?.sketch?.constraints.contains(where: { $0.name.lowercased() == "parallel" }) == true, "wrong constraint applied")
                                check(self.keyboardTop == nil && !self.textEntryActive, "applying Parallel raised text input")
                                capture("applied")
                                self.run("edit.undo")
                                wait("undo", until: { self.pending.isEmpty && self.state?.sketch?.constraints.map(\.id) == constraintIDs }) {
                                    capture("undo"); verifyConstraintFirst()
                                }
                            }
                        }
                    }
                }
            }
        }
        let fixture: [(String, [String: Any])] = [("file.new", [:]), ("sketch.create", ["plane": "XY"]),
            ("sketch.line", ["points": [[-20, -10], [15, -8]], "infer": false, "connect": false]),
            ("sketch.line", ["points": [[-18, 10], [20, 15]], "infer": false, "connect": false]),
            ("tool.set", ["tool": "select"]), ("view.fit", [:])]
        func prepare(_ index: Int) {
            guard active, generation == expected, !finished else { return }
            guard index < fixture.count else {
                wait("fixture edges", until: { self.state?.sketch?.curves.count == 2 && self.pending.isEmpty }) {
                    guard let sketch = self.state?.sketch else { return }
                    self.browserOpen = false; self.inspectorOpen = false; self.relayoutPanels()
                    self.session.selectEntity(["type": "sketch_curve", "id": sketch.curves[0].id, "sketch": sketch.id], additive: false) { result in
                        guard case .success = result else { issues.append("first edge selection failed"); finish(); return }
                        self.session.selectEntity(["type": "sketch_curve", "id": sketch.curves[1].id, "sketch": sketch.id], additive: true) { result in
                            guard case .success = result else { issues.append("second edge selection failed"); finish(); return }
                            wait("two selected edges", until: { self.state?.selection.count == 2 && self.pending.isEmpty }, then: verify)
                        }
                    }
                }
                return
            }
            session.command(fixture[index].0, params: fixture[index].1) { result in
                if case .failure(let error) = result {
                    issues.append("fixture command failed: " + fixture[index].0 + ": " + error.description); finish(); return
                }
                prepare(index + 1)
            }
        }
        prepare(0)
    }

    /// Uses production joint forms and dismissal handlers on a disposable launch
    /// fixture. Input is synthetic; this does not verify physical Pencil hardware.
    /// Actual compact UIKit controls with synthetic fields; no engine command or library mutation.
    func checkGroupedInspectorOnDevice() {
        let prefix = "--grouped-inspector-run="
        let runID = ProcessInfo.processInfo.arguments.first(where: { $0.hasPrefix(prefix) }).map { String($0.dropFirst(prefix.count)) } ?? "missing"
        guard let directory = try? FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true) else {
            NSLog("Grouped inspector RUN %@ FAIL: evidence directory unavailable", runID); return
        }
        var report = ["RUN " + runID, "START compact UIKit fixture; synthetic actions, no engine/Pencil/VoiceOver claim"]
        func saveReport() {
            let text = report.joined(separator: "\n") + "\n"; NSLog("%@", text)
            do { try text.write(to: directory.appendingPathComponent("grouped-inspector-check.log"), atomically: true, encoding: .utf8) }
            catch { NSLog("Grouped inspector evidence write failed: %@", error.localizedDescription) }
        }
        saveReport()
        guard let workspace = state, UUID(uuidString: runID) != nil else {
            report.append("FAIL missing workspace or valid --grouped-inspector-run UUID"); saveReport(); return
        }
        func require(_ condition: Bool, _ message: String) throws {
            if !condition { throw NSError(domain: "GroupedInspectorCheck", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
        }
        func descendants(_ root: UIView) -> [UIView] { [root] + root.subviews.flatMap(descendants) }
        func find<T: UIView>(_ id: String, in root: UIView, as type: T.Type) throws -> T {
            guard let value = descendants(root).first(where: { $0.accessibilityIdentifier == id }) as? T else {
                throw NSError(domain: "GroupedInspectorCheck", code: 2, userInfo: [NSLocalizedDescriptionKey: "Missing " + id])
            }; return value
        }
        func actions(_ elements: [UIMenuElement]) -> [UIAction] {
            elements.flatMap { element in
                if let action = element as? UIAction { return [action] }
                if let menu = element as? UIMenu { return actions(menu.children) }
                return []
            }
        }
        func menuAction(_ title: String, from button: UIButton) throws -> UIAction {
            guard let action = actions(button.menu?.children ?? []).first(where: { $0.title == title }) else {
                throw NSError(domain: "GroupedInspectorCheck", code: 3, userInfo: [NSLocalizedDescriptionKey: "Missing action " + title])
            }; return action
        }
        func field(_ key: String, kind: String, options: [ModellingState.Field.Option], units: String? = nil) -> ModellingState.Field {
            .init(key: key, label: "ui.ipados.modelling.field." + key, kind: kind, options: options, units: units, min: nil, max: nil, value: nil)
        }
        let profile = PresentationJSON.object(["face_body": .string("Body1"), "faces": .array([.array([.integer(0), .integer(0), .integer(20)])])])
        let fields = [
            field("profile", kind: "reference", options: [.init(label: "Body1", value: profile)]),
            field("distance", kind: "expression", options: [], units: "mm"),
            field("operation", kind: "inline_choice", options: [("new", "new_body"), ("join", "union"), ("intersect", "intersect"), ("cut", "subtract")].map {
                .init(label: "ui.ipados.face_extrude." + $0.1, value: .string($0.0)) }),
            field("direction", kind: "choice", options: ["positive", "negative", "symmetric"].map { .init(label: $0, value: .string($0)) }),
            field("through_all", kind: "choice", options: [.init(label: "false", value: .bool(false)), .init(label: "true", value: .bool(true))]),
            field("targets", kind: "references", options: ["Body1", "Body2", "Body3"].map { .init(label: $0, value: .string($0)) })
        ]
        func model(_ params: [String: PresentationJSON]) -> ModellingState {
            .init(token: 9001, operation: "solid.extrude", feature: nil, params: params, fields: fields, valid: true,
                  feedback: [], reply: nil, handle: nil, workspace: workspace)
        }
        do {
            let initial: [String: PresentationJSON] = ["profile": profile, "distance": .string("5 mm"), "operation": .string("new"),
                "direction": .string("positive"), "through_all": .bool(false), "targets": .array([.string("Body1"), .string("Body2")])]
            let form = ModellingInspector(state: model(initial), compact: true); form.evaluated(model(initial))
            let more = try find("workspace-model-more-options", in: form, as: UIButton.self); more.sendActions(for: .touchUpInside)
            let apply = try find("workspace-model-apply", in: form.footer, as: UIButton.self)
            let oldRemove = try find("workspace-model-reference-targets-0-remove", in: form, as: UIButton.self)
            let oldDirection = try find("workspace-model-field-direction", in: form, as: UIButton.self)
            let oldChoice = try menuAction(ModellingFormText.text("negative"), from: oldDirection)
            try require(apply.isEnabled, "validated fixture did not enable Apply")
            let toggle = try find("workspace-model-field-through_all", in: form, as: UISwitch.self)
            toggle.setOn(true, animated: false); toggle.sendActions(for: .valueChanged)
            try require(form.params["through_all"] == .bool(true) && !apply.isEnabled, "switch lost Boolean payload or reused validated Apply")
            oldRemove.sendActions(for: .primaryActionTriggered); oldChoice.performWithSender(oldDirection, target: nil)
            try require(form.params["targets"] == initial["targets"] && form.params["direction"] == initial["direction"], "stale controls changed a rebuilt form")
            form.setBusy(committing: false, allowInput: true)
            let busyToggle = try find("workspace-model-field-through_all", in: form, as: UISwitch.self)
            try require(!busyToggle.isEnabled, "busy switch remains enabled for accessibility")
            busyToggle.setOn(false, animated: false); busyToggle.sendActions(for: .valueChanged)
            try require(busyToggle.isOn && form.params["through_all"] == .bool(true), "busy switch disagreed with its draft value")
            form.evaluated(model(form.params))
            let restoredToggle = try find("workspace-model-field-through_all", in: form, as: UISwitch.self)
            try require(restoredToggle.isEnabled && restoredToggle.isOn, "completed preview did not restore switch availability/value")
            let direction = try find("workspace-model-field-direction", in: form, as: UIButton.self)
            try menuAction(ModellingFormText.text("negative"), from: direction).performWithSender(direction, target: nil)
            try require(form.params["direction"] == .string("negative") && !apply.isEnabled, "choice changed the engine value or reused Apply")
            let reorder = try find("workspace-model-reference-targets-0-reorder", in: form, as: UIButton.self)
            try menuAction(ModellingText.text("move_down"), from: reorder).performWithSender(reorder, target: nil)
            try require(form.params["targets"] == .array([.string("Body2"), .string("Body1")]), "Move down did not preserve target order")
            try find("workspace-model-reference-targets-0-remove", in: form, as: UIButton.self).sendActions(for: .primaryActionTriggered)
            try require(form.params["targets"] == .array([.string("Body1")]), "Remove changed the wrong target")
            try require(!descendants(form).contains { $0.accessibilityIdentifier == "workspace-model-reference-targets-0-reorder" }, "singleton still displays reorder")
            let add = try find("workspace-model-field-targets", in: form, as: UIButton.self)
            let addThird = try menuAction("Body3", from: add)
            addThird.performWithSender(add, target: nil); addThird.performWithSender(add, target: nil)
            try require(form.params["targets"] == .array([.string("Body1"), .string("Body3")]), "Add duplicated a reference or changed its order")
            let refreshedAdd = try find("workspace-model-field-targets", in: form, as: UIButton.self)
            let names = actions(refreshedAdd.menu?.children ?? []).map(\.title)
            try require(names.contains("Body2") && !names.contains("Body1") && !names.contains("Body3"), "selected references remain in the nested add menu")
            var selectedField: String?; form.onSelect = { selectedField = $0 }
            let beforeSelection = form.params
            try menuAction(ModellingFormText.text("select_in_model"), from: refreshedAdd).performWithSender(refreshedAdd, target: nil)
            try require(selectedField == "targets" && form.params == beforeSelection, "viewport action lost its target field or changed the draft")
            let disabledButton = ModellingFormButton(title: "Empty", symbol: "plus", label: "Empty", identifier: "grouped-check-empty")
            disabledButton.isEnabled = false; let disabledGroup = ModellingFormGroup(); disabledGroup.append(disabledButton)
            disabledGroup.setEditingEnabled(false); disabledGroup.setEditingEnabled(true)
            try require(!disabledButton.isEnabled, "busy recovery enabled an unavailable menu")
            report.append("PASS typed values, Apply, busy restoration, stale actions, ordered references, duplicates and target viewport callback"); saveReport()
            let variants: [(CGFloat, UIContentSizeCategory, UIUserInterfaceStyle, String)] = [
                (320, .large, .light, "320-light"), (380, .large, .dark, "380-dark"),
                (320, .accessibilityExtraExtraExtraLarge, .dark, "320-xxxl-dark"), (380, .accessibilityExtraExtraExtraLarge, .light, "380-xxxl-light")]
            for (width, category, style, name) in variants {
                let host = UIViewController(); host.loadViewIfNeeded(); host.overrideUserInterfaceStyle = style
                host.traitOverrides.preferredContentSizeCategory = category
                addChild(host); view.addSubview(host.view); host.didMove(toParent: self)
                defer { host.willMove(toParent: nil); host.view.removeFromSuperview(); host.removeFromParent() }
                host.view.frame = CGRect(x: 0, y: 0, width: width, height: min(900, max(600, view.bounds.height - 24)))
                let specimen = ModellingInspector(state: model(form.params), compact: true)
                let panel = WorkspacePanel(title: specimen.title, onClose: {}); panel.frame = host.view.bounds
                host.view.addSubview(panel); panel.setFooter(specimen.footer)
                let scroll = UIScrollView(); scroll.translatesAutoresizingMaskIntoConstraints = false; panel.content.addSubview(scroll)
                specimen.translatesAutoresizingMaskIntoConstraints = false; scroll.addSubview(specimen)
                NSLayoutConstraint.activate([
                    scroll.leadingAnchor.constraint(equalTo: panel.content.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: panel.content.trailingAnchor),
                    scroll.topAnchor.constraint(equalTo: panel.content.topAnchor), scroll.bottomAnchor.constraint(equalTo: panel.content.bottomAnchor),
                    specimen.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 16),
                    specimen.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -16),
                    specimen.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 12),
                    specimen.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -12),
                    specimen.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -32)])
                let disclosure = try find("workspace-model-more-options", in: specimen, as: UIButton.self)
                disclosure.sendActions(for: .touchUpInside); host.view.layoutIfNeeded()
                for control in descendants(specimen).compactMap({ $0 as? UIControl }).filter({ $0 is ModellingFormButton || $0 is ModellingFormSwitch }) {
                    let frame = control.convert(control.bounds, to: specimen)
                    try require(specimen.bounds.insetBy(dx: -0.5, dy: -0.5).contains(frame), name + ": grouped control escaped content bounds")
                    let touchHeight = control is UISwitch ? control.point(inside: CGPoint(x: control.bounds.midX, y: control.bounds.midY + 21), with: nil) : frame.height >= 44
                    try require(frame.width >= 44 && touchHeight, name + ": grouped control lost its touch target")
                }
                for id in ["workspace-model-cancel", "workspace-model-preview", "workspace-model-apply"] {
                    let action = try find(id, in: panel, as: UIButton.self), frame = action.convert(action.bounds, to: panel)
                    try require(!action.isDescendant(of: scroll) && panel.bounds.contains(frame) && frame.width >= 44 && frame.height >= 44, name + ": persistent action clipped")
                    if let label = action.titleLabel {
                        let unwrapped = label.sizeThatFits(CGSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude))
                        try require(label.bounds.width + 0.5 >= unwrapped.width, name + ": action title wrapped into narrow columns")
                    }
                }
                let offset = disclosure.convert(disclosure.bounds, to: scroll).minY - 12
                scroll.setContentOffset(CGPoint(x: 0, y: min(max(0, offset), max(0, scroll.contentSize.height - scroll.bounds.height))), animated: false)
                host.view.layoutIfNeeded()
                let capture = UIGraphicsImageRenderer(bounds: host.view.bounds).image { _ in host.view.drawHierarchy(in: host.view.bounds, afterScreenUpdates: true) }
                guard let png = capture.pngData() else { throw NSError(domain: "GroupedInspectorCheck", code: 4) }
                try png.write(to: directory.appendingPathComponent("grouped-inspector-" + name + ".png"), options: .atomic)
                report.append("PASS " + name + ": compact panel, expanded options, 44pt controls, persistent footer and actual UIKit capture"); saveReport()
            }
            report.append("PASS complete; library projects and saved preferences unchanged")
        } catch { report.append("FAIL " + error.localizedDescription) }
        saveReport()
    }

    /// Disposable fixture and synthetic Pencil samples through the native mailbox.
    /// Captures follow matching presented frames; physical Pencil remains separate.
    private func checkSelectionTapsOnDevice() {
        guard let directory = try? FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true) else {
            NSLog("Selection taps check FAIL: evidence directory unavailable"); return
        }
        let started = Date().timeIntervalSince1970
        let run = UUID().uuidString
        let candidate = Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped"
        let expected = generation
        let cases = ["face", "empty", "face-again", "same-face-toggle"]
        var stages: [[String: Any]] = []
        var firstFace: WorkspaceEntity?
        var finished = false
        func save(status: String, failure: String = "") {
            let report: [String: Any] = ["status": status, "passed": status == "passed", "error": failure,
                "run_id": run, "candidate": candidate, "started_at": started,
                "completed_at": status == "running" ? 0 : Date().timeIntervalSince1970,
                "coverage": "Physical runtime; synthetic Pencil samples through InputMailbox; physical Pencil unverified",
                "library_project_untouched": session.currentProject == nil, "stages": stages]
            do {
                try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
                    .write(to: directory.appendingPathComponent("selection-taps-check.json"), options: .atomic)
            } catch { NSLog("Selection taps evidence write FAIL: %@", error.localizedDescription) }
            NSLog("SolidCraft selection taps check %@", String(describing: report))
        }
        func finish(_ failure: String? = nil) {
            guard !finished else { return }; finished = true
            session.endDevelopmentSelectionTapsCheck()
            save(status: failure == nil && stages.count == cases.count ? "passed" : "failed", failure: failure ?? "")
        }
        func step(_ index: Int) {
            guard !finished else { return }
            guard active, generation == expected else { finish("Workspace lifecycle changed"); return }
            if index == cases.count { finish(); return }
            let name = cases[index]
            session.developmentSelectionTap(index == 1 ? nil : [-8, 0, 10]) { [weak self] result in
                guard let self, !finished else { return }
                do {
                    let selected = try result.get()
                    let selectedFace = index == 0 || index == 2
                    guard selectedFace ? selected.count == 1 && selected.first?.type == "face" : selected.isEmpty else {
                        throw BridgeError(code: "selection_taps_check", message: "Unexpected selection after " + name + ": " + selected.map(\.readout).joined(separator: ", "))
                    }
                    if index == 0 { firstFace = selected.first }
                    if index == 2, selected.first != firstFace {
                        throw BridgeError(code: "selection_taps_check", message: "Face target changed after clearing empty space")
                    }
                    self.view.layoutIfNeeded()
                    let surface: UIView = self.view.window ?? self.view
                    let capture = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in
                        surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true)
                    }
                    guard let png = capture.pngData() else { throw BridgeError(code: "selection_taps_check", message: "Screenshot encoding failed") }
                    let filename = "selection-taps-" + name + ".png"
                    try png.write(to: directory.appendingPathComponent(filename), options: .atomic)
                    stages.append(["name": name, "passed": true, "selection": selected.map(\.readout),
                                   "move_points": 6, "lift_points": 8, "screenshot": filename])
                    save(status: "running"); step(index + 1)
                } catch { finish(String(describing: error)) }
            }
        }
        let setup: [(String, [String: Any])] = [
            ("tool.set", ["tool": "select"]), ("select.filter", ["filter": "face"]),
            ("select.multiple", ["enabled": true]), ("view.perspective", ["enabled": false]),
            ("view.top", [:]), ("view.fit", [:]), ("select.clear", [:])
        ]
        func prepare(_ index: Int) {
            guard !finished else { return }
            if index == setup.count { step(0); return }
            session.command(setup[index].0, params: setup[index].1) { result in
                switch result { case .success: prepare(index + 1); case .failure(let error): finish(error.description) }
            }
        }
        save(status: "running")
        guard session.currentProject == nil else { finish("Diagnostic must use the disposable launch fixture"); return }
        // Late workspace reads may still contain an earlier selected face.
        // Keep this disposable session quiet; the saved preference is untouched.
        extrudeOnFaceSelection = false
        browserOpen = false; inspectorOpen = false; relayoutPanels()
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) { if !finished { finish("Diagnostic timed out waiting for a matching presented frame") } }
        prepare(0)
    }

    func checkToolDismissalOnDevice() {
        guard state != nil else {
            assemblyCheckWorkspace = { [weak self] _ in
                self?.assemblyCheckWorkspace = nil; self?.checkToolDismissalOnDevice()
            }
            session.refreshWorkspace()
            return
        }
        let savedPreference = cancelToolsOnDismiss
        let expected = generation
        let cases = ["legacy", "close", "toggle", "browser", "empty"]
        var results: [String] = []
        var finished = false
        func finish(_ failure: String? = nil) {
            guard !finished else { return }; finished = true
            cancelToolsOnDismiss = savedPreference
            session.command("view.tool_dismissal", params: ["cancel_on_dismiss": savedPreference])
            let report: [String: Any] = ["passed": failure == nil && results.count == cases.count,
                                         "cases": results, "failure": failure ?? ""]
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
                    .write(to: directory.appendingPathComponent("tool-dismissal-check.json"), options: .atomic)
            } catch { NSLog("Tool dismissal report failed: %@", String(describing: error)) }
            NSLog("SolidCraft tool dismissal check %@", String(describing: report))
        }
        func step(_ index: Int) {
            guard active, generation == expected, !finished else { return }
            if index == cases.count { finish(); return }
            let name = cases[index]
            NSLog("SolidCraft tool dismissal case %@", name)
            cancelToolsOnDismiss = name != "legacy"
            session.command("view.tool_dismissal", params: ["cancel_on_dismiss": cancelToolsOnDismiss]) { [weak self] result in
                guard let self, case .success = result else { finish("preference setup failed"); return }
                self.assemblyCheckBegan = { [weak self] result in
                    guard let self, case .success(let model) = result, let form = self.modellingInspector,
                          let token = model.token, self.inspectorOpen else { finish("joint form did not open: \(name)"); return }
                    self.assemblyCheckCancelled = { [weak self] result in
                        guard let self else { return }
                        if case .failure(let error) = result, error.code != "stale_edit" { finish(error.description); return }
                        self.view.layoutIfNeeded()
                        guard !self.modellingEditing, !self.inspectorOpen,
                              !self.modellingButton.isHidden, !self.assemblyButton.isHidden,
                              self.modellingButton.isEnabled, self.assemblyButton.isEnabled else {
                            finish("dismissal did not restore the normal tool rail: \(name)"); return
                        }
                        results.append(name); step(index + 1)
                    }
                    switch name {
                    case "legacy":
                        self.closePanels(); self.toggleInspector()
                        guard self.modellingToken == token, self.modellingInspector === form, self.inspectorOpen,
                              self.modellingInspector?.params == form.params else { finish("legacy draft did not reopen intact"); return }
                        self.cancelModelling(); self.closePanels()
                    case "toggle": self.toggleInspector()
                    case "browser": self.toggleBrowser()
                    case "empty":
                        let scale = Double(self.canvas.contentScaleFactor)
                        for phase in [InputSample.Phase.began, .ended] {
                            var sample = InputSample(source: .finger, phase: phase, id: 900,
                                x_points: 180, y_points: 140, x_pixels: 180 * scale, y_pixels: 140 * scale,
                                scale: scale, timestamp: ProcessInfo.processInfo.systemUptime,
                                pressure: nil, altitude: nil, geometry_revision: self.canvas.geometryRevision,
                                consumed: false, shift: false, button: 0, scroll: 0)
                            sample.viewport_epoch = self.canvas.viewportEpoch; sample.camera_epoch = self.canvas.cameraEpoch
                            sample.document_id = self.canvas.documentID
                            self.session.record(sample)
                        }
                    default: self.closePanels()
                    }
                }
                self.beginModelling(operation: "joint.create")
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 20) { if !finished { finish("diagnostic timed out") } }
        step(0)
    }

    /// Disposable launch fixture only. Exercise the real browser button and eye
    /// controls, inspect the long-press menu and capture the actual UIKit views.
    /// Disposable real-engine sketch for physical tap/keyboard/drag verification.
    /// NativeSession's diagnostic launch path never opens or autosaves a project.
    /// Disposable fixture; invokes the production control handlers, then restores
    /// every saved visibility override. It never opens/autosaves a library design.
    private func checkViewControlsOnDevice() {
        let expected = generation
        let savedOrigin = originVisibility, savedGrid = gridVisible, savedIntegratedAxes = integratedGridAxes
        let defaults = UserDefaults.standard
        let savedOriginDefault = defaults.object(forKey: "SolidCraft.workspace.originVisibility")
        let savedGridDefault = defaults.object(forKey: "SolidCraft.workspace.gridVisible")
        var original: WorkspaceState?
        var issues: [String] = []
        func check(_ value: Bool, _ message: String) { if !value { issues.append(message) } }
        func control(_ id: String, in parent: UIView? = nil) -> UIControl? {
            guard let parent = parent ?? viewControlsPopover?.view else { return nil }
            if parent.accessibilityIdentifier == "workspace-view-" + id { return parent as? UIControl }
            for child in parent.subviews { if let found = control(id, in: child) { return found } }
            return nil
        }
        func master() -> UISwitch? { control("origin") as? UISwitch }
        func capture(_ name: String) {
            view.layoutIfNeeded()
            do {
                let folder = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let surface: UIView = view.window ?? view
                let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
                if let png = image.pngData() { try png.write(to: folder.appendingPathComponent("view-controls-" + name + ".png"), options: .atomic) }
            } catch { issues.append("capture: \(error)") }
        }
        func finish() {
            originVisibility = savedOrigin; gridVisible = savedGrid
            defaults.set(savedOriginDefault, forKey: "SolidCraft.workspace.originVisibility")
            defaults.set(savedGridDefault, forKey: "SolidCraft.workspace.gridVisible")
            session.command("view.origin", params: ["visible": NSNull()])
            for (item, visible) in savedOrigin { session.command("view.origin", params: ["item": item, "visible": visible]) }
            session.command("view.grid", params: ["visible": savedGrid, "integrated_axes": savedIntegratedAxes])
            failure = nil; updateStatus(); rebuildViewMenu()
            viewControlsPopover?.dismiss(animated: false); viewControlsPopover = nil
            print("View controls check " + (issues.isEmpty ? "PASS" : "FAIL: " + issues.joined(separator: "; ")))
        }
        func wait(_ condition: @escaping () -> Bool, remaining: Int = 100, then next: @escaping () -> Void) {
            guard active, generation == expected else { return }
            if condition() { DispatchQueue.main.asyncAfter(deadline: .now() + 0.12, execute: next); return }
            guard remaining > 0 else { issues.append("state update timed out"); finish(); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { wait(condition, remaining: remaining - 1, then: next) }
        }
        func unchanged() {
            check(state?.document_id == original?.document_id && state?.geometry_revision == original?.geometry_revision &&
                  state?.can_undo == original?.can_undo && state?.selection == original?.selection, "visibility mutated document/history/selection")
        }
        let steps: [(() -> Void, () -> Bool, () -> Void)] = [
            ({ self.setOriginVisibility(visible: false) }, { self.viewPresentation?.visibility == ViewControlsPresentation.Visibility.none }, {
                check(master()?.isOn == false, "none master indicator"); capture("none")
            }),
            ({ control("origin")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.visibility == .all }, {
                check(master()?.isOn == true, "all master indicator"); capture("all")
            }),
            ({ control("origin")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.visibility == ViewControlsPresentation.Visibility.none }, {}),
            ({
                control("individual-references")?.sendActions(for: .primaryActionTriggered)
                control("reference-X")?.sendActions(for: .valueChanged)
            }, { self.viewPresentation?.visibility == .some }, {
                control("back")?.sendActions(for: .primaryActionTriggered)
                check(master()?.isOn == true && master()?.accessibilityValue == ViewText.text("some_visible"), "mixed master indicator/subtitle")
                capture("mixed")
            }),
            ({ control("origin")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.visibility == .all }, {
                let origin = self.viewPresentation?.origin
                self.viewControlPending = true; self.rebuildViewMenu()
                check(master()?.isEnabled == false, "busy master remains enabled")
                self.setOriginVisibility(visible: false)
                self.viewControlPending = false; self.rebuildViewMenu()
                check(self.viewPresentation?.origin == origin, "busy command changed visibility")
            }),
            ({ control("grid")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.grid.visible != savedGrid }, {
                check(self.viewPresentation?.visibility == .all, "Grid changed references")
            }),
            ({ self.setOriginVisibility(visible: nil) }, { self.viewPresentation?.visibility == ViewControlsPresentation.Visibility.none }, {
                check(self.originVisibility.isEmpty, "Automatic retained overrides")
                check(self.viewPresentation?.grid.visible != savedGrid, "references changed Grid")
            }),
            ({ self.session.command("view.choose_sketch_plane") }, { self.viewPresentation?.origin.choosing == true }, {
                check(self.viewPresentation?.visibility == .all && master()?.isOn == true, "Automatic effective state")
            }),
            ({ self.session.command("view.cancel_plane") }, { self.viewPresentation?.origin.choosing == false }, {
                check(self.viewPresentation?.visibility == ViewControlsPresentation.Visibility.none, "Automatic cancellation state")
            })
        ]
        func step(_ index: Int) {
            guard index < steps.count else {
                let beforeFailure = viewPresentation
                session.command("view.origin", params: ["visible": "invalid"]) { result in
                    if case .success = result { issues.append("invalid command succeeded") }
                    check(self.viewPresentation == beforeFailure && self.originVisibility.isEmpty, "failed command changed state/preferences")
                    unchanged()
                    self.failure = nil; self.updateStatus()
                    control("front")?.sendActions(for: .primaryActionTriggered)
                    wait({ !self.viewControlPending && self.viewPresentation?.camera.standardView == "front" }) {
                        check((control("front") as? UIButton)?.isSelected == true && (control("isometric") as? UIButton)?.isSelected == false, "standard-view selection is not camera-derived")
                        control("isometric")?.sendActions(for: .primaryActionTriggered)
                        wait({ !self.viewControlPending && self.viewPresentation?.camera.standardView == "isometric" }) {
                            control("perspective")?.sendActions(for: .valueChanged)
                            wait({ !self.viewControlPending && (self.viewPresentation?.camera.fov ?? 0) > 0 }) {
                                check((control("perspective") as? UISwitch)?.isOn == true, "Perspective switch is stale")
                                control("perspective")?.sendActions(for: .valueChanged)
                                wait({ !self.viewControlPending && self.viewPresentation?.camera.fov == 0 }) {
                                    check(self.keyboardTop == nil, "View popup opened a keyboard")
                                    unchanged(); capture("panel"); finish()
                                }
                            }
                        }
                    }
                }
                return
            }
            let (run, ready, verify) = steps[index]
            run()
            wait({ !self.viewControlPending && ready() }) { verify(); unchanged(); step(index + 1) }
        }
        wait({ self.viewPresentation != nil && self.state != nil && self.pending.isEmpty }) {
            original = self.state
            let openingState = self.viewPresentation, openingFrame = self.canvas.frame
            self.showViewControls()
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) {
                check(self.viewControlsPopover != nil, "View popup did not open")
                check(self.viewPresentation == openingState && self.canvas.frame == openingFrame, "opening View changed the camera or viewport")
                check(self.keyboardTop == nil, "opening View raised a keyboard")
                capture("opened")
                self.session.command("view.grid", params: ["visible": self.gridVisible, "integrated_axes": false]) { _ in
                    step(0)
                }
            }
        }
    }

    /// Opt-in acceptance for the three deferred changes, on a disposable fixture.
    private func checkDeferredChangesOnDevice() {
        let expected = generation, started = Date().timeIntervalSince1970
        let savedStyle = overrideUserInterfaceStyle, savedLegacy = legacyOriginPlanes
        let savedOrigin = originVisibility
        let keys = ["SolidCraft.workspace.legacyOriginPlanes", "SolidCraft.workspace.originVisibility"]
        let defaults = UserDefaults.standard, savedDefaults = keys.map { UserDefaults.standard.object(forKey: $0) }
        let candidate = Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped"
        guard let directory = try? FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true) else { return }
        var stages: [[String: Any]] = [], issues: [String] = [], finished = false
        func require(_ value: Bool, _ message: String) { if !value { issues.append(message) } }
        func descendants(_ parent: UIView) -> [UIView] { [parent] + parent.subviews.flatMap(descendants) }
        func control(_ id: String) -> UIControl? {
            guard let root = viewControlsPopover?.view else { return nil }
            return descendants(root).first { $0.accessibilityIdentifier == "workspace-view-" + id } as? UIControl
        }
        func report(_ status: String) {
            let value: [String: Any] = ["candidate": candidate, "started_at": started,
                "completed_at": status == "running" ? 0 : Date().timeIntervalSince1970,
                "status": status, "passed": status == "passed", "issues": issues, "stages": stages,
                "library_project_untouched": session.currentProject == nil,
                "coverage": "Physical iPad runtime, production UIKit controls, synthetic finger bridge events, matching presented Metal frames; physical finger/Pencil unverified"]
            do { try JSONSerialization.data(withJSONObject: value, options: [.prettyPrinted, .sortedKeys])
                .write(to: directory.appendingPathComponent("deferred-changes-check.json"), options: .atomic) }
            catch { NSLog("Deferred changes report error: %@", String(describing: error)) }
        }
        func finish(_ error: String? = nil) {
            guard !finished else { return }; finished = true
            if let error { issues.append(error) }
            gridAxesCheckAfterFrame = nil; session.endDevelopmentSelectionTapsCheck()
            viewControlsPopover?.dismiss(animated: false); viewControlsPopover = nil
            overrideUserInterfaceStyle = savedStyle; legacyOriginPlanes = savedLegacy; originVisibility = savedOrigin
            for (key, value) in zip(keys, savedDefaults) { defaults.set(value, forKey: key) }
            session.command("view.origin_style", params: ["legacy": savedLegacy])
            session.command("view.touch_twist", params: ["enabled": twoFingerTwist])
            session.command("view.origin", params: ["visible": NSNull()])
            for (item, visible) in savedOrigin { session.command("view.origin", params: ["item": item, "visible": visible]) }
            report(issues.isEmpty ? "passed" : "failed")
            NSLog("Deferred changes check %@", issues.isEmpty ? "PASS" : "FAIL: " + issues.joined(separator: "; "))
        }
        func capture(_ name: String, details: [String: Any] = [:]) {
            let surface: UIView = view.window ?? view
            surface.layoutIfNeeded()
            viewControlsPopover?.view.layoutIfNeeded()
            let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
            do {
                guard let png = image.pngData() else { throw BridgeError(code: "deferred_check", message: "PNG encoding failed") }
                let filename = "deferred-" + name + ".png"
                try png.write(to: directory.appendingPathComponent(filename), options: .atomic)
                var stage = details; stage["name"] = name; stage["screenshot"] = filename; stages.append(stage); report("running")
            } catch { issues.append(String(describing: error)) }
        }
        func commands(_ values: [(String, [String: Any])], _ done: @escaping () -> Void) {
            guard !finished else { return }
            guard let first = values.first else { done(); return }
            session.command(first.0, params: first.1) { result in
                switch result { case .success: commands(Array(values.dropFirst()), done); case .failure(let error): finish(error.description) }
            }
        }
        func frame(_ name: String, _ done: @escaping () -> Void) {
            commands([("view.grid", ["visible": true])]) {
                self.gridAxesCheckAfterFrame = {
                    // Trait/font changes need their UIKit layout transaction to
                    // settle as well as a matching Metal frame.
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.25) {
                        guard !finished else { return }
                        self.viewControlsPopover?.view.setNeedsLayout()
                        capture(name); done()
                    }
                }
            }
        }
        func wait(_ ready: @escaping () -> Bool, remaining: Int = 100, _ done: @escaping () -> Void) {
            guard !finished else { return }
            guard active, generation == expected else { finish("Workspace lifecycle changed"); return }
            if ready() { done(); return }
            guard remaining > 0 else { finish("Native state timed out"); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { wait(ready, remaining: remaining - 1, done) }
        }
        func inspectIcons() {
            guard let root = viewControlsPopover?.view else { issues.append("View popup missing"); return }
            let nodes = descendants(root)
            let buttons = nodes.compactMap { $0 as? UIButton }.filter { $0.accessibilityIdentifier?.hasPrefix("workspace-view-") == true && $0.configuration != nil }
            let mainPage = control("fit") != nil
            require(buttons.count >= (mainPage ? 9 : 4) && buttons.allSatisfy { $0.configuration?.image != nil }, "View option icon missing")
            let icons = nodes.compactMap { $0 as? UIImageView }.filter { $0.accessibilityIdentifier?.hasPrefix("workspace-view-icon-") == true }
            require(icons.count >= (mainPage ? 4 : 11) && icons.allSatisfy { $0.image != nil }, "View reference/switch icon missing")
            require(self.keyboardTop == nil, "View popup opened the keyboard")
            require(root.backgroundColor == .clear, "Opaque View background leaked outside native corners")
            require((self.viewControlsPopover?.preferredContentSize.height ?? .infinity) <= 560, "View popup exceeded its scrolling content height limit")
            if let window = root.window {
                let panelFrame = root.convert(root.bounds, to: window)
                let historyFrame = self.featureHistory.convert(self.featureHistory.bounds, to: window)
                require(panelFrame.maxY + 8 <= historyFrame.minY, "View popup covered History")
            }
            for button in buttons {
                button.layoutIfNeeded()
                guard let label = button.titleLabel, let icon = button.imageView else { continue }
                let titleFrame = label.convert(label.bounds, to: button)
                let iconFrame = icon.convert(icon.bounds, to: button)
                require(abs(titleFrame.minX - iconFrame.maxX - 8) < 2, "View title separated from its icon: " + (button.accessibilityIdentifier ?? "unknown"))
                require(button.bounds.height >= WorkspaceStyle.touch, "View action lost its touch target")
                let fitted = label.sizeThatFits(CGSize(width: titleFrame.width, height: .greatestFiniteMagnitude))
                require(fitted.height <= titleFrame.height + 1 && fitted.width <= titleFrame.width + 1,
                        "View title clipped: " + (button.accessibilityIdentifier ?? "unknown"))
            }
        }
        func twists(_ index: Int) {
            let cases: [(Bool, Double, String)] = [(false, 0.4, "twist-disabled"), (true, 0.4, "twist-clockwise"), (true, -0.4, "twist-counterclockwise")]
            guard index < cases.count else { finish(); return }
            let (enabled, angle, name) = cases[index]
            commands([("view.top", [:]), ("view.touch_twist", ["enabled": enabled])]) {
                self.session.developmentTwist(angle, enabled: enabled) { result in
                    switch result {
                    case .success(let delta):
                        capture(name, details: ["enabled": enabled, "expected_radians": enabled ? angle : 0, "actual_radians": delta])
                        twists(index + 1)
                    case .failure(let error): finish(error.description)
                    }
                }
            }
        }
        func planes() {
            self.overrideUserInterfaceStyle = .light
            control("legacy-planes")?.sendActions(for: .valueChanged)
            wait({ !self.viewControlPending && self.legacyOriginPlanes }) {
                require(defaults.bool(forKey: keys[0]), "Legacy plane preference did not persist")
                frame("planes-legacy-light") {
                    control("legacy-planes")?.sendActions(for: .valueChanged)
                    wait({ !self.viewControlPending && !self.legacyOriginPlanes }) {
                        require(!defaults.bool(forKey: keys[0]), "Neutral plane preference did not persist")
                        self.viewControlsPopover?.dismiss(animated: false); self.viewControlsPopover = nil
                        self.overrideUserInterfaceStyle = .dark
                        frame("planes-neutral-dark") { twists(0) }
                    }
                }
            }
        }
        report("running")
        DispatchQueue.main.asyncAfter(deadline: .now() + 60) { if !finished { finish("Diagnostic timed out") } }
        wait({ self.state != nil && self.viewPresentation != nil && self.pending.isEmpty }) {
            require(self.session.currentProject == nil, "Diagnostic loaded a library project")
            self.overrideUserInterfaceStyle = .light; self.legacyOriginPlanes = false
            self.browserOpen = false; self.inspectorOpen = false; self.relayoutPanels()
            commands([("view.isometric", [:]), ("view.fit", [:]), ("view.origin_style", ["legacy": false]),
                      ("view.origin", ["group": "planes", "visible": true])]) {
                frame("planes-neutral-light") {
                    self.updateAvailability()
                    self.showViewControls(animated: false)
                    self.viewControlsPopover?.overrideUserInterfaceStyle = .light
                    wait({ self.viewControlsPopover?.view.window != nil && self.viewControlsPopover?.isBeingPresented == false }) {
                        frame("menu-light") {
                            inspectIcons()
                            self.viewControlsPopover?.overrideUserInterfaceStyle = .dark
                            frame("menu-dark") {
                                inspectIcons()
                                let category = self.viewControlsPopover?.traitCollection.preferredContentSizeCategory ?? .large
                                self.viewControlsPopover?.traitOverrides.preferredContentSizeCategory = .accessibilityLarge
                                frame("menu-large-text") {
                                    inspectIcons()
                                    let scroll = self.viewControlsPopover?.view.subviews.compactMap { $0 as? UIScrollView }.first
                                    require((scroll?.contentSize.height ?? 0) > (scroll?.bounds.height ?? 0), "Large View page did not scroll")
                                    if let scroll { scroll.setContentOffset(CGPoint(x: 0, y: max(0, scroll.contentSize.height - scroll.bounds.height)), animated: false) }
                                    frame("menu-large-text-scrolled") {
                                        inspectIcons()
                                        if let button = control("individual-references"), let scroll {
                                            require(scroll.bounds.contains(button.convert(button.bounds, to: scroll)), "Last View option was unreachable")
                                        }
                                        self.viewControlsPopover?.traitOverrides.preferredContentSizeCategory = category
                                        control("individual-references")?.sendActions(for: .primaryActionTriggered)
                                        self.overrideUserInterfaceStyle = .dark
                                        frame("references-dark") { inspectIcons(); planes() }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Disposable native fixture; real View controls and Rust presentation state.
    /// A fresh presented frame precedes every capture. No project is loaded/saved.
    private func checkGridAxesOnDevice() {
        let expected = generation
        let savedOrigin = originVisibility, savedGrid = gridVisible, savedAxes = integratedGridAxes
        let savedAxisLabels = axisLabelsVisible
        let defaults = UserDefaults.standard
        let keys = ["SolidCraft.workspace.originVisibility", "SolidCraft.workspace.gridVisible", "SolidCraft.workspace.integratedGridAxes", "SolidCraft.workspace.axisLabels"]
        let savedDefaults = keys.map { defaults.object(forKey: $0) }
        let started = Date().timeIntervalSince1970
        let runID = UUID().uuidString
        let candidate = Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped"
        var original: WorkspaceState?
        var issues: [String] = []
        var stages: [[String: Any]] = []
        var finished = false
        guard let directory = try? FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true) else {
            NSLog("Grid axes check FAIL: evidence directory unavailable"); return
        }
        func check(_ value: Bool, _ message: String) { if !value { issues.append(message) } }
        func control(_ id: String, in parent: UIView? = nil) -> UIControl? {
            guard let parent = parent ?? viewControlsPopover?.view else { return nil }
            if parent.accessibilityIdentifier == "workspace-view-" + id { return parent as? UIControl }
            for child in parent.subviews { if let found = control(id, in: child) { return found } }
            return nil
        }
        func axes(_ expected: [String]) -> Bool {
            guard let presentation = viewPresentation else { return false }
            return Self.originItems.allSatisfy { presentation.originVisible($0) == expected.contains($0) }
        }
        func save(_ status: String) {
            let report: [String: Any] = ["status": status, "passed": status == "passed", "candidate": candidate,
                "run_id": runID, "started_at": started, "completed_at": status == "running" ? 0 : Date().timeIntervalSince1970,
                "coverage": "Physical native runtime; programmatic View controls and rendered captures; physical touch/Pencil unverified",
                "library_project_untouched": session.currentProject == nil, "issues": issues, "stages": stages]
            do {
                try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
                    .write(to: directory.appendingPathComponent("grid-axes-check.json"), options: .atomic)
            } catch { NSLog("Grid axes evidence write FAIL: %@", error.localizedDescription) }
        }
        func finish(_ error: String? = nil) {
            guard !finished else { return }; finished = true
            if let error { issues.append(error) }
            gridAxesCheckAfterFrame = nil
            originVisibility = savedOrigin; gridVisible = savedGrid; integratedGridAxes = savedAxes
            axisLabelsVisible = savedAxisLabels; showReferenceLabels(latestReferenceLabels)
            for (key, value) in zip(keys, savedDefaults) { defaults.set(value, forKey: key) }
            session.command("view.origin", params: ["visible": NSNull()])
            for (item, visible) in savedOrigin { session.command("view.origin", params: ["item": item, "visible": visible]) }
            session.command("view.grid", params: ["visible": savedGrid, "integrated_axes": savedAxes])
            viewControlsPopover?.dismiss(animated: false); viewControlsPopover = nil
            failure = nil; updateStatus(); rebuildViewMenu()
            save(issues.isEmpty ? "passed" : "failed")
            NSLog("Grid axes check %@", issues.isEmpty ? "PASS" : "FAIL: " + issues.joined(separator: "; "))
        }
        func unchanged() {
            check(state?.document_id == original?.document_id && state?.geometry_revision == original?.geometry_revision &&
                  state?.can_undo == original?.can_undo && state?.selection == original?.selection, "grid axes changed document/history/selection")
        }
        func capture(_ stage: String) {
            view.layoutIfNeeded()
            let surface: UIView = view.window ?? view
            let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
            let filename = "grid-axes-" + stage + ".png"
            do {
                guard let data = image.pngData() else { throw NSError(domain: "GridAxesCheck", code: 1) }
                try data.write(to: directory.appendingPathComponent(filename), options: .atomic)
                stages.append(["stage": stage, "screenshot": filename,
                    "visible": Self.originItems.filter { viewPresentation?.originVisible($0) == true },
                    "integrated_axes": viewPresentation?.grid.integrated_axes == true,
                    "grid_visible": viewPresentation?.grid.visible == true])
                save("running")
            } catch { issues.append("capture " + stage + ": " + String(describing: error)) }
        }
        func wait(_ condition: @escaping () -> Bool, remaining: Int = 100, then next: @escaping () -> Void) {
            guard !finished else { return }
            guard active, generation == expected else { finish("workspace lifecycle changed"); return }
            if condition() { next(); return }
            guard remaining > 0 else { finish("state update timed out"); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { wait(condition, remaining: remaining - 1, then: next) }
        }
        let steps: [(String, () -> Void, () -> Bool, () -> Void)] = [
            ("top", { control("top")?.sendActions(for: .primaryActionTriggered) }, { self.viewPresentation?.camera.standardView == "top" && axes(["X", "Y"]) }, {
                check((control("integrated-axes") as? UISwitch)?.isOn == true, "grid axes switch did not follow renderer")
            }),
            ("explicit-z", {
                control("individual-references")?.sendActions(for: .primaryActionTriggered)
                control("reference-Z")?.sendActions(for: .valueChanged)
            }, { axes(["X", "Y", "Z"]) }, {}),
            ("z-hidden", { control("reference-Z")?.sendActions(for: .valueChanged) }, { axes(["X", "Y"]) }, {}),
            ("automatic", {
                control("automatic")?.sendActions(for: .primaryActionTriggered)
                control("back")?.sendActions(for: .primaryActionTriggered)
            }, { self.originVisibility.isEmpty && axes(["X", "Y"]) }, {}),
            ("grid-hidden", { control("grid")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.grid.visible == false && axes([]) }, {}),
            ("grid-restored", { control("grid")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.grid.visible == true && axes(["X", "Y"]) }, {}),
            ("legacy", { control("integrated-axes")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.grid.integrated_axes == false && axes([]) }, {
                check(defaults.object(forKey: "SolidCraft.workspace.integratedGridAxes") as? Bool == false, "legacy preference did not persist")
            }),
            ("integrated", { control("integrated-axes")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.grid.integrated_axes == true && axes(["X", "Y"]) }, {
                check(defaults.object(forKey: "SolidCraft.workspace.integratedGridAxes") as? Bool == true, "integrated preference did not persist")
                self.viewControlPending = true; self.rebuildViewMenu()
                check(control("integrated-axes")?.isEnabled == false, "busy grid-axis switch remained enabled")
                self.toggleIntegratedGridAxes()
                self.viewControlPending = false; self.rebuildViewMenu()
                check(self.integratedGridAxes, "busy grid-axis switch changed preference")
            }),
            ("front", { control("front")?.sendActions(for: .primaryActionTriggered) }, { self.viewPresentation?.camera.standardView == "front" && axes(["X", "Z"]) }, {}),
            ("right", { control("right")?.sendActions(for: .primaryActionTriggered) }, { self.viewPresentation?.camera.standardView == "right" && axes(["Y", "Z"]) }, {}),
            ("isometric", { control("isometric")?.sendActions(for: .primaryActionTriggered) }, { self.viewPresentation?.camera.standardView == "isometric" && axes(["X", "Y"]) }, {}),
            ("isometric-z", {
                control("individual-references")?.sendActions(for: .primaryActionTriggered)
                control("reference-Z")?.sendActions(for: .valueChanged)
            }, { axes(["X", "Y", "Z"]) }, {
                check(Self.originAxes.allSatisfy { self.referenceLabels[$0] == nil }, "axis labels visible by default")
            }),
            ("axis-labels-shown", { control("axis-labels")?.sendActions(for: .valueChanged) }, { self.axisLabelsVisible }, {
                check(self.referenceLabels["Z"] != nil, "axis labels preference did not restore the visible Z label")
                check(defaults.bool(forKey: "SolidCraft.workspace.axisLabels"), "axis labels preference did not persist")
            }),
            ("axis-labels-hidden", { control("axis-labels")?.sendActions(for: .valueChanged) }, { !self.axisLabelsVisible }, {
                check(Self.originAxes.allSatisfy { self.referenceLabels[$0] == nil }, "axis labels preference left visible labels")
                check(!defaults.bool(forKey: "SolidCraft.workspace.axisLabels"), "hidden axis labels preference did not persist")
            }),
            ("grid-hidden-explicit", {
                control("back")?.sendActions(for: .primaryActionTriggered)
                control("grid")?.sendActions(for: .valueChanged)
            }, { self.viewPresentation?.grid.visible == false && axes(["Z"]) }, {}),
            ("grid-restored-explicit", { control("grid")?.sendActions(for: .valueChanged) }, { self.viewPresentation?.grid.visible == true && axes(["X", "Y", "Z"]) }, {})
        ]
        func step(_ index: Int) {
            guard !finished else { return }
            guard index < steps.count else { unchanged(); finish(); return }
            let (name, action, ready, verify) = steps[index]
            action()
            wait({ !self.viewControlPending && ready() }) {
                verify(); unchanged()
                // A no-op viewport command schedules a fresh frame after its
                // completion; capture only after that frame has been presented.
                self.session.command("view.grid", params: ["visible": self.viewPresentation?.grid.visible ?? true]) { result in
                    if case .failure(let error) = result { finish(error.description); return }
                    self.gridAxesCheckAfterFrame = {
                        DispatchQueue.main.async {
                            guard !finished else { return }
                            capture(name); step(index + 1)
                        }
                    }
                }
            }
        }
        save("running")
        DispatchQueue.main.asyncAfter(deadline: .now() + 45) { if !finished { finish("diagnostic timed out") } }
        wait({ self.viewPresentation != nil && self.state != nil && self.pending.isEmpty }) {
            original = self.state
            self.session.command("view.origin", params: ["visible": NSNull()]) { result in
                if case .failure(let error) = result { finish(error.description); return }
                self.originVisibility = [:]
                self.session.command("view.grid", params: ["visible": true, "integrated_axes": true]) { result in
                    if case .failure(let error) = result { finish(error.description); return }
                    self.gridVisible = true; self.integratedGridAxes = true
                    self.axisLabelsVisible = false; self.showReferenceLabels(self.latestReferenceLabels)
                    self.showViewControls()
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                        guard self.viewControlsPopover != nil else { finish("View panel did not open"); return }
                        step(0)
                    }
                }
            }
        }
    }

    func checkInlineDimensionsOnDevice() {
        print("Inline dimension candidate: " + (Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped"))
        let expected = generation
        freeDimensions = true; repeatRectangles = false; inlineDimensions = true
        let commands: [(String, [String: Any])] = [
            ("file.new", [:]),
            ("sketch.create", ["plane": "XY"]),
            ("tool.free_dimensions", ["enabled": true]),
            ("tool.repeat_rectangles", ["enabled": false]),
            ("tool.set", ["tool": "sketch.rectangle.two_point"])
        ]
        func step(_ index: Int) {
            guard active, generation == expected else { return }
            guard index < commands.count else {
                session.proposeSketchTransform(command: "sketch.rectangle.two_point", params: ["p0": [-30, -20], "p1": [30, 20], "connect": false], commit: true) { result in
                    guard self.active, self.generation == expected else { return }
                    switch result {
                    case .failure(let error): self.showFailure(error)
                    case .success:
                        self.session.command("view.fit") { result in
                            guard self.active, self.generation == expected else { return }
                            switch result {
                            case .failure(let error): self.showFailure(error)
                            case .success:
                                self.browserOpen = false; self.inspectorOpen = false; self.relayoutPanels()
                                print("Inline dimension fixture READY: tap an unselected edge's size helper")
                                if ProcessInfo.processInfo.arguments.contains("--inline-dimension-check-run") { self.verifyInlineDimensionsOnDevice() }
                            }
                        }
                    }
                }
                return
            }
            let (command, params) = commands[index]
            session.command(command, params: params) { result in
                switch result {
                case .failure(let error): self.showFailure(error)
                case .success: step(index + 1)
                }
            }
        }
        step(0)
    }

    /// Exercises the actual UIKit editor and native transactions; contact grammar
    /// is checked separately by the deterministic tracker/viewport tests.
    private func verifyInlineDimensionsOnDevice() {
        let expected = generation
        func wait(_ stage: String, remaining: Int = 100, until condition: @escaping () -> Bool, then action: @escaping () -> Void) {
            guard active, generation == expected else { return }
            if condition() { action(); return }
            guard remaining > 0 else { print("Inline dimension check FAIL: " + stage); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
                wait(stage, remaining: remaining - 1, until: condition, then: action)
            }
        }
        func capture(_ stage: String) {
            view.layoutIfNeeded()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let surface: UIView = view.window ?? view
                let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
                guard let data = image.pngData() else { print("Inline dimension check FAIL: image encoding"); return }
                try data.write(to: directory.appendingPathComponent("inline-dimension-" + stage + ".png"), options: .atomic)
            } catch { print("Inline dimension check FAIL: \(error)") }
        }
        var helperEntity: SketchAnnotationPresentation?
        func selectEdge(_ next: @escaping () -> Void) {
            guard let entity = helperEntity else { print("Inline dimension check FAIL: missing edge"); return }
            session.selectEntity(entity.entity.dictionary, additive: false) { result in
                switch result {
                case .failure(let error): print("Inline dimension check FAIL: \(error)")
                case .success:
                    wait("edge annotation", until: { self.sketchAnnotations.values.contains { $0.selected && !$0.driven } }, then: next)
                }
            }
        }
        func openEditor(selectedOnly: Bool = true, _ next: @escaping () -> Void) {
            func eligible(_ annotation: SketchAnnotationPresentation) -> Bool {
                self.annotationEditable(annotation) && (selectedOnly ? annotation.selected : annotation.id == helperEntity?.id)
            }
            wait("available dimension value", until: { self.sketchAnnotations.values.contains(where: eligible) }) {
                guard let annotation = self.sketchAnnotations.values.first(where: eligible) else { return }
                self.canvas.onDimensionTap?(annotation.id)
                wait("focused popover", until: { self.inlineNumericEditor != nil && self.editor?.field.isFirstResponder == true }, then: next)
            }
        }
        func hasDimension(_ expression: String) -> Bool {
            state?.sketch?.constraints.filter(\.dimension).count == 1 &&
                state?.sketch?.constraints.contains { $0.dimension && $0.expression == expression } == true
        }
        func checkUndoRedo() {
            run("edit.undo")
            wait("undo", until: { self.state?.sketch?.constraints.contains(where: \.dimension) == false }) {
                self.run("edit.redo")
                wait("redo", until: { hasDimension("80 mm") }) {
                    selectEdge {
                        openEditor {
                            capture("editor")
                            print("Inline dimension check PASS: rectangle returns to Select; 2 exterior top/right helpers without automatic editor; direct helper editing; opposite size helper suppressed after dimension; focused UIKit popover, native Apply, repeat edit, Preview/Cancel, Undo/Redo; physical contacts unverified")
                        }
                    }
                }
            }
        }
        wait("rectangle fixture and free helpers", until: { self.state?.sketch?.curves.count == 4 && self.state?.active_tool == "select" && self.sketchAnnotations.values.filter { $0.dimension_type != nil }.count == 2 }) {
            guard self.state?.active_tool == "select", self.state?.selection.isEmpty == true,
                  self.sketchAnnotations.values.allSatisfy({ !$0.selected }), !self.numericEditing,
                  self.inlineNumericEditor == nil else {
                print("Inline dimension check FAIL: rectangle did not return to unselected helpers without an automatic editor"); return
            }
            helperEntity = self.sketchAnnotations.values.filter { $0.dimension_type != nil }.min { $0.points[1] < $1.points[1] }
            capture("helpers")
            openEditor(selectedOnly: false) {
                guard let editor = self.editor, let range = editor.field.selectedTextRange,
                      editor.field.offset(from: range.start, to: range.end) == (editor.field.text ?? "").utf16.count else {
                    print("Inline dimension check FAIL: expression was not selected"); return
                }
                capture("initial-editor")
                editor.field.insertText("80 mm"); editor.onApply?(editor.field.text ?? "")
                wait("apply and opposite helper suppression", until: { !self.numericEditing && hasDimension("80 mm") && self.sketchAnnotations.values.contains { abs($0.value - 80) < 0.001 } && self.sketchAnnotations.values.filter { $0.dimension_type != nil }.count == 1 }) {
                    capture("applied")
                    selectEdge {
                        openEditor {
                            guard let editor = self.editor else { return }
                            editor.field.insertText("100 mm"); editor.onPreview?(editor.field.text ?? "")
                            wait("preview", until: { !editor.busy && self.sketchAnnotations.values.contains { abs($0.value - 100) < 0.001 } }) {
                                editor.onCancel?()
                                wait("cancel", until: { !self.numericEditing && hasDimension("80 mm") }) { checkUndoRedo() }
                            }
                        }
                    }
                }
            }
        }
    }

    func checkBrowserOnDevice() {
        leftBrowser = true; appearance = 2
        originExpanded = false
        configureChrome(); applyAppearance()
        func capture(_ name: String) {
            view.layoutIfNeeded()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let image = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in view.drawHierarchy(in: view.bounds, afterScreenUpdates: true) }
                guard let data = image.pngData() else { print("Browser check FAIL: image encoding"); return }
                try data.write(to: directory.appendingPathComponent("browser-check-" + name + ".png"), options: .atomic)
                print("Browser check CAPTURE \(name)")
            } catch { print("Browser check FAIL: \(error)") }
        }
        func eye(_ row: WorkspaceRow) -> WorkspaceButton? {
            guard let index = visibleRows.firstIndex(where: { $0.id == row.id }),
                  let cell = table.cellForRow(at: IndexPath(row: index, section: 1)),
                  let controls = cell.accessoryView as? UIStackView else { return nil }
            return controls.arrangedSubviews.compactMap { $0 as? WorkspaceButton }.first
        }
        browserCheckWorkspace = { [weak self] value in
            guard let self, value.rows.contains(where: { $0.label == "Frame" }) else { return }
            self.browserCheckWorkspace = nil
            self.browserButton.sendActions(for: .touchUpInside)
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in
                guard let self, self.active, let frame = self.layout?.browser,
                      let row = self.visibleRows.first(where: { $0.id == "body:Post" }), let button = eye(row) else {
                    print("Browser check FAIL: browser or eye unavailable"); return
                }
                var issues: [String] = []
                for cell in self.table.visibleCells where cell.accessibilityIdentifier?.hasPrefix("workspace-row-") == true {
                    guard let content = cell.contentConfiguration as? UIListContentConfiguration else { issues.append("missing content"); continue }
                    if content.image == nil || content.secondaryText != nil { issues.append("missing icon or extra subtitle") }
                }
                for control in [self.top, self.railScroll, self.browserButton, self.historyActions, self.featureHistory] where control.frame.intersects(frame) {
                    issues.append("overlapping chrome")
                }
                let menu = self.browserMenu(row)
                if !menu.children.contains(where: { $0.title == BrowserText.text("rename") }) { issues.append("missing long-press Rename") }
                if !menu.children.contains(where: { $0.title == BrowserText.text("move_to_folder") }) { issues.append("missing folder move") }
                print("Browser check ROWS \(issues.isEmpty ? "PASS" : "FAIL: " + issues.joined(separator: "; "))")
                capture("dark")
                self.browserCheckWorkspace = { [weak self] value in
                    guard let self, let hidden = value.rows.first(where: { $0.id == row.id }), hidden.browser?.visible == false else { return }
                    self.browserCheckWorkspace = nil
                    print("Browser check HIDE PASS")
                    capture("hidden")
                    self.browserCheckWorkspace = { [weak self] value in
                        guard let self, value.rows.first(where: { $0.id == row.id })?.browser?.visible == true else { return }
                        self.browserCheckWorkspace = nil
                        self.appearance = 1; self.applyAppearance(); self.table.reloadData()
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                            capture("light")
                            self.appearance = 2; self.applyAppearance(); self.table.reloadData()
                            print("Browser check SHOW PASS")
                            self.checkBrowserSearchOnDevice(capture: capture)
                        }
                    }
                    eye(hidden)?.sendActions(for: .touchUpInside)
                }
                button.sendActions(for: .touchUpInside)
            }
        }
        session.command("browser.group", params: ["folder": "bodies", "name": "Frame", "items": ["Base"]])
    }

    /// Extends the existing disposable browser driver through production search,
    /// table selection and eye controls. Never loads or saves a library design.
    private func checkBrowserSearchOnDevice(capture: @escaping (String) -> Void) {
        var issues: [String] = []
        func check(_ condition: Bool, _ message: String) { if !condition { issues.append(message) } }
        func query(_ text: String) {
            browserSearchField.text = text; browserSearchField.sendActions(for: .editingChanged)
            view.layoutIfNeeded(); table.layoutIfNeeded()
        }
        func columns() {
            var eyeX: CGFloat?
            for cell in table.visibleCells {
                guard let controls = cell.accessoryView as? UIStackView else { continue }
                controls.layoutIfNeeded()
                check(controls.bounds.width == BrowserRowColumns.width, "missing reserved columns")
                check(controls.arrangedSubviews.count == 2, "missing trailing slot")
                guard let eye = controls.arrangedSubviews.first as? WorkspaceButton else { continue }
                let frame = eye.convert(eye.bounds, to: table)
                if let x = eyeX { check(abs(frame.minX - x) < 1, "eye column drifts") } else { eyeX = frame.minX }
                check(frame.width >= 44 && frame.height >= 44, "small eye target")
                if let disclosure = controls.arrangedSubviews.last {
                    check(!frame.intersects(disclosure.convert(disclosure.bounds, to: table)), "overlapping targets")
                }
            }
        }
        guard let original = state, let folder = original.rows.first(where: { $0.label == "Frame" }) else {
            print("Browser search check FAIL: missing fixture"); return
        }
        let oldCategory = traitCollection.preferredContentSizeCategory
        var savedCollapsed: Set<String> = []
        var savedOffset: CGFloat = 0
        func afterLayout(_ action: @escaping () -> Void) {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.45) { [weak self = self] in
                guard let self, self.active else { return }
                self.view.layoutIfNeeded(); self.table.layoutIfNeeded(); action()
            }
        }
        func finishSelection(_ value: WorkspaceState) {
            let selection = value.selection
            cancelKey()
            check(!browserSearchOpen && !textEntryActive, "Escape did not dismiss search")
            check(collapsed == savedCollapsed && state?.selection == selection, "dismiss changed explicit selection or expansion")
            check(currentBrowserRow("body:Post", document: "stale", revision: value.geometry_revision) == nil, "stale document accepted")
            check(currentBrowserRow("body:Post", document: value.document_id, revision: value.geometry_revision &+ 1) == nil, "stale revision accepted")
            pending = "test"
            check(currentBrowserRow("body:Post", document: value.document_id, revision: value.geometry_revision) == nil, "busy browser accepted action")
            pending = ""
            traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
            afterLayout {
                let originalFrame = self.table.frame
                for width: CGFloat in [260, 320] {
                    self.table.frame.size.width = width; self.table.reloadData(); self.table.layoutIfNeeded(); columns()
                }
                self.table.frame = originalFrame; self.table.reloadData()
                afterLayout {
                    columns(); capture("large-text")
                    self.traitOverrides.preferredContentSizeCategory = oldCategory
                    self.rebuildMenus(); self.view.setNeedsLayout(); self.table.reloadData()
                    afterLayout {
                        self.appearance = 1; self.applyAppearance(); self.table.reloadData()
                        self.browserFindButton.sendActions(for: .touchUpInside); query("post")
                        self.browserSearchField.resignFirstResponder()
                        afterLayout {
                            capture("search-light")
                            print("Browser search check \(issues.isEmpty ? "PASS" : "FAIL: " + issues.joined(separator: "; ")); complete")
                        }
                    }
                }
            }
        }
        // Include references and allow real UIKit reuse/trait/layout passes to
        // settle before capturing; rapid same-turn snapshots include old cells.
        originInBrowser = true; originExpanded = true; updateRows(); view.setNeedsLayout()
        afterLayout {
            columns(); capture("aligned")
            self.collapsed.insert(folder.id); self.updateRows()
            savedCollapsed = self.collapsed; savedOffset = self.table.contentOffset.y
            self.browserFindButton.sendActions(for: .touchUpInside)
            check(self.browserSearchField.isFirstResponder && self.textEntryActive, "search has no text ownership")
            check(!WorkspaceFocusPolicy.documentShortcutsEnabled(textEntryActive: self.textEntryActive, numericEditActive: false), "search permits document shortcuts")
            query("BASE")
            check(self.visibleRows.contains(where: { $0.id == "body:Base" }) && self.visibleRows.contains(where: { $0.id == folder.id }), "collapsed ancestor not revealed")
            check(!self.visibleRows.contains(where: { $0.id == "body:Post" }), "unrelated result remains")
            check(self.collapsed == savedCollapsed && self.state?.selection == original.selection, "typing changed selection or expansion")
            check(self.state?.geometry_revision == original.geometry_revision && self.state?.can_undo == original.can_undo, "search changed document")
            self.browserSearchField.resignFirstResponder()
            afterLayout {
                columns(); capture("search"); query("not-present-in-fixture")
                check(self.visibleRows.isEmpty && self.visibleOriginRows.isEmpty && self.browserMessage.text == BrowserText.text("no_matches"), "no-results state missing")
                afterLayout {
                    capture("no-results"); query("")
                    check(self.collapsed == savedCollapsed && !self.visibleRows.contains(where: { $0.id == "body:Base" }), "clear failed to restore collapse")
                    check(abs(self.table.contentOffset.y - savedOffset) < 1, "clear failed to restore scroll")
                    query("post")
                    check(self.visibleRows.filter { self.browserName($0).localizedCaseInsensitiveContains("post") }.count == 2, "match count loses feature/body identities")
                    afterLayout {
                        columns(); capture("search-post")
                        guard let index = self.visibleRows.firstIndex(where: { $0.id == "body:Post" }) else { print("Browser search check FAIL: missing result"); return }
                        func selectPost() {
                            guard let row = self.visibleRows.firstIndex(where: { $0.id == "body:Post" }) else { return }
                            self.browserCheckWorkspace = { [weak self = self] value in
                                guard let self, value.selection.contains(where: { $0.type == "body" && $0.name == "Post" }) else { return }
                                self.browserCheckWorkspace = nil; afterLayout { finishSelection(value) }
                            }
                            self.tableView(self.table, didSelectRowAt: IndexPath(row: row, section: 1))
                        }
                        func tapEye() {
                            guard let row = self.visibleRows.firstIndex(where: { $0.id == "body:Post" }),
                                  let controls = self.table.cellForRow(at: IndexPath(row: row, section: 1))?.accessoryView as? UIStackView,
                                  let button = controls.arrangedSubviews.first as? WorkspaceButton else {
                                issues.append("filtered eye missing"); return
                            }
                            button.sendActions(for: .touchUpInside)
                        }
                        self.browserCheckWorkspace = { [weak self = self] hidden in
                            guard let self, hidden.rows.first(where: { $0.id == "body:Post" })?.browser?.visible == false else { return }
                            self.browserCheckWorkspace = nil
                            check(hidden.rows.first(where: { $0.id == "body:Base" })?.browser?.visible == true, "filtered eye changed another body")
                            check(self.visibleRows.contains(where: { $0.id == "body:Post" }), "visibility removed search result")
                            self.browserCheckWorkspace = { [weak self = self] shown in
                                guard let self, let row = shown.rows.first(where: { $0.id == "body:Post" }), row.browser?.visible == true,
                                      let rename = row.browser?.rename else { return }
                                self.browserCheckWorkspace = nil
                                self.browserCheckWorkspace = { [weak self = self] renamed in
                                    guard let self, renamed.rows.contains(where: { $0.kind == "body" && $0.label == "Rail" }) else { return }
                                    self.browserCheckWorkspace = nil
                                    check(!self.visibleRows.contains(where: { $0.id == row.id }), "rename left stale search result")
                                    self.browserCheckWorkspace = { [weak self = self] restored in
                                        guard let self, restored.rows.first(where: { $0.id == row.id })?.label == "Post" else { return }
                                        self.browserCheckWorkspace = nil
                                        check(self.visibleRows.contains(where: { $0.id == row.id }), "undo failed to restore search identity")
                                        afterLayout { selectPost() }
                                    }
                                    self.session.command("edit.undo")
                                }
                                var params = rename.params.mapValues(\.value); params["name"] = "Rail"
                                self.session.command(rename.command, params: params)
                            }
                            afterLayout { tapEye() }
                        }
                        // The filtered index differs from the original tree row.
                        check(index != original.rows.firstIndex(where: { $0.id == "body:Post" }), "fixture did not exercise filtered indexing")
                        tapEye()
                    }
                }
            }
        }
    }

    /// Opt-in disposable fixture: actual UIKit buttons and engine selection,
    /// with native chrome captures. Adapter/GPU tests check model highlights.
    func checkHistoryOnDevice() {
        let expected = generation
        var finished = false
        func button(_ id: String, in parent: UIView) -> UIControl? {
            if parent.accessibilityIdentifier == id { return parent as? UIControl }
            return parent.subviews.compactMap { button(id, in: $0) }.first
        }
        func capture(_ stage: String) {
            view.layoutIfNeeded()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let image = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in view.drawHierarchy(in: view.bounds, afterScreenUpdates: true) }
                guard let data = image.pngData() else { print("History check FAIL: image encoding"); return }
                try data.write(to: directory.appendingPathComponent("history-check-" + stage + ".png"), options: .atomic)
                print("History check CAPTURE \(stage)")
            } catch { print("History check FAIL: \(error)") }
        }
        func scrubHistory(_ index: Int = 0) {
            let paths = [[2, 1], [2, 0, 1], [0, 2, 3], [2, 0], [1, 3]]
            guard index < paths.count else {
                finished = true
                print("History check PASS: icon-only, toggle, selection, arrows, scrub routing, reverse drag, empty stage and final lift")
                return
            }
            let path = paths[index]
            guard let position = path.last else { return }
            historyCheckWorkspace = { [weak self] value in
                guard let self, self.active, self.generation == expected,
                      value.modelling?.activeFeatureCount == position, !self.featureHistory.isScrubbing else { return }
                self.historyCheckWorkspace = nil
                let bodies = position == 0 ? 0 : (position == 1 ? 1 : 2)
                guard value.rows.filter({ $0.kind == "body" }).count == bodies else {
                    finished = true; print("History check FAIL: scrub geometry at stage \(position)"); return
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    capture("scrub-" + String(index) + "-stage-" + String(position))
                    scrubHistory(index + 1)
                }
            }
            guard featureHistory.developmentScrub(path) else {
                finished = true; historyCheckWorkspace = nil; print("History check FAIL: scrub route unavailable"); return
            }
        }
        func navigate(_ index: Int = 0) {
            let steps = [("back", 2, 2), ("back", 1, 1), ("forward", 2, 2), ("forward", 3, 2)]
            guard index < steps.count else {
                scrubHistory()
                return
            }
            let (direction, position, bodies) = steps[index]
            guard let control = button("workspace-history-" + direction, in: featureHistory), control.isEnabled else {
                finished = true; print("History check FAIL: unavailable transport \(direction)"); return
            }
            historyCheckWorkspace = { [weak self] value in
                guard let self, self.active, self.generation == expected, value.modelling?.activeFeatureCount == position else { return }
                self.historyCheckWorkspace = nil
                guard value.rows.filter({ $0.kind == "body" }).count == bodies else {
                    finished = true; print("History check FAIL: incorrect geometry at stage \(position)"); return
                }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                    capture("stage-" + String(position)); navigate(index + 1)
                }
            }
            control.sendActions(for: .touchUpInside)
        }
        func start(_ value: WorkspaceState) {
            guard let feature = value.modelling?.timeline.first,
                  let toggle = button("workspace-feature-history-toggle", in: featureHistory) else {
                print("History check FAIL: history controls unavailable"); return
            }
            guard !historyExpanded && !featureHistory.expanded else { print("History check FAIL: must start collapsed"); return }
            capture("collapsed")
            toggle.sendActions(for: .touchUpInside); view.layoutIfNeeded()
            guard historyExpanded, featureHistory.expanded,
                  let item = button("workspace-history-feature-" + String(feature.id), in: featureHistory) else {
                print("History check FAIL: feature strip did not expand"); return
            }
            guard (item as? UIButton)?.configuration?.title == nil else { print("History check FAIL: visible feature name"); return }
            guard toggle.frame.maxX >= featureHistory.bounds.width - 8 else { print("History check FAIL: toggle left the right edge"); return }
            assemblyCheckBegan = { [weak self] result in
                guard let self, self.active, self.generation == expected, case .success = result,
                      self.modellingFeature == feature.id else { print("History check FAIL: feature editor did not open"); return }
                capture("selected")
                self.assemblyCheckCancelled = { [weak self] _ in
                    guard let self, self.active, self.generation == expected else { return }
                    toggle.sendActions(for: .touchUpInside)
                    guard !self.historyExpanded else { print("History check FAIL: did not collapse"); return }
                    toggle.sendActions(for: .touchUpInside)
                    self.browserButton.sendActions(for: .touchUpInside)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { [weak self] in
                        guard let self, self.active, self.generation == expected else { return }
                        capture("browser")
                        navigate()
                    }
                }
                self.closePanels()
            }
            item.sendActions(for: .primaryActionTriggered)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 20) { [weak self] in
            guard !finished else { return }
            self?.historyCheckWorkspace = nil
            print("History check FAIL: timed out")
        }
        if let state { start(state) }
        else {
            historyCheckWorkspace = { [weak self] value in
                self?.historyCheckWorkspace = nil; start(value)
            }
            session.refreshWorkspace()
        }
    }

    /// Disposable native workflow: real history buttons, UIMenu actions and the
    /// same revision-checked confirmation closure used by the destructive alert.
    func checkHistoryActionsOnDevice() {
        let expected = generation
        let deadline = ProcessInfo.processInfo.systemUptime + 30
        var finished = false
        func fail(_ message: String) { finished = true; historyDeleteConfirmation = nil; print("History actions FAIL: \(message)") }
        func wait(_ name: String, until ready: @escaping () -> Bool, then next: @escaping () -> Void) {
            guard !finished, active, generation == expected else { return }
            if ready() { next(); return }
            guard ProcessInfo.processInfo.systemUptime < deadline else { fail("timeout: " + name); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) { wait(name, until: ready, then: next) }
        }
        func item(_ id: UInt64) -> UIButton? {
            func find(_ view: UIView) -> UIButton? {
                if view.accessibilityIdentifier == "workspace-history-feature-" + String(id) { return view as? UIButton }
                return view.subviews.compactMap(find).first
            }
            return find(featureHistory)
        }
        func capture(_ stage: String) {
            view.layoutIfNeeded()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let target: UIView = view.window ?? view
                let image = UIGraphicsImageRenderer(bounds: target.bounds).image { _ in target.drawHierarchy(in: target.bounds, afterScreenUpdates: true) }
                guard let data = image.pngData() else { fail("image encoding"); return }
                try data.write(to: directory.appendingPathComponent("history-actions-" + stage + ".png"), options: .atomic)
            } catch { fail(String(describing: error)) }
        }
        wait("fixture", until: { self.state?.modelling?.timeline.count == 3 }) {
            guard let features = self.state?.modelling?.timeline, features.count == 3 else { fail("fixture timeline"); return }
            let base = features[0].id, post = features[1].id, sketch = features[2].id
            func editSketch() {
                item(sketch)?.sendActions(for: .primaryActionTriggered)
                wait("sketch editor", until: { self.state?.sketch?.id == sketch }) {
                    capture("edit-sketch")
                    self.run("sketch.finish")
                    wait("finish sketch", until: { self.state?.sketch == nil && self.pending.isEmpty }) {
                        finished = true
                        print("History actions PASS: tap edits original feature, Apply, future-feature edit, anchored Delete sheet retains icon without editor/keyboard, confirmation, undo and sketch editing")
                    }
                }
            }
            func deletePost() {
                guard let control = item(post), self.featureHistory.showActions(for: post, source: control) else { fail("missing held-feature actions"); return }
                wait("delete menu", until: {
                    guard let sheet = self.presentedViewController as? UIAlertController else { return false }
                    return sheet.preferredStyle == .actionSheet && !sheet.isBeingPresented && sheet.viewIfLoaded?.window != nil
                }) {
                    guard let sheet = self.presentedViewController as? UIAlertController, let delete = self.historyMenuDelete,
                          sheet.actions.contains(where: { $0.style == .destructive && $0.title == ModellingText.text("delete_feature") }),
                          !self.modellingEditing, self.historyActivation == nil, self.keyboardTop == nil,
                          item(post) === control, control.configuration?.image != nil else { fail("Delete sheet, icon or focus"); return }
                    capture("delete-menu")
                    self.historyMenuDelete = nil
                    sheet.dismiss(animated: false) { delete() }
                    confirmDelete()
                }
            }
            func confirmDelete() {
                wait("delete confirmation", until: {
                    guard let alert = self.presentedViewController as? UIAlertController else { return false }
                    return self.historyDeleteConfirmation != nil && !alert.isBeingPresented && alert.viewIfLoaded?.window != nil
                }) {
                    guard let alert = self.presentedViewController as? UIAlertController,
                          alert.actions.contains(where: { $0.style == .destructive }), let confirm = self.historyDeleteConfirmation else { fail("confirmation action"); return }
                    capture("delete-confirmation")
                    self.historyDeleteConfirmation = nil
                    alert.dismiss(animated: false) { confirm() }
                    wait("delete feature", until: { self.state?.modelling?.timeline.contains(where: { $0.id == post }) == false && self.pending.isEmpty }) {
                        guard self.state?.rows.filter({ $0.kind == "body" }).count == 1 else { fail("deleted body still visible"); return }
                        capture("deleted")
                        self.run("edit.undo")
                        wait("undo delete", until: { self.state?.modelling?.timeline.contains(where: { $0.id == post }) == true && self.pending.isEmpty }) {
                            guard self.state?.rows.filter({ $0.kind == "body" }).count == 2 else { fail("undo did not restore geometry"); return }
                            editSketch()
                        }
                    }
                }
            }
            func editFuture() {
                self.moveHistory(to: 1)
                wait("rollback", until: { self.state?.modelling?.activeFeatureCount == 1 && self.pending.isEmpty }) {
                    item(post)?.sendActions(for: .primaryActionTriggered)
                    wait("future feature editor", until: { self.modellingFeature == post && self.modellingInspector != nil }) {
                        guard self.state?.modelling?.activeFeatureCount == 2 else { fail("future feature was not rolled forward"); return }
                        capture("edit-future")
                        self.closePanels()
                        wait("cancel future edit", until: { !self.modellingEditing && self.pending.isEmpty }) { deletePost() }
                    }
                }
            }
            self.historyExpanded = true; self.featureHistory.setExpanded(true); self.relayoutPanels()
            guard let control = item(base), control.configuration?.image != nil,
                  !control.showsLargeContentViewer,
                  !control.interactions.contains(where: { $0 is UILargeContentViewerInteraction }),
                  control.menu == nil, !control.isContextMenuInteractionEnabled,
                  control.gestureRecognizers?.contains(where: { $0 is UILongPressGestureRecognizer && $0.cancelsTouchesInView }) == true else {
                fail("history icon/menu gesture configuration"); return
            }
            control.sendActions(for: .primaryActionTriggered)
            wait("feature editor", until: { self.modellingFeature == base && self.modellingInspector != nil }) {
                guard let form = self.modellingInspector, form.params["height"] != nil, let revision = self.state?.geometry_revision else { fail("original box inputs"); return }
                capture("edit-box")
                var params = form.params; params["height"] = .string("14 mm")
                form.onApply?(params)
                wait("apply feature", until: { self.state?.geometry_revision != revision && !self.modellingEditing && self.pending.isEmpty }) {
                    guard self.state?.modelling?.timeline.map(\.id) == features.map(\.id) else { fail("editing replaced feature IDs"); return }
                    capture("edited")
                    editFuture()
                }
            }
        }
    }

    /// Disposable engine fixture plus actual UIKit input/preview callbacks.
    /// Layout variants below are synthetic UIKit sizes, not hardware gestures.
    func checkNumericEntryOnDevice() {
        let savedAppearance = appearance, savedCompact = compactInspector
        compactInspector = true
        var issues: [String] = []
        var finished = false
        func find(_ id: String, in root: UIView) -> UIView? {
            if root.accessibilityIdentifier == id { return root }
            return root.subviews.lazy.compactMap { find(id, in: $0) }.first
        }
        func check(_ condition: Bool, _ message: String) { if !condition { issues.append(message) } }
        func finish() {
            guard !finished else { return }; finished = true
            appearance = savedAppearance; compactInspector = savedCompact; applyAppearance()
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let report: [String: Any] = ["issues": issues, "passed": issues.isEmpty,
                    "coverage": "Real UIKit/engine; synthetic text, layout and Return; no physical Pencil/VoiceOver claim"]
                try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
                    .write(to: directory.appendingPathComponent("numeric-entry-check.json"), options: .atomic)
            } catch { issues.append("report: \(error)") }
            print("Numeric entry check \(issues.isEmpty ? "PASS" : "FAIL"): \(issues)")
        }
        func wait(_ name: String, _ ready: @escaping () -> Bool, attempts: Int = 100, then: @escaping () -> Void) {
            guard !finished else { return }
            if ready() { then(); return }
            guard attempts > 0 else { issues.append("timeout: " + name); self.cancelNumeric(); finish(); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { wait(name, ready, attempts: attempts - 1, then: then) }
        }
        func capture(_ name: String) {
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let surface: UIView = view.window ?? view
                let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
                try image.pngData()?.write(to: directory.appendingPathComponent("numeric-entry-" + name + ".png"), options: .atomic)
            } catch { issues.append("capture: \(error)") }
        }
        func layoutVariants() {
            for size in [CGSize(width: 320, height: 300), CGSize(width: 380, height: 500), CGSize(width: 744, height: 1000)] {
                for category in [UIContentSizeCategory.large, .accessibilityExtraExtraExtraLarge] {
                    let controller = NumericValueEditorController(expression: "-2 mm + 10 mm", units: "mm", name: WorkspaceStyle.text("radius"))
                    controller.traitOverrides.preferredContentSizeCategory = category
                    controller.loadViewIfNeeded(); controller.view.frame = CGRect(origin: .zero, size: size)
                    controller.view.setNeedsLayout(); controller.view.layoutIfNeeded()
                    controller.editor.actionFooter.layoutIfNeeded()
                    for button in [controller.editor.actionFooter.preview, controller.editor.actionFooter.apply, controller.editor.actionFooter.cancel] {
                        let frame = button.convert(button.bounds, to: controller.view)
                        check(frame.width >= 44 && frame.height >= 44 && controller.view.bounds.contains(frame), "unreachable action at \(size)/\(category.rawValue): \(frame)")
                    }
                    check(controller.editor.field.bounds.height >= 44, "compressed numeric field")
                }
            }
        }
        func inspectorEntry() {
            let revision = self.state?.geometry_revision
            self.beginModelling(operation: "solid.extrude", initialParams: [
                "profile": .object(["faces": .array([.array([.number(10), .number(4), .number(35)])]), "face_body": .string("Post")]),
                "distance": .string("10 mm")
            ])
            wait("modelling form", { self.modellingInspector != nil }) {
                guard let form = self.modellingInspector,
                      let descriptor = form.fields.first(where: { $0.kind == "expression" && InspectorPresentation.isDistance($0.key) }),
                      let field = find("workspace-model-field-" + descriptor.key, in: form) as? UITextField else {
                    issues.append("missing inspector expression field"); finish(); return
                }
                let raw = field.text
                _ = field.becomeFirstResponder()
                wait("inspector numeric popup", { self.modelValueEditor?.editor.panelKeypadVisible == true && self.modelValueEditor?.editor.field.isFirstResponder == true }) {
                    guard let popup = self.modelValueEditor else { issues.append("missing inspector popup"); finish(); return }
                    check(popup.parameter == descriptor.key && popup.editor.field.text == raw, "inspector opened the wrong raw expression")
                    capture("inspector-entry")
                    popup.editor.actionFooter.preview.sendActions(for: .touchUpInside)
                    wait("inspector preview", { !popup.editor.busy }) {
                        check(popup.editor.draft.isValidated && popup.editor.actionFooter.apply.isEnabled, "inspector raw expression failed native Preview")
                        capture("inspector-preview")
                        popup.editor.actionFooter.cancel.sendActions(for: .touchUpInside)
                        wait("inspector cancel", { !self.modellingEditing && self.presentedViewController == nil }) {
                            check(self.state?.geometry_revision == revision, "cancelling inspector entry changed geometry")
                            layoutVariants(); finish()
                        }
                    }
                }
            }
        }
        func runStyle(_ style: Int) {
            guard style < 3 else { inspectorEntry(); return }
            appearance = style; applyAppearance()
            let originalCanvas = self.canvas.frame
            beginNumeric(source: browserButton)
            wait("editor and embedded number pad", {
                self.inlineNumericEditor?.editor.field.isFirstResponder == true &&
                self.inlineNumericEditor?.editor.panelKeypadVisible == true &&
                self.inlineNumericEditor?.viewIfLoaded?.window != nil
            }) {
                guard let controller = self.inlineNumericEditor else { issues.append("missing editor"); finish(); return }
                let editor = controller.editor, field = editor.field
                let token = self.numericToken
                let revision = self.state?.geometry_revision
                let expectedStyle = style == 1 ? UIUserInterfaceStyle.light : (style == 2 ? .dark : self.traitCollection.userInterfaceStyle)
                check(controller.traitCollection.userInterfaceStyle == expectedStyle, "popover appearance \(style)")
                check(find("workspace-embedded-keypad", in: controller.view)?.traitCollection.userInterfaceStyle == expectedStyle, "embedded pad appearance \(style)")
                check((field.inputView?.bounds.height ?? 0) == 0 && field.inputAccessoryView == nil, "numeric entry opened a separate keyboard")
                for key in ["7", "8", "9", "⌫", "4", "5", "6", "−", "1", "2", "3", "+", "±", "0", "."] {
                    guard let button = find("workspace-keypad-" + key, in: controller.view) else { issues.append("missing embedded key " + key); continue }
                    check(button.bounds.width >= 44 && button.bounds.height >= 44, "compressed embedded key " + key)
                }
                check(editor.actionFooter.apply.isEnabled == !editor.requiresPreview, "dimension Apply preview preference was ignored")
                check(self.canvas.frame == originalCanvas, "embedded pad resized the Metal viewport")
                func enter(_ text: String) { field.text = text; field.sendActions(for: .editingChanged) }
                func selectedOffsets() -> [Int] {
                    guard let range = field.selectedTextRange else { return [] }
                    return [field.offset(from: field.beginningOfDocument, to: range.start), field.offset(from: field.beginningOfDocument, to: range.end)]
                }
                enter("6 + 6")
                if let sign = find("workspace-keypad-±", in: controller.view) as? UIControl {
                    sign.sendActions(for: .touchUpInside)
                    check(field.text == "-(6 + 6)", "sign key did not negate the whole draft")
                    sign.sendActions(for: .touchUpInside)
                    check(field.text == "6 + 6", "sign key did not restore the raw draft")
                }
                editor.actionFooter.preview.sendActions(for: .touchUpInside)
                enter("2 +")
                if let start = field.position(from: field.beginningOfDocument, offset: 1),
                   let end = field.position(from: field.beginningOfDocument, offset: 3) { field.selectedTextRange = field.textRange(from: start, to: end) }
                let selection = selectedOffsets()
                wait("stale preview", { !editor.busy }) {
                    check(field.text == "2 +" && field.isFirstResponder && selectedOffsets() == selection, "preview lost draft/caret/focus")
                    check(!editor.draft.isValidated && editor.actionFooter.apply.isEnabled == !editor.requiresPreview, "stale preview changed Apply eligibility")
                    capture("stale-\(style)")
                    editor.actionFooter.preview.sendActions(for: .touchUpInside)
                    wait("invalid expression", { !editor.busy }) {
                        check(field.text == "2 +" && !editor.draft.isValidated && !editor.message.isHidden, "invalid expression lost text or feedback")
                        capture("invalid-\(style)")
                        enter("-2 mm + 10 mm")
                        editor.actionFooter.preview.sendActions(for: .touchUpInside)
                        wait("units and negative terms", { !editor.busy }) {
                            check(editor.draft.isValidated && editor.actionFooter.apply.isEnabled, "valid units/negative terms did not validate")
                            check(field.text == "-2 mm + 10 mm" && field.isFirstResponder, "valid preview changed expression/focus")
                            let frame = editor.actionFooter.convert(editor.actionFooter.bounds, to: controller.view)
                            check(controller.view.bounds.contains(frame), "keyboard covered numeric actions")
                            capture("valid-\(style)")
                            if ProcessInfo.processInfo.arguments.contains("--numeric-entry-hold-\(style)") {
                                print("Numeric entry READY for physical display inspection: style \(style), disposable preview, no library design")
                                return
                            }
                            if let mode = find("workspace-expression-keyboard", in: controller.view) as? UIControl {
                                let text = field.text, selection = selectedOffsets()
                                mode.sendActions(for: .touchUpInside)
                                check(field.inputView == nil && field.text == text && field.isFirstResponder && selectedOffsets() == selection, "ABC switch lost text/caret/focus")
                                if let accessory = field.inputAccessoryView,
                                   let numeric = find("workspace-expression-keyboard", in: accessory) as? UIControl { numeric.sendActions(for: .touchUpInside) }
                                check(editor.panelKeypadVisible && field.inputView != nil && field.text == text && selectedOffsets() == selection, "123 switch lost text/caret")
                            } else { issues.append("missing keyboard switch") }
                            _ = editor.textFieldShouldReturn(field)
                            check(!field.isFirstResponder && self.numericToken == token && self.state?.geometry_revision == revision, "Return committed or cancelled geometry")
                            field.becomeFirstResponder()
                            if let done = find("workspace-expression-done", in: controller.view) as? UIControl { done.sendActions(for: .touchUpInside) }
                            check(!editor.panelKeypadVisible, "Done left the embedded pad open")
                            check(!field.isFirstResponder && self.numericToken == token && self.state?.geometry_revision == revision, "Done committed or cancelled geometry")
                            editor.actionFooter.cancel.sendActions(for: .touchUpInside)
                            wait("cancel", { self.numericToken == nil && self.inlineNumericEditor == nil && self.presentedViewController == nil }) {
                                check(self.state?.geometry_revision == revision, "Cancel changed authoritative geometry")
                                runStyle(style + 1)
                            }
                        }
                    }
                }
            }
        }
        func directApply() {
            let before = self.state?.radius?.value
            let revision = self.state?.geometry_revision
            self.beginNumeric(source: self.browserButton)
            wait("direct Apply editor", { self.inlineNumericEditor?.editor.field.isFirstResponder == true }) {
                guard let editor = self.editor else { issues.append("missing direct Apply editor"); finish(); return }
                editor.requiresPreview = true
                check(!editor.actionFooter.apply.isEnabled, "legacy preference did not require Preview")
                editor.requiresPreview = false
                let token = self.numericToken
                editor.field.text = "2 +"; editor.field.sendActions(for: .editingChanged)
                check(editor.actionFooter.apply.isEnabled && !editor.draft.isValidated, "direct Apply required a preview")
                editor.actionFooter.apply.sendActions(for: .touchUpInside)
                wait("direct Apply rejects invalid expression", { !editor.busy }) {
                    check(self.numericToken == token && self.state?.geometry_revision == revision && self.state?.radius?.value == before,
                          "invalid direct Apply changed the document or closed the editor")
                    check(editor.field.text == "2 +" && !editor.message.isHidden && !editor.draft.isValidated,
                          "invalid direct Apply lost its expression or error")
                    capture("direct-apply-invalid")
                    editor.field.text = "9 mm"; editor.field.sendActions(for: .editingChanged)
                    check(!editor.draft.isValidated && editor.actionFooter.apply.isEnabled, "corrected direct Apply required Preview")
                    capture("direct-apply-ready")
                    editor.actionFooter.apply.sendActions(for: .touchUpInside)
                    wait("direct Apply committed", { self.inlineNumericEditor == nil && self.presentedViewController == nil && self.state?.geometry_revision != revision }) {
                        check(self.numericToken == nil, "direct Apply did not finish its transaction")
                        capture("direct-apply-committed")
                        // Successful commits clear selection. Reselect before
                        // reading the radius; Undo also replaces the scene.
                        self.session.command("select.entity", params: ["entity": ["type": "body", "name": "Post"]]) { _ in
                            wait("direct Apply radius", { self.state?.radius?.value == 9 }) {
                                let committedRevision = self.state?.geometry_revision
                                self.run("edit.undo")
                                wait("direct Apply single undo", { self.pending.isEmpty && self.state?.geometry_revision != committedRevision }) {
                                    self.session.command("select.entity", params: ["entity": ["type": "body", "name": "Post"]]) { _ in
                                        wait("restored radius", { self.state?.radius?.value == before }) { runStyle(0) }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        session.command("select.entity", params: ["entity": ["type": "body", "name": "Post"]]) { result in
            guard case .success = result else { issues.append("fixture selection failed"); finish(); return }
            wait("radius selection", { self.state?.radius != nil }) { directApply() }
        }
    }

    /// Actual UIKit regression for floating controls across appearance and safe-area
    /// settings. Foundation rectangle tests cannot observe retained scroll state.
    func checkChromeOnDevice() {
        struct Report: Encodable {
            let compact: Bool
            let appearance: Int
            let useBottomSafeArea: Bool
            let issues: [String]
            let frames: [String: [Double]]
        }
        let savedAppearance = appearance
        let savedBottomSafeArea = useBottomSafeArea
        let expected = generation
        let variants = [false, true].flatMap { bottom in
            [(1, bottom), (2, bottom)]
        }
        var reports: [Report] = []
        func step(_ index: Int) {
            guard active, generation == expected else { return }
            if index == variants.count {
                let restoreDark = ProcessInfo.processInfo.arguments.contains("--chrome-check-restore-dark")
                appearance = restoreDark ? 2 : savedAppearance
                useBottomSafeArea = savedBottomSafeArea
                if restoreDark {
                    UserDefaults.standard.set(2, forKey: "SolidCraft.workspace.appearance")
                }
                applyAppearance(); configureChrome(); updateSubtitle(); updateAvailability(); rebuildMenus()
                view.layoutIfNeeded()
                do {
                    let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                    let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
                    try encoder.encode(reports).write(to: directory.appendingPathComponent("workspace-chrome-check.json"), options: .atomic)
                } catch { print("SolidCraft native chrome check could not save its report: \(error)") }
                print("SolidCraft native chrome check \(reports.allSatisfy { $0.issues.isEmpty } ? "PASS" : "FAIL")")
                return
            }
            canvas.cancelInput()
            appearance = variants[index].0; useBottomSafeArea = variants[index].1
            applyAppearance(); configureChrome(); updateSubtitle(); updateAvailability(); rebuildMenus()
            view.layoutIfNeeded()
            // Check after UIKit completes the reparenting/layout turn, rather
            // than inspecting only the requested policy rectangles.
            DispatchQueue.main.async { [weak self] in
                guard let self, self.active, self.generation == expected else { return }
                self.view.layoutIfNeeded()
                var issues: [String] = []
                var frames: [String: [Double]] = [:]
                let bounds = self.workspace.bounds
                let safe = self.workspace.convert(self.view.safeAreaLayoutGuide.layoutFrame, from: self.view)
                let chrome = WorkspaceLayout.chromeBounds(safeArea: safe, viewport: bounds, useBottomSafeArea: self.useBottomSafeArea)
                for (name, frame) in [("safeArea", safe), ("chrome", chrome), ("header", self.top.frame), ("status", self.status.frame), ("history", self.featureHistory.frame)] {
                    frames[name] = [frame.minX, frame.minY, frame.width, frame.height].map(Double.init)
                }
                if abs(self.featureHistory.frame.maxY - (chrome.maxY - 8)) > 0.5 {
                    issues.append("history does not use the available bottom space")
                }
                if self.compactChrome {
                    let canvas = self.canvas.frame
                    frames["canvas"] = [canvas.minX, canvas.minY, canvas.width, canvas.height].map(Double.init)
                    frames["workspace"] = [bounds.minX, bounds.minY, bounds.width, bounds.height].map(Double.init)
                    if canvas.minY != bounds.minY || canvas.maxY != bounds.maxY {
                        issues.append("viewport does not fill top/bottom system-indicator space")
                    }
                    let headerBounds = self.topStripHeaderEnabled ? bounds : safe
                    if !headerBounds.contains(self.top.frame) || !chrome.contains(self.status.frame) {
                        issues.append("floating chrome escaped its available bounds")
                    }
                    if self.top.frame != self.layout?.topBar {
                        issues.append("document header does not match its resolved placement")
                    }
                    if self.top.frame.minY < safe.minY &&
                        (self.top.frame.minX < bounds.minX + 240 || self.top.frame.maxX > bounds.maxX - 240) {
                        issues.append("raised document header overlaps the system-indicator margins")
                    }
                    for y in [bounds.minY + 1, bounds.maxY - 1] {
                        if self.workspace.hitTest(CGPoint(x: bounds.midX, y: y), with: nil) !== self.canvas {
                            issues.append("viewport edge lost its native input surface")
                        }
                    }
                    for (name, button, host) in [("document", self.documentActionsButton, self.top),
                                                  ("view", self.viewButton, self.navigationScroll),
                                                  (self.homeToolbar ? "home" : "fit", self.homeButton, self.navigationScroll),
                                                  ("projection", self.projectionButton, self.navigationScroll),
                                                  ("properties", self.inspectorButton, self.navigationScroll)] {
                        let frame = button.convert(button.bounds, to: host)
                        let visible = frame.intersection(host.bounds)
                        frames[name] = [frame.minX, frame.minY, frame.width, frame.height].map(Double.init)
                        if visible.width < 43 || visible.height < 43 || button.configuration?.image == nil {
                            issues.append(name + " lost its visible icon/touch region")
                        }
                        if button.isEnabled {
                            for x in [button.bounds.minX + 2, button.bounds.midX, button.bounds.maxX - 2] {
                                for y in [button.bounds.minY + 2, button.bounds.midY, button.bounds.maxY - 2] {
                                    let point = button.convert(CGPoint(x: x, y: y), to: self.view.window)
                                    let hit = self.view.window?.hitTest(point, with: nil)
                                    if hit !== button && hit?.isDescendant(of: button) != true {
                                        issues.append(name + " has an obstructed touch region")
                                    }
                                }
                            }
                        }
                    }
                    if self.browserButton.frame != self.layout?.browserAccess { issues.append("floating browser frame was changed by Auto Layout") }
                    do {
                        let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                        let capture = UIGraphicsImageRenderer(bounds: bounds).image { _ in
                            self.view.drawHierarchy(in: self.view.bounds, afterScreenUpdates: true)
                        }
                        if let png = capture.pngData() { try png.write(to: directory.appendingPathComponent("workspace-chrome-\(index).png"), options: .atomic) }
                    } catch { issues.append("chrome capture failed: \(error)") }
                }
                reports.append(Report(compact: self.compactChrome, appearance: self.appearance, useBottomSafeArea: self.useBottomSafeArea, issues: issues, frames: frames))
                step(index + 1)
            }
        }
        step(0)
    }

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
        if ProcessInfo.processInfo.arguments.contains("--compact-inspector-check") {
            checkCompactInspectorOnDevice()
            return
        }
        if ProcessInfo.processInfo.arguments.contains("--face-extrude-check") {
            checkFaceExtrusionOnDevice()
            return
        }
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

    /// Disposable engine fixture, actual UIKit layout/actions and native captures.
    /// No project-library design or saved preference is changed by this driver.
    func checkCompactInspectorOnDevice() {
        let savedCompact = compactInspector, savedAppearance = appearance
        let fixture: [String: Any] = ["profile": ["faces": [[10, 10, 20]], "face_body": "Inspector stock"], "distance": "5 mm"]
        func find(_ id: String, in root: UIView) -> UIView? {
            if root.accessibilityIdentifier == id { return root }
            return root.subviews.lazy.compactMap { find(id, in: $0) }.first
        }
        func capture(_ name: String) {
            do {
                let directory = try FileManager.default.url(for: .documentDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
                let image = UIGraphicsImageRenderer(bounds: view.bounds).image { _ in view.drawHierarchy(in: view.bounds, afterScreenUpdates: true) }
                if let data = image.pngData() { try data.write(to: directory.appendingPathComponent("compact-inspector-" + name + ".png"), options: .atomic) }
            } catch { assemblyCheckFail("inspector capture: \(error)") }
        }
        func verifyFooter(_ stage: String) -> Bool {
            view.layoutIfNeeded()
            for id in ["workspace-model-cancel", "workspace-model-preview", "workspace-model-apply"] {
                guard let button = find(id, in: inspectorPanel), !button.isDescendant(of: inspectorScroll) else {
                    assemblyCheckFail("\(stage): action still scrolls"); return false
                }
                let frame = button.convert(button.bounds, to: workspace)
                guard inspectorPanel.frame.contains(frame), frame.width >= 44, frame.height >= 44,
                      frame.maxY <= (keyboardTop.map { workspace.convert(CGPoint(x: 0, y: $0), from: view).y } ?? workspace.bounds.maxY) else {
                    assemblyCheckFail("\(stage): action clipped or below keyboard: \(frame)"); return false
                }
            }
            assemblyCheckWrite("PASS \(stage): persistent 44pt actions inside panel and above keyboard")
            return true
        }
        func largeTextLayouts(_ model: ModellingState) -> Bool {
            for width: CGFloat in [320, 380] {
                let host = UIViewController()
                host.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
                host.loadViewIfNeeded(); host.view.frame = CGRect(x: 0, y: 0, width: width, height: 600)
                let panel = WorkspacePanel(title: "Extrude", onClose: {})
                let form = ModellingInspector(state: model)
                panel.frame = host.view.bounds; host.view.addSubview(panel)
                panel.setFooter(form.footer)
                let scroll = UIScrollView(); panel.content.addSubview(scroll); scroll.addSubview(form)
                form.translatesAutoresizingMaskIntoConstraints = false
                NSLayoutConstraint.activate([
                    form.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 16),
                    form.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -16),
                    form.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 12),
                    form.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -12),
                    form.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -32)
                ])
                host.view.layoutIfNeeded(); scroll.frame = panel.content.bounds
                panel.setNeedsLayout(); panel.layoutIfNeeded(); form.footer.layoutIfNeeded()
                for id in ["workspace-model-cancel", "workspace-model-preview", "workspace-model-apply"] {
                    guard let action = find(id, in: panel) else { assemblyCheckFail("large-text action missing"); return false }
                    let frame = action.convert(action.bounds, to: panel)
                    guard panel.bounds.contains(frame), frame.width >= 44, frame.height >= 44 else {
                        assemblyCheckFail("large-text footer clipped at width \(width): \(frame)"); return false
                    }
                }
            }
            assemblyCheckWrite("PASS synthetic UIKit inspector layouts at widths 320/380 and accessibility XXXL; actions retained")
            return true
        }
        func legacy() {
            assemblyCheckCancelled = { [weak self] _ in
                guard let self else { return }
                self.compactInspector = false
                self.assemblyCheckBegin("solid.extrude", params: fixture) { _ in
                    guard let form = self.modellingInspector, !form.compact, form.footer.isDescendant(of: self.inspectorScroll) else {
                        self.assemblyCheckFail("previous inspector preference"); return
                    }
                    self.view.layoutIfNeeded(); capture("legacy")
                    self.assemblyCheckWrite("PASS previous inspector: scrolling footer and original field order retained")
                    self.assemblyCheckCancelled = { _ in
                        self.compactInspector = savedCompact; self.appearance = savedAppearance; self.applyAppearance()
                        self.assemblyCheckWrite("PASS complete; no library project changed; hardware Pencil/VoiceOver unverified")
                        self.assemblyCheckRunning = false; self.assemblyCheckTicket &+= 1
                        self.assemblyCheckSignal?.cancel(); self.assemblyCheckSignal = nil
                    }
                    self.cancelModelling()
                }
            }
            cancelModelling()
        }
        compactInspector = true; appearance = 2; applyAppearance()
        let commands: [(String, [String: Any])] = [
            ("file.new", ["name": "Compact inspector runtime"]),
            ("solid.box", ["length": 40, "width": 30, "height": 20, "body_name": "Inspector stock"]),
            ("view.isometric", [:]), ("view.fit", [:])
        ]
        assemblyCheckCommands(commands, index: 0) { [weak self] in
            guard let self else { return }
            self.view.layoutIfNeeded()
            let canvasFrame = self.canvas.frame
            self.assemblyCheckBegin("solid.extrude", params: fixture) { _ in
                guard let form = self.modellingInspector,
                      let apply = find("workspace-model-apply", in: self.inspectorPanel) as? UIButton,
                      !apply.isEnabled, verifyFooter("initial") else { self.assemblyCheckFail("unvalidated Apply enabled"); return }
                self.view.layoutIfNeeded()
                guard self.canvas.frame == canvasFrame else { self.assemblyCheckFail("opening inspector resized Metal canvas"); return }
                for id in ["workspace-model-field-profile", "workspace-model-field-distance", "workspace-model-inline-operation-0", "workspace-model-inline-operation-3"] {
                    guard let input = find(id, in: self.inspectorPanel), self.inspectorScroll.bounds.contains(input.convert(input.bounds, to: self.inspectorScroll)) else {
                        self.assemblyCheckFail("primary input requires scrolling: \(id)"); return
                    }
                }
                self.assemblyCheckWrite("PASS primary source, distance and boolean choices visible without scrolling; canvas unchanged")
                guard let summary = find("workspace-model-summary-profile", in: form) as? UILabel, summary.text == "Inspector stock" else {
                    self.assemblyCheckFail("reference summary exposed command JSON or invented a face label"); return
                }
                self.assemblyCheckEvaluate(commit: false) { model in
                    guard apply.isEnabled else { self.assemblyCheckFail("matching preview did not enable Apply"); return }
                    self.assemblyCheckWaitForFrame(model, preview: true, stage: "compact-inspector") {
                        capture("dark")
                        self.appearance = 1; self.applyAppearance(); self.view.layoutIfNeeded(); capture("light")
                        for id in ["workspace-model-cancel", "workspace-model-preview", "workspace-model-apply"] {
                            guard let button = find(id, in: self.inspectorPanel) as? UIButton, button.configuration?.image == nil else {
                                self.assemblyCheckFail("appearance refresh restored footer artwork/wrapping"); return
                            }
                        }
                        self.appearance = 2; self.applyAppearance()
                        guard let distance = find("workspace-model-field-distance", in: form) as? UITextField else { self.assemblyCheckFail("distance field"); return }
                        distance.text = "5 mm +"; distance.sendActions(for: .editingChanged)
                        guard !apply.isEnabled, form.params["distance"] == .string("5 mm +") else { self.assemblyCheckFail("edited raw expression reused preview"); return }
                        form.beginContextRefresh()
                        guard !apply.isEnabled else { self.assemblyCheckFail("reference refresh reused preview"); return }
                        form.contextRefreshDiscarded()
                        self.assemblyCheckWrite("PASS raw edit/reference refresh invalidate Apply")
                        let popupEntry = form.onEditValue
                        form.onEditValue = nil // Exercise the retained direct/full-keyboard field path.
                        guard distance.becomeFirstResponder() else { self.assemblyCheckFail("distance did not accept keyboard focus"); return }
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) {
                            guard form.textEntryActive, let keyboard = self.keyboardTop else {
                                self.assemblyCheckFail("keyboard focus or frame notification missing"); return
                            }
                            self.assemblyCheckWrite("PASS actual field focus; observed keyboard boundary y=\(keyboard)")
                            guard verifyFooter("keyboard") else { return }
                            capture("keyboard"); form.endEditing(true); form.onEditValue = popupEntry
                            DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                                form.editExpression("distance", expression: "5 mm")
                                guard let subtract = find("workspace-model-inline-operation-3", in: form) as? UIButton else {
                                    self.assemblyCheckFail("Subtract button missing"); return
                                }
                                self.assemblyCheckEvaluated = { result, commit in
                                    guard !commit, case .success(let next) = result, next.valid == true,
                                          next.params?["operation"] == .string("cut"), apply.isEnabled else {
                                        self.assemblyCheckFail("operation change did not validate the current Subtract draft"); return
                                    }
                                    self.assemblyCheckWaitForFrame(next, preview: true, stage: "compact-inspector-subtract") {
                                        capture("subtract")
                                        self.assemblyCheckWrite("PASS real Subtract action previews with engine-owned direction and explicit override")
                                        guard largeTextLayouts(next) else { return }
                                        legacy()
                                    }
                                }
                                subtract.sendActions(for: .touchUpInside)
                                guard !apply.isEnabled, form.params["_auto_subtract"] == .bool(false) else {
                                    self.assemblyCheckFail("operation change reused validation or dropped explicit override"); return
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    func checkFaceExtrusionOnDevice() {
        let commands: [(String, [String: Any])] = [
            ("file.new", ["name": "Face extrusion inline runtime"]),
            ("solid.box", ["length": 40, "width": 30, "height": 20, "body_name": "Inline stock"]),
            ("view.isometric", [:]), ("view.fit", [:])
        ]
        assemblyCheckCommands(commands, index: 0) { [weak self] in
            self?.assemblyCheckBegin("solid.extrude", params: ["profile": ["faces": [[10, 10, 20]], "face_body": "Inline stock"], "distance": "5 mm"]) { [weak self] _ in
                self?.assemblyCheckEvaluate(commit: false) { [weak self] model in
                    self?.assemblyCheckWaitForFrame(model, preview: true, stage: "face-inline-new") { [weak self] in self?.checkFaceSubtractButton() }
                }
            }
        }
    }

    func checkFaceSubtractButton() {
        func button(in view: UIView) -> UIButton? {
            if view.accessibilityIdentifier == "workspace-model-inline-operation-3" { return view as? UIButton }
            return view.subviews.lazy.compactMap { button(in: $0) }.first
        }
        guard let form = modellingInspector, let subtract = button(in: form), subtract.menu == nil, subtract.isEnabled else {
            assemblyCheckFail("Subtract is not an inline button"); return
        }
        assemblyCheckEvaluated = { [weak self] result, commit in
            guard let self else { return }
            guard case .success(let model) = result, !commit, model.valid == true,
                  model.params?["operation"] == .string("cut"), model.params?["direction"] == .string("negative") else {
                self.assemblyCheckFail("Tapping Subtract did not automatically preview an inward cut"); return
            }
            self.assemblyCheckWrite("PASS inline Subtract button automatically previewed a negative 5 mm cut")
            self.assemblyCheckWaitForFrame(model, preview: true, stage: "face-inline-subtract") { [weak self] in
                guard let self else { return }
                self.inspectorOpen = false; self.relayoutPanels(); self.editModelValue()
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { [weak self] in
                    guard let self, let editor = self.modelValueEditor, button(in: editor.view) != nil else {
                        self?.assemblyCheckFail("On-model distance editor has no inline Subtract button"); return
                    }
                    self.assemblyCheckWaitForFrame(model, preview: true, stage: "face-inline-distance-editor") { [weak self, weak editor] in
                        guard let self, let editor else { self?.assemblyCheckFail("Inline editor closed before Apply"); return }
                        self.assemblyCheckEvaluated = { [weak self] result, commit in
                            guard let self else { return }
                            guard case .success = result, commit else { self.assemblyCheckFail("Inline Subtract Apply failed"); return }
                            self.assemblyCheckWrite("PASS inline face Subtract Apply; production callbacks and Metal frames")
                            self.assemblyCheckRunning = false; self.assemblyCheckTicket &+= 1
                            self.assemblyCheckSignal?.cancel(); self.assemblyCheckSignal = nil
                        }
                        editor.editor.onApply?(editor.editor.field.text ?? "")
                    }
                }
            }
        }
        subtract.sendActions(for: .touchUpInside)
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

enum SketchText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.sketch." + key) }
}
