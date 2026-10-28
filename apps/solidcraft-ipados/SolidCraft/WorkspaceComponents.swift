import UIKit

/// Native presentation tokens stay inside the approved UIKit boundary. Geometry,
/// camera colours and highlights continue to come from the shared renderer.
enum WorkspaceStyle {
    static let spacing: CGFloat = 12
    static let inset: CGFloat = 16
    static let radius: CGFloat = 14
    static let touch: CGFloat = 44
    static let background = UIColor.systemBackground
    static let surface = UIColor.secondarySystemBackground
    static let separator = UIColor.separator
    static let selectionFill = UIColor.tertiarySystemFill
    static let scrim = UIColor.systemBackground.withAlphaComponent(0.45)
    static let foreground = UIColor.label
    static let secondary = UIColor.secondaryLabel
    static let accent = UIColor.systemBlue
    static let selectedForeground = UIColor.white
    static let error = UIColor.systemRed

    static func floatingSurface(_ view: UIView) {
        view.backgroundColor = surface.withAlphaComponent(0.96)
        view.layer.cornerRadius = radius
        view.layer.borderWidth = 0.5
        view.layer.borderColor = separator.cgColor
        view.clipsToBounds = true
    }

    static func text(_ key: String) -> String { NativeText.value("ui.ipados.workspace." + key) }
    static func label(_ style: UIFont.TextStyle = .body, secondary: Bool = false) -> UILabel {
        let label = UILabel()
        label.font = .preferredFont(forTextStyle: style)
        label.adjustsFontForContentSizeCategory = true
        label.textColor = secondary ? self.secondary : foreground
        label.numberOfLines = 0
        return label
    }
}

/// The native image cache consumes Xcode's vector assets generated from the same
/// SVG masters, semantic aliases and palette as egui. No native icon registry.
enum WorkspaceIcon {
    private static let cache = NSCache<NSString, UIImage>()
    private static let commands: [String: String] = {
        guard let url = Bundle.main.url(forResource: "SolidCraftCommandIcons", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let values = try? JSONDecoder().decode([String: String].self, from: data) else { return [:] }
        cache.countLimit = 256
        return values
    }()

    static func command(_ id: String, traits: UITraitCollection, size: CGFloat = 28) -> UIImage? {
        guard let icon = commands[id] else { return nil }
        return image(icon, traits: traits, size: size)
    }

    static func image(_ id: String, traits: UITraitCollection, size: CGFloat = 28) -> UIImage? {
        let key = "\(id):\(traits.userInterfaceStyle.rawValue):\(traits.displayScale):\(size)" as NSString
        if let cached = cache.object(forKey: key) { return cached }
        let name = (size * max(1, traits.displayScale) <= 24 ? "sc-small-" : "sc-") + id
        guard let source = UIImage(named: name, in: .main, compatibleWith: traits) else { return nil }
        let format = UIGraphicsImageRendererFormat()
        format.scale = max(1, traits.displayScale)
        let rendered = UIGraphicsImageRenderer(size: CGSize(width: size, height: size), format: format).image { _ in
            source.draw(in: CGRect(x: 0, y: 0, width: size, height: size))
        }.withRenderingMode(.alwaysOriginal)
        cache.setObject(rendered, forKey: key)
        return rendered
    }
}

final class WorkspaceStatusLabel: UILabel {
    override init(frame: CGRect) {
        super.init(frame: frame)
        font = .preferredFont(forTextStyle: .caption1)
        adjustsFontForContentSizeCategory = true
        textColor = WorkspaceStyle.secondary
        numberOfLines = 2
    }
    convenience init() { self.init(frame: .zero) }
    required init?(coder: NSCoder) { nil }
    override func drawText(in rect: CGRect) { super.drawText(in: rect.insetBy(dx: 10, dy: 4)) }
    override func sizeThatFits(_ size: CGSize) -> CGSize {
        let fitted = super.sizeThatFits(CGSize(width: max(0, size.width - 20), height: max(0, size.height - 8)))
        return CGSize(width: fitted.width + 20, height: fitted.height + 8)
    }
}

/// Icon-first chrome retains full labels for VoiceOver, pointer tooltips and
/// menus. Forms continue to use text. Missing artwork keeps a readable title.
final class WorkspaceButton: UIButton {
    private var fullTitle: String
    private var iconID: String?
    private var commandID: String?
    private var compactIcon = false
    init(_ title: String, prominent: Bool = false, action: @escaping () -> Void) {
        fullTitle = title
        super.init(frame: .zero)
        var config = prominent ? UIButton.Configuration.filled() : .plain()
        config.title = title
        config.cornerStyle = .medium
        config.contentInsets = NSDirectionalEdgeInsets(top: 10, leading: 10, bottom: 10, trailing: 10)
        configuration = config
        titleLabel?.adjustsFontForContentSizeCategory = true
        titleLabel?.numberOfLines = 2
        heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        widthAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        accessibilityLabel = title
        toolTip = title
        isPointerInteractionEnabled = true
        showsLargeContentViewer = true
        largeContentTitle = title
        addInteraction(UILargeContentViewerInteraction())
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitDisplayScale.self]) { (button: WorkspaceButton, _: UITraitCollection) in
            button.refreshIcon()
        }
        addAction(UIAction { _ in action() }, for: .touchUpInside)
    }
    required init?(coder: NSCoder) { nil }

    func updateTitle(_ title: String) {
        fullTitle = title
        configuration?.title = title
        largeContentTitle = title; toolTip = title
    }

    func artwork(_ id: String, compact: Bool = false) {
        iconID = id; commandID = nil; compactIcon = compact; refreshIcon()
    }
    func commandArtwork(_ id: String, compact: Bool = false) {
        commandID = id; iconID = nil; compactIcon = compact; refreshIcon()
    }
    private func refreshIcon() {
        let image = commandID.flatMap { WorkspaceIcon.command($0, traits: traitCollection) }
            ?? iconID.flatMap { WorkspaceIcon.image($0, traits: traitCollection) }
        var config = configuration
        config?.image = image
        config?.imagePadding = compactIcon ? 0 : 8
        config?.title = compactIcon && image != nil ? nil : fullTitle
        config?.contentInsets = NSDirectionalEdgeInsets(top: compactIcon ? 8 : 10, leading: 8, bottom: compactIcon ? 8 : 10, trailing: 8)
        configuration = config
        accessibilityLabel = fullTitle
        largeContentImage = image
    }

    func availability(enabled: Bool, reason: String? = nil) {
        isEnabled = enabled
        accessibilityHint = reason
        accessibilityValue = reason
        toolTip = reason.map { fullTitle + " · " + $0 } ?? fullTitle
    }
    func active(_ value: Bool) {
        isSelected = value
        var config = configuration
        let fill = compactIcon ? WorkspaceStyle.selectionFill : WorkspaceStyle.accent
        config?.baseBackgroundColor = value ? fill : .clear
        config?.baseForegroundColor = value && !compactIcon ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
        if value { config?.background.backgroundColor = fill }
        else { config?.background.backgroundColor = .clear }
        configuration = config
        if value { accessibilityTraits.insert(.selected) }
        else { accessibilityTraits.remove(.selected) }
    }
}

