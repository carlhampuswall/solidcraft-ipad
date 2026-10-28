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

/// A text-labelled touch target; no essential meaning depends on hover or artwork.
final class WorkspaceButton: UIButton {
    init(_ title: String, prominent: Bool = false, action: @escaping () -> Void) {
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
        addAction(UIAction { _ in action() }, for: .touchUpInside)
    }
    required init?(coder: NSCoder) { nil }

    func availability(enabled: Bool, reason: String? = nil) {
        isEnabled = enabled
        accessibilityHint = reason
        accessibilityValue = reason
        toolTip = reason
    }
    func active(_ value: Bool) {
        isSelected = value
        var config = configuration
        config?.baseBackgroundColor = value ? WorkspaceStyle.accent : .clear
        config?.baseForegroundColor = value ? WorkspaceStyle.selectedForeground : WorkspaceStyle.foreground
        if value { config?.background.backgroundColor = WorkspaceStyle.accent }
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

    init(expression: String, units: String, showsActions: Bool = true) {
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true, action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        super.init(frame: .zero)
        axis = .vertical; spacing = 12
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
        for text in ["−", ".", "(", ")", "+", "*", "/", "mm"] {
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

    init(state: ModellingState) {
        params = state.params ?? [:]
        fields = state.fields ?? []
        preview = WorkspaceButton(WorkspaceStyle.text("preview"), action: {})
        apply = WorkspaceButton(WorkspaceStyle.text("apply"), prominent: true, action: {})
        cancel = WorkspaceButton(WorkspaceStyle.text("cancel"), action: {})
        select = WorkspaceButton(ModellingText.text("select_in_viewport"), action: {})
        super.init(frame: .zero)
        axis = .vertical; spacing = 16
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