/// Shared chrome for docked panels and compact contextual sheets.
final class WorkspacePanel: UIView {
    let content = UIView()
    private let heading = WorkspaceStyle.label(.headline)
    private let close: WorkspaceButton
    init(title: String, onClose: @escaping () -> Void) {
        close = WorkspaceButton(WorkspaceStyle.text("close_short"), action: onClose)
        super.init(frame: .zero)
        backgroundColor = WorkspaceStyle.surface
        close.artwork("close", compact: true)
        layer.cornerRadius = WorkspaceStyle.radius
        layer.borderWidth = 0.5
        layer.borderColor = WorkspaceStyle.separator.cgColor
        clipsToBounds = true
        heading.text = title
        heading.accessibilityTraits = .header
        heading.numberOfLines = 1
        heading.lineBreakMode = .byTruncatingTail
        heading.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        close.accessibilityLabel = WorkspaceStyle.text("close")
        close.titleLabel?.numberOfLines = 1
        close.configuration?.titleLineBreakMode = .byClipping
        close.setContentCompressionResistancePriority(.required, for: .horizontal)
        close.setContentHuggingPriority(.required, for: .horizontal)
        for child in [heading, close, content] { child.translatesAutoresizingMaskIntoConstraints = false; addSubview(child) }
        NSLayoutConstraint.activate([
            heading.leadingAnchor.constraint(equalTo: leadingAnchor, constant: WorkspaceStyle.inset),
            heading.topAnchor.constraint(equalTo: topAnchor, constant: 14),
            heading.trailingAnchor.constraint(lessThanOrEqualTo: close.leadingAnchor, constant: -4),
            close.topAnchor.constraint(equalTo: topAnchor, constant: 4),
            close.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -4),
            content.topAnchor.constraint(greaterThanOrEqualTo: close.bottomAnchor, constant: 6),
            content.topAnchor.constraint(greaterThanOrEqualTo: heading.bottomAnchor, constant: 6),
            content.leadingAnchor.constraint(equalTo: leadingAnchor),
            content.trailingAnchor.constraint(equalTo: trailingAnchor),
            content.bottomAnchor.constraint(equalTo: bottomAnchor)
        ])
        let contentTop = content.topAnchor.constraint(equalTo: close.bottomAnchor, constant: 6)
        contentTop.priority = .defaultHigh
        contentTop.isActive = true
        close.accessibilityIdentifier = "workspace-close-" + title
        registerForTraitChanges([UITraitUserInterfaceStyle.self]) { (panel: WorkspacePanel, _: UITraitCollection) in
            panel.layer.borderColor = WorkspaceStyle.separator.cgColor
        }
    }
    required init?(coder: NSCoder) { nil }
}

final class WorkspacePropertyRow: UIStackView {
    init(label: String, value: String) {
        super.init(frame: .zero)
        axis = .vertical; spacing = 3
        let name = WorkspaceStyle.label(.caption1, secondary: true)
        name.text = label
        let detail = WorkspaceStyle.label(.body)
        detail.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 16, weight: .medium))
        detail.text = value
        addArrangedSubview(name); addArrangedSubview(detail)
        isAccessibilityElement = true
        accessibilityLabel = label; accessibilityValue = value
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
}

/// Inline validation/progress is used by both panels and numeric transactions.
final class WorkspaceMessage: UIStackView {
    private let spinner = UIActivityIndicatorView(style: .medium)
    private let label = WorkspaceStyle.label(.footnote, secondary: true)
    init() {
        super.init(frame: .zero)
        axis = .horizontal; alignment = .center; spacing = 8
        addArrangedSubview(spinner); addArrangedSubview(label)
        spinner.hidesWhenStopped = true
        isHidden = true
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
    func show(_ message: String?, error: Bool = false, busy: Bool = false) {
        label.text = message
        label.textColor = error ? WorkspaceStyle.error : WorkspaceStyle.secondary
        if busy { spinner.startAnimating() } else { spinner.stopAnimating() }
        isHidden = message?.isEmpty != false && !busy
        accessibilityLabel = message
    }
}

/// Editing text is deliberately provisional. Only explicit Preview/Apply invoke
/// the upstream expression evaluator; cancellation never rewrites the document.
final class NumericExpressionEditor: UIStackView, UITextFieldDelegate {
    let field = UITextField()
    let message = WorkspaceMessage()
    private let evaluation = WorkspaceStyle.label(.footnote, secondary: true)
    private(set) var busy = false
    private let preview: WorkspaceButton
    private let apply: WorkspaceButton
    private let cancel: WorkspaceButton
    var onPreview: ((String) -> Void)?
    var onApply: ((String) -> Void)?
    var onCancel: (() -> Void)?
    var onFocus: (() -> Void)?
    var onChange: (() -> Void)?
    private var usesNumericKeyboard = false

    init(expression: String, units: String, showsActions: Bool = true, numericKeyboard: Bool = false) {
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true, action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        super.init(frame: .zero)
        axis = .vertical; spacing = 12
        preview.artwork("eye"); apply.artwork("finish"); cancel.artwork("close")
        let heading = WorkspaceStyle.label(.caption1, secondary: true)
        heading.text = WorkspaceStyle.text("expression") + (units.isEmpty ? "" : " · " + units)
        addArrangedSubview(heading)
        field.text = expression
        field.borderStyle = .roundedRect
        field.font = UIFontMetrics(forTextStyle: .body).scaledFont(for: .monospacedSystemFont(ofSize: 18, weight: .medium))
        field.adjustsFontForContentSizeCategory = true
        field.keyboardType = .asciiCapable
        field.returnKeyType = .done
        field.autocorrectionType = .no
        field.autocapitalizationType = .none
        field.smartDashesType = .no; field.smartQuotesType = .no
        field.delegate = self
        usesNumericKeyboard = numericKeyboard
        if numericKeyboard { field.inputView = ExpressionKeypad(field: field) }
        field.accessibilityLabel = heading.text
        field.accessibilityIdentifier = "workspace-numeric-expression"
        field.heightAnchor.constraint(greaterThanOrEqualToConstant: 48).isActive = true
        field.addAction(UIAction { [weak self] _ in
            guard let self else { return }
            self.message.show(nil)
            self.evaluation.isHidden = true
            self.onChange?()
        }, for: .editingChanged)
        // A scrolling accessory retains every expression character and 44-point
        // target in narrow windows instead of compressing a fixed toolbar.
        let accessory = UIView(frame: CGRect(x: 0, y: 0, width: 320, height: 52))
        accessory.backgroundColor = WorkspaceStyle.surface
        accessory.autoresizingMask = .flexibleWidth
        let scroll = UIScrollView(frame: accessory.bounds.insetBy(dx: 4, dy: 4))
        scroll.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        let keys = UIStackView()
        keys.axis = .horizontal; keys.spacing = 4
        let keyboard = WorkspaceButton(HandleText.text("keyboard")) { [weak self] in
            guard let self else { return }
            self.usesNumericKeyboard.toggle()
            self.field.inputView = self.usesNumericKeyboard ? ExpressionKeypad(field: self.field) : nil
            self.field.reloadInputViews()
        }
        keyboard.accessibilityIdentifier = "workspace-expression-keyboard"
        keys.addArrangedSubview(keyboard)
        for text in ["−", ".", "(", ")", "+", "*", "/", "mm", "cm", "in", "deg"] {
            keys.addArrangedSubview(WorkspaceButton(text) { [weak self] in self?.field.insertText(text == "−" ? "-" : text) })
        }
        keys.addArrangedSubview(WorkspaceButton(WorkspaceStyle.text("keyboard_done")) { [weak self] in self?.field.resignFirstResponder() })
        accessory.addSubview(scroll); scroll.addSubview(keys)
        keys.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            keys.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor),
            keys.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor),
            keys.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor),
            keys.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor),
            keys.heightAnchor.constraint(equalTo: scroll.frameLayoutGuide.heightAnchor)
        ])
        field.inputAccessoryView = accessory
        addArrangedSubview(field)
        evaluation.isHidden = true
        evaluation.accessibilityIdentifier = "workspace-numeric-evaluation"
        addArrangedSubview(evaluation)
        let hint = WorkspaceStyle.label(.footnote, secondary: true)
        hint.text = WorkspaceStyle.text("decimal_help")
        addArrangedSubview(hint)
        addArrangedSubview(message)
        let actions = UIStackView(arrangedSubviews: [preview, apply])
        actions.axis = .horizontal; actions.spacing = 8; actions.distribution = .fillEqually
        addArrangedSubview(actions); addArrangedSubview(cancel)
        actions.isHidden = !showsActions; cancel.isHidden = !showsActions
        preview.addAction(UIAction { [weak self] _ in guard let self else { return }; self.field.resignFirstResponder(); self.onPreview?(self.field.text ?? "") }, for: .touchUpInside)
        apply.addAction(UIAction { [weak self] _ in guard let self else { return }; self.field.resignFirstResponder(); self.onApply?(self.field.text ?? "") }, for: .touchUpInside)
        cancel.addAction(UIAction { [weak self] _ in self?.onCancel?() }, for: .touchUpInside)
        preview.accessibilityIdentifier = "workspace-numeric-preview"
        apply.accessibilityIdentifier = "workspace-numeric-apply"
        cancel.accessibilityIdentifier = "workspace-numeric-cancel"
    }
    required init(coder: NSCoder) {
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        super.init(coder: coder)
    }
    func setEvaluation(value: Double, units: String) {
        guard value.isFinite else { evaluation.isHidden = true; return }
        let formatter = NumberFormatter()
        formatter.locale = .current
        formatter.numberStyle = .decimal
        formatter.usesSignificantDigits = true
        formatter.minimumSignificantDigits = 1
        formatter.maximumSignificantDigits = 12
        guard let formatted = formatter.string(from: NSNumber(value: value)) else { evaluation.isHidden = true; return }
        let resolved = formatted + " " + units
        evaluation.text = WorkspaceStyle.text("numeric_resolved") + "\n" + resolved
        evaluation.accessibilityLabel = WorkspaceStyle.text("numeric_resolved")
        evaluation.accessibilityValue = resolved
        evaluation.isHidden = false
    }
    func setBusy(_ value: Bool, committing: Bool = false) {
        busy = value
        field.isEnabled = !value; preview.isEnabled = !value; apply.isEnabled = !value
        cancel.isEnabled = !(value && committing)
        message.show(value ? NativeText.value("ui.ipados.updating") : nil, busy: value)
    }
    func textFieldDidBeginEditing(_ textField: UITextField) { onFocus?() }
    func textFieldShouldReturn(_ textField: UITextField) -> Bool { textField.resignFirstResponder(); return true }
}

enum HandleText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.handles." + key) }
}

/// This inserts expression characters through UIKit. Parsing and units stay in Rust.
private final class ExpressionKeypad: UIInputView {
    init(field: UITextField) {
        super.init(frame: CGRect(x: 0, y: 0, width: 320, height: 256), inputViewStyle: .keyboard)
        allowsSelfSizing = true
        let rows = UIStackView()
        rows.axis = .vertical; rows.spacing = 6; rows.distribution = .fillEqually
        rows.translatesAutoresizingMaskIntoConstraints = false
        for values in [["7", "8", "9", "⌫"], ["4", "5", "6", "−"], ["1", "2", "3", "+"], ["0", ".", "*", "/"]] {
            let row = UIStackView()
            row.axis = .horizontal; row.spacing = 6; row.distribution = .fillEqually
            for value in values {
                let key = WorkspaceButton(value) { [weak field] in
                    if value == "⌫" { field?.deleteBackward() }
                    else { field?.insertText(value == "−" ? "-" : value) }
                }
                if value == "⌫" { key.accessibilityLabel = HandleText.text("delete") }
                key.accessibilityIdentifier = "workspace-keypad-" + value
                row.addArrangedSubview(key)
            }
            rows.addArrangedSubview(row)
        }
        addSubview(rows)
        NSLayoutConstraint.activate([
            heightAnchor.constraint(equalToConstant: 256),
            rows.topAnchor.constraint(equalTo: topAnchor, constant: 8),
            rows.bottomAnchor.constraint(equalTo: safeAreaLayoutGuide.bottomAnchor, constant: -8),
            rows.centerXAnchor.constraint(equalTo: centerXAnchor),
            rows.widthAnchor.constraint(lessThanOrEqualToConstant: 560),
            rows.leadingAnchor.constraint(greaterThanOrEqualTo: leadingAnchor, constant: 8),
            rows.trailingAnchor.constraint(lessThanOrEqualTo: trailingAnchor, constant: -8)
        ])
        let width = rows.widthAnchor.constraint(equalTo: widthAnchor, constant: -16)
        width.priority = .defaultHigh; width.isActive = true
    }
    required init?(coder: NSCoder) { nil }
}

/// The compact entry sheet shares the current modelling transaction and actions.
final class ModelValueEditorController: UIViewController {
    let editor: NumericExpressionEditor
    init(value: ModelValuePresentation, expression: String) {
        editor = NumericExpressionEditor(expression: expression, units: value.units, numericKeyboard: true)
        super.init(nibName: nil, bundle: nil)
        modalPresentationStyle = .popover
        preferredContentSize = CGSize(width: 340, height: 360)
    }
    required init?(coder: NSCoder) { nil }
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        editor.field.resignFirstResponder()
    }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.surface
        let scroll = UIScrollView()
        scroll.translatesAutoresizingMaskIntoConstraints = false
        editor.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(scroll); scroll.addSubview(editor)
        NSLayoutConstraint.activate([
            scroll.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor),
            scroll.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            scroll.leadingAnchor.constraint(equalTo: view.leadingAnchor), scroll.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            editor.topAnchor.constraint(equalTo: scroll.contentLayoutGuide.topAnchor, constant: 16),
            editor.bottomAnchor.constraint(equalTo: scroll.contentLayoutGuide.bottomAnchor, constant: -16),
            editor.leadingAnchor.constraint(equalTo: scroll.contentLayoutGuide.leadingAnchor, constant: 16),
            editor.trailingAnchor.constraint(equalTo: scroll.contentLayoutGuide.trailingAnchor, constant: -16),
            editor.widthAnchor.constraint(equalTo: scroll.frameLayoutGuide.widthAnchor, constant: -32)
        ])
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        editor.field.becomeFirstResponder(); editor.field.selectAll(nil)
    }
}

enum ModellingText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.modelling." + key) }
    static func label(_ value: String) -> String { PresentationLabelKey.label(value, lookup: NativeText.value) }
}

enum AssemblyText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.assembly." + key) }
}

/// A form over Rust-authored fields and opaque JSON. It has no operation map,
/// topology rules or numeric parser. Only a matching validated preview enables Apply.
final class ModellingInspector: UIStackView {
    private(set) var params: [String: PresentationJSON]
    private(set) var fields: [ModellingState.Field]
    private var validated: [String: PresentationJSON]?
    private var locallyEdited = false
    private var editors: [NumericExpressionEditor] = []
    private var textFields: [UITextField] = []
    private var fieldViews: [UIView] = []
    private var busy = false
    private var inputWhileBusy = false
    private var committing = false
    private var sliderActive = false
    private var contextRefreshing = false
    private var sliders: [UISlider] = []
    let message = WorkspaceMessage()
    private let fieldsStack = UIStackView()
    private let preview: WorkspaceButton
    private let apply: WorkspaceButton
    private let cancel: WorkspaceButton
    private let select: WorkspaceButton
    var onPreview: (([String: PresentationJSON]) -> Void)?
    var onLivePreview: (([String: PresentationJSON]) -> Void)?
    var onContextChange: (([String: PresentationJSON]) -> Void)?
    var onApply: (([String: PresentationJSON]) -> Void)?
    var onCancel: (() -> Void)?
    var onFocus: (() -> Void)?
    var onChange: (() -> Void)?
    var onSelect: (() -> Void)?
    var focusedField: UITextField? { editors.first(where: { $0.field.isFirstResponder })?.field ?? textFields.first(where: { $0.isFirstResponder }) }
    var textEntryActive: Bool { focusedField != nil }
    var handleEntryEnabled: Bool { !busy && !contextRefreshing && !committing }
    var hasLocalChanges: Bool { locallyEdited }

    func editExpression(_ path: String, expression: String) {
        setParameter(path, value: .string(expression)); changed()
        if !textEntryActive { rebuildFields() }
    }

    init(state: ModellingState) {
        params = state.params ?? [:]
        fields = state.fields ?? []
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true, action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        select = WorkspaceButton(ModellingText.text("select_in_viewport"), action: {})
        super.init(frame: .zero)
        axis = .vertical; spacing = 16
        preview.artwork("eye"); apply.artwork("finish"); cancel.artwork("close"); select.artwork("select")
        let heading = WorkspaceStyle.label(.headline)
        if let id = state.feature, let feature = state.workspace.modelling?.timeline.first(where: { $0.id == id }) {
            heading.text = ModellingText.text("editing") + ": " + feature.name
        } else if let operation = state.operation {
            heading.text = ((state.workspace.modelling?.tools ?? []) + (state.workspace.assembly?.tools ?? [])).first(where: { $0.id == operation }).map { ModellingText.label($0.label) }
                ?? ModellingText.label("cmd." + operation)
        } else { heading.text = ModellingText.text("tools") }
        heading.accessibilityTraits = .header
        addArrangedSubview(heading)
        let hint = WorkspaceStyle.label(.footnote, secondary: true)
        hint.text = ModellingText.text("draft_hint")
        addArrangedSubview(hint)
        fieldsStack.axis = .vertical; fieldsStack.spacing = 16
        addArrangedSubview(fieldsStack); addArrangedSubview(select); addArrangedSubview(message)
        let actions = UIStackView(arrangedSubviews: [preview, apply])
        actions.axis = .horizontal; actions.spacing = 8; actions.distribution = .fillEqually
        addArrangedSubview(actions); addArrangedSubview(cancel)
        preview.addAction(UIAction { [weak self] _ in
            guard let self, !self.busy else { return }; self.endEditing(true); self.onPreview?(self.params)
        }, for: .touchUpInside)
        apply.addAction(UIAction { [weak self] _ in
            guard let self, !self.busy, self.validated == self.params else { return }; self.endEditing(true); self.onApply?(self.params)
        }, for: .touchUpInside)
        cancel.addAction(UIAction { [weak self] _ in self?.onCancel?() }, for: .touchUpInside)
        select.addAction(UIAction { [weak self] _ in self?.endEditing(true); self?.onSelect?() }, for: .touchUpInside)
        preview.accessibilityIdentifier = "workspace-model-preview"
        apply.accessibilityIdentifier = "workspace-model-apply"
        cancel.accessibilityIdentifier = "workspace-model-cancel"
        rebuildFields(); updateAvailability()
    }
    required init(coder: NSCoder) {
        params = [:]; fields = []
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        select = WorkspaceButton(ModellingText.text("select_in_viewport"), action: {})
        super.init(coder: coder)
    }

    func evaluated(_ state: ModellingState) {
        busy = false; committing = false; inputWhileBusy = false
        if state.params == params, let fields = state.fields { self.fields = fields }
        validated = state.valid == true && state.params == params ? params : nil
        if validated != nil { locallyEdited = false }
        if state.params == params && !sliderActive && !textEntryActive { rebuildFields() }
        updateAvailability()
        let feedback = (state.feedback ?? []).map(ModellingText.label)
        message.show(([validated == nil ? ModellingText.text("preview_required") : WorkspaceStyle.text("preview_ready")] + feedback).joined(separator: "\n"))
    }
    func inspected(_ state: ModellingState) {
        // Inspection refreshes available native references. Current draft text
        // remains owned by its fields; inspection never silently replaces an input.
        if let fields = state.fields { self.fields = fields }
        if !locallyEdited, let params = state.params { self.params = params }
        validated = state.valid == true && state.params == params ? params : nil
        rebuildFields(); updateAvailability()
        message.show(([validated == nil ? ModellingText.text("preview_required") : WorkspaceStyle.text("preview_ready")] + (state.feedback ?? []).map(ModellingText.label)).joined(separator: "\n"))
    }
    func beginContextRefresh() {
        contextRefreshing = true; validated = nil; busy = false; inputWhileBusy = false
        editors.forEach { $0.setBusy(false) }
        updateAvailability()
        message.show(NativeText.value("ui.ipados.updating"), busy: true)
    }
    func contextRefreshed(_ state: ModellingState) {
        if let params = state.params { self.params = params }
        if let fields = state.fields { self.fields = fields }
        contextRefreshing = false; busy = false; inputWhileBusy = false
        locallyEdited = false; validated = nil
        rebuildFields(); updateAvailability(); message.show(ModellingText.text("preview_required"))
    }
    func contextRefreshDiscarded() {
        contextRefreshing = false; validated = nil; busy = false; inputWhileBusy = false
        editors.forEach { $0.setBusy(false) }
        updateAvailability()
        message.show(ModellingText.text("preview_required"))
    }
    func failed(_ error: BridgeError) {
        busy = false; committing = false; inputWhileBusy = false; contextRefreshing = false; validated = nil
        editors.forEach { $0.setBusy(false) }
        updateAvailability(); message.show(error.message, error: true)
    }
    func setBusy(committing: Bool, allowInput: Bool = false) {
        busy = true; self.committing = committing; inputWhileBusy = allowInput && !committing
        editors.forEach { $0.setBusy(!inputWhileBusy, committing: committing) }
        updateAvailability(); message.show(NativeText.value("ui.ipados.updating"), busy: true)
    }
    func invalidatePreview() {
        validated = nil; updateAvailability()
        if !busy { message.show(ModellingText.text("preview_required")) }
    }
    private func updateAvailability() {
        preview.availability(enabled: !busy && !contextRefreshing)
        apply.availability(enabled: !busy && !contextRefreshing && validated == params, reason: validated == params ? nil : ModellingText.text("preview_required"))
        cancel.availability(enabled: !committing)
        select.availability(enabled: !busy)
        fieldViews.forEach { $0.isUserInteractionEnabled = !busy || inputWhileBusy }
        sliders.forEach { $0.isUserInteractionEnabled = !contextRefreshing && (!busy || inputWhileBusy) }
    }
    private func changed() {
        locallyEdited = true
        invalidatePreview(); onChange?()
    }
    private func parameter(_ path: String) -> PresentationJSON? { PresentationJSON.parameter(path, in: params) }
    private func setParameter(_ path: String, value: PresentationJSON) { params = PresentationJSON.setting(path, value: value, in: params) }
    private func display(_ value: PresentationJSON) -> String {
        if case .string(let text) = value { return text }
        guard JSONSerialization.isValidJSONObject([value.value]),
              let data = try? JSONSerialization.data(withJSONObject: [value.value], options: [.sortedKeys]),
              let result = String(data: data, encoding: .utf8), result.count >= 2 else { return "—" }
        return String(result.dropFirst().dropLast())
    }
    private func rebuildFields() {
        fieldsStack.arrangedSubviews.forEach { fieldsStack.removeArrangedSubview($0); $0.removeFromSuperview() }
        editors.removeAll(); textFields.removeAll(); fieldViews.removeAll(); sliders.removeAll()
        for field in fields {
            let heading = WorkspaceStyle.label(.body)
            heading.text = ModellingText.label(field.label)
            fieldsStack.addArrangedSubview(heading)
            if field.kind == "text" {
                let input = UITextField()
                input.text = parameter(field.key).map(display) ?? ""
                input.borderStyle = .roundedRect; input.autocorrectionType = .no; input.autocapitalizationType = .none
                input.font = .preferredFont(forTextStyle: .body); input.adjustsFontForContentSizeCategory = true
                input.heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
                input.accessibilityLabel = ModellingText.label(field.label)
                input.accessibilityIdentifier = "workspace-model-field-" + field.key
                input.addAction(UIAction { [weak self, weak input] _ in self?.setParameter(field.key, value: .string(input?.text ?? "")); self?.changed() }, for: .editingChanged)
                input.addAction(UIAction { [weak self] _ in self?.onFocus?() }, for: .editingDidBegin)
                textFields.append(input); fieldViews.append(input); fieldsStack.addArrangedSubview(input)
            } else if field.kind == "expression" || field.kind == "slider" {
                let expression = parameter(field.key).map(display) ?? ""
                let editor = NumericExpressionEditor(expression: expression, units: field.units ?? "", showsActions: false)
                editor.field.accessibilityIdentifier = "workspace-model-field-" + field.key
                editor.onChange = { [weak self, weak editor] in
                    guard let self, let editor else { return }
                    self.setParameter(field.key, value: .string(editor.field.text ?? "")); self.changed()
                }
                editor.onFocus = { [weak self] in self?.onFocus?() }
                editors.append(editor); fieldViews.append(editor); fieldsStack.addArrangedSubview(editor)
                if field.kind == "slider" { buildSlider(field, editor: editor) }
            } else if field.kind == "references" {
                buildReferences(field)
            } else {
                let selected = parameter(field.key)
                let title = field.options?.first(where: { $0.value == selected }).map { ModellingText.label($0.label) }
                    ?? selected.map(display) ?? ModellingText.text("choose")
                let button = choiceButton(title, field: field) { [weak self] value in
                    guard let self else { return }
                    self.setParameter(field.key, value: value); self.changed(); self.rebuildFields()
                    self.onContextChange?(self.params)
                }
                fieldsStack.addArrangedSubview(button); fieldViews.append(button)
                if field.options?.isEmpty != false {
                    let hint = WorkspaceStyle.label(.footnote, secondary: true); hint.text = ModellingText.text("no_inputs"); fieldsStack.addArrangedSubview(hint)
                }
            }
        }
    }
    private func choiceButton(_ title: String, field: ModellingState.Field, action: @escaping (PresentationJSON) -> Void) -> WorkspaceButton {
        let button = WorkspaceButton(title, action: {})
        button.menu = UIMenu(children: (field.options ?? []).map { option in
            UIAction(title: ModellingText.label(option.label)) { _ in action(option.value) }
        })
        button.showsMenuAsPrimaryAction = true
        button.accessibilityIdentifier = "workspace-model-field-" + field.key
        if field.options?.isEmpty != false { button.availability(enabled: false, reason: ModellingText.text("no_inputs")) }
        return button
    }
    private func buildSlider(_ field: ModellingState.Field, editor: NumericExpressionEditor) {
        guard let minimum = field.min, let maximum = field.max,
              minimum.isFinite, maximum.isFinite, minimum < maximum,
              abs(minimum) < Double(Float.greatestFiniteMagnitude), abs(maximum) < Double(Float.greatestFiniteMagnitude) else { return }
        let resolved: Double?
        switch parameter(field.key) {
        case .number(let value): resolved = value
        case .integer(let value): resolved = Double(value)
        case .unsignedInteger(let value): resolved = Double(value)
        default: resolved = field.value
        }
        guard let resolved, resolved.isFinite, abs(resolved) < Double(Float.greatestFiniteMagnitude) else { return }
        let slider = UISlider()
        slider.minimumValue = Float(minimum); slider.maximumValue = Float(maximum)
        slider.value = Float(resolved); slider.isContinuous = true
        slider.accessibilityLabel = ModellingText.label(field.label)
        slider.accessibilityIdentifier = "workspace-model-slider-" + field.key
        slider.heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        var start = parameter(field.key)
        slider.addAction(UIAction { [weak self] _ in
            guard let self else { return }; start = self.parameter(field.key); self.sliderActive = true
        }, for: .touchDown)
        slider.addAction(UIAction { [weak self, weak slider, weak editor] _ in
            guard let self, let slider else { return }
            let number = String(Double(slider.value))
            let units = field.units ?? ""
            let expression = number + (units.isEmpty ? "" : " " + units)
            self.setParameter(field.key, value: units.isEmpty ? .number(Double(slider.value)) : .string(expression))
            editor?.field.text = expression; self.changed(); self.onLivePreview?(self.params)
            slider.accessibilityValue = expression
        }, for: .valueChanged)
        slider.addAction(UIAction { [weak self] _ in
            guard let self else { return }; self.sliderActive = false; self.onLivePreview?(self.params)
        }, for: [.touchUpInside, .touchUpOutside])
        slider.addAction(UIAction { [weak self, weak editor] _ in
            guard let self else { return }; self.sliderActive = false
            self.setParameter(field.key, value: start ?? .null)
            editor?.field.text = start.map(self.display) ?? ""
            self.changed(); self.onLivePreview?(self.params)
        }, for: .touchCancel)
        fieldsStack.addArrangedSubview(slider); fieldViews.append(slider); sliders.append(slider)
        let range = WorkspaceStyle.label(.footnote, secondary: true)
        range.text = String(minimum) + " … " + String(maximum) + " " + (field.units ?? "")
        fieldsStack.addArrangedSubview(range)
    }
    private func buildReferences(_ field: ModellingState.Field) {
        let values: [PresentationJSON]
        if case .array(let current) = parameter(field.key) { values = current } else { values = [] }
        if values.isEmpty {
            let empty = WorkspaceStyle.label(.footnote, secondary: true); empty.text = ModellingText.text("no_selection")
            fieldsStack.addArrangedSubview(empty)
        }
        for (index, value) in values.enumerated() {
            let title = field.options?.first(where: { $0.value == value }).map { ModellingText.label($0.label) } ?? display(value)
            fieldsStack.addArrangedSubview(WorkspacePropertyRow(label: String(index + 1), value: title))
            let remove = WorkspaceButton(ModellingText.text("remove")) { [weak self] in
                var next = values; next.remove(at: index); self?.setParameter(field.key, value: .array(next)); self?.changed(); self?.rebuildFields()
            }
            let up = WorkspaceButton(ModellingText.text("move_up")) { [weak self] in
                guard index > 0 else { return }; var next = values; next.swapAt(index, index - 1)
                self?.setParameter(field.key, value: .array(next)); self?.changed(); self?.rebuildFields()
            }
            let down = WorkspaceButton(ModellingText.text("move_down")) { [weak self] in
                guard index + 1 < values.count else { return }; var next = values; next.swapAt(index, index + 1)
                self?.setParameter(field.key, value: .array(next)); self?.changed(); self?.rebuildFields()
            }
            up.availability(enabled: index > 0); down.availability(enabled: index + 1 < values.count)
            let actions = UIStackView(arrangedSubviews: [remove, up, down]); actions.axis = .horizontal; actions.spacing = 4; actions.distribution = .fillEqually
            fieldsStack.addArrangedSubview(actions); fieldViews.append(actions)
        }
        let add = choiceButton(ModellingText.text("add_input"), field: field) { [weak self] value in
            guard !values.contains(value) else { return }
            self?.setParameter(field.key, value: .array(values + [value])); self?.changed(); self?.rebuildFields()
        }
        fieldsStack.addArrangedSubview(add); fieldViews.append(add)
        if field.options?.isEmpty != false {
            let hint = WorkspaceStyle.label(.footnote, secondary: true); hint.text = ModellingText.text("no_inputs"); fieldsStack.addArrangedSubview(hint)
        }
    }
}
