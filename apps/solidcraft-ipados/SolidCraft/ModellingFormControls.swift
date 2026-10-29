import UIKit

/// Native form chrome only. The descriptor and opaque values remain Rust-owned.
enum ModellingFormText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.modelling_form." + key) }
    static func named(_ key: String, name: String) -> String { text(key).replacingOccurrences(of: "{name}", with: name) }
    static func choice(_ label: String) -> String {
        switch label {
        case "positive", "negative", "symmetric": return text(label)
        default: return ModellingText.label(label)
        }
    }
}

/// An inset group of related fields, with separators instead of repeated cards.
final class ModellingFormGroup: UIStackView {
    private var controls: [(control: UIControl, available: Bool)] = []
    override init(frame: CGRect) {
        super.init(frame: frame)
        axis = .vertical; spacing = 0
        backgroundColor = .tertiarySystemBackground
        layer.cornerRadius = 12; clipsToBounds = true
    }
    convenience init() { self.init(frame: .zero) }
    required init(coder: NSCoder) { super.init(coder: coder) }

    func setEditingEnabled(_ enabled: Bool) {
        isUserInteractionEnabled = enabled
        controls.forEach { $0.control.isEnabled = enabled && $0.available }
    }

    func append(_ row: UIView) {
        func register(_ view: UIView) {
            if let control = view as? UIControl {
                controls.append((control, control.isEnabled))
                control.isEnabled = isUserInteractionEnabled && control.isEnabled
            }
            view.subviews.forEach(register)
        }
        register(row)
        if !arrangedSubviews.isEmpty {
            let separator = UIView()
            let line = UIView(); line.backgroundColor = WorkspaceStyle.separator
            line.translatesAutoresizingMaskIntoConstraints = false; separator.addSubview(line)
            NSLayoutConstraint.activate([
                separator.heightAnchor.constraint(equalToConstant: 0.5),
                line.leadingAnchor.constraint(equalTo: separator.leadingAnchor, constant: 14),
                line.trailingAnchor.constraint(equalTo: separator.trailingAnchor, constant: -14),
                line.topAnchor.constraint(equalTo: separator.topAnchor),
                line.bottomAnchor.constraint(equalTo: separator.bottomAnchor)
            ])
            addArrangedSubview(separator)
        }
        addArrangedSubview(row)
    }
}

/// Labels and controls share a row at normal type sizes and stack for large text.
final class ModellingFormRow: UIStackView {
    private let heading = WorkspaceStyle.label(.body)
    init(title: String, control: UIView) {
        super.init(frame: .zero)
        heading.text = title; heading.isAccessibilityElement = false
        heading.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        control.setContentHuggingPriority(.required, for: .horizontal)
        control.setContentCompressionResistancePriority(control is UISwitch ? .required : .defaultHigh, for: .horizontal)
        control.widthAnchor.constraint(lessThanOrEqualTo: widthAnchor, constant: -28).isActive = true
        isLayoutMarginsRelativeArrangement = true
        directionalLayoutMargins = NSDirectionalEdgeInsets(top: 5, leading: 14, bottom: 5, trailing: 10)
        heightAnchor.constraint(greaterThanOrEqualToConstant: 54).isActive = true
        addArrangedSubview(heading); addArrangedSubview(control)
        updateLayout()
        registerForTraitChanges([UITraitPreferredContentSizeCategory.self]) { (row: ModellingFormRow, _: UITraitCollection) in row.updateLayout() }
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
    private func updateLayout() {
        let large = traitCollection.preferredContentSizeCategory.isAccessibilityCategory
        axis = large ? .vertical : .horizontal
        alignment = large ? .leading : .center
        spacing = large ? 4 : 12
    }
}

/// A native menu button; no editable text or text-selection gesture recognizer.
final class ModellingFormButton: UIButton {
    init(title: String, symbol: String, label: String, identifier: String, accent: Bool = false, iconOnly: Bool = false) {
        super.init(frame: .zero)
        var style = UIButton.Configuration.plain()
        style.title = iconOnly ? nil : title
        style.image = UIImage(systemName: symbol)
        style.imagePlacement = iconOnly || accent ? .leading : .trailing
        style.imagePadding = 8
        style.preferredSymbolConfigurationForImage = UIImage.SymbolConfiguration(pointSize: 15, weight: .medium)
        style.baseForegroundColor = accent ? WorkspaceStyle.accent : WorkspaceStyle.foreground
        style.contentInsets = NSDirectionalEdgeInsets(top: 8, leading: 8, bottom: 8, trailing: 8)
        style.titleTextAttributesTransformer = UIConfigurationTextAttributesTransformer { original in
            var attributes = original; attributes.font = .preferredFont(forTextStyle: .body); return attributes
        }
        configuration = style
        titleLabel?.adjustsFontForContentSizeCategory = true
        titleLabel?.numberOfLines = 0
        contentHorizontalAlignment = accent ? .center : .trailing
        heightAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        widthAnchor.constraint(greaterThanOrEqualToConstant: WorkspaceStyle.touch).isActive = true
        if iconOnly { widthAnchor.constraint(equalToConstant: WorkspaceStyle.touch).isActive = true }
        accessibilityLabel = label; accessibilityIdentifier = identifier
        toolTip = label; isPointerInteractionEnabled = true
        showsLargeContentViewer = true; largeContentTitle = label; largeContentImage = style.image
        addInteraction(UILargeContentViewerInteraction())
    }
    required init?(coder: NSCoder) { nil }
}

/// Expands the native switch's 31-point visual height to a 44-point touch target.
final class ModellingFormSwitch: UISwitch {
    override func point(inside point: CGPoint, with event: UIEvent?) -> Bool {
        bounds.insetBy(dx: 0, dy: -max(0, (WorkspaceStyle.touch - bounds.height) / 2)).contains(point)
    }
}

final class ModellingReferenceRow: UIStackView {
    init(title: String, index: Int, count: Int, body: Bool, identifier: String,
         moveActions: [UIAction], remove: @escaping () -> Void) {
        super.init(frame: .zero)
        axis = .horizontal; alignment = .center; spacing = 6
        isLayoutMarginsRelativeArrangement = true
        directionalLayoutMargins = NSDirectionalEdgeInsets(top: 5, leading: 14, bottom: 5, trailing: 4)
        heightAnchor.constraint(greaterThanOrEqualToConstant: 54).isActive = true
        if count > 1 {
            let order = WorkspaceStyle.label(.caption1, secondary: true)
            order.text = String(index + 1); order.isAccessibilityElement = false
            order.setContentHuggingPriority(.required, for: .horizontal)
            addArrangedSubview(order)
        }
        if body {
            let icon = UIImageView(image: UIImage(systemName: "cube"))
            icon.tintColor = WorkspaceStyle.secondary; icon.contentMode = .scaleAspectFit
            icon.isAccessibilityElement = false
            NSLayoutConstraint.activate([icon.widthAnchor.constraint(equalToConstant: 20), icon.heightAnchor.constraint(equalToConstant: 20)])
            addArrangedSubview(icon)
        }
        let name = WorkspaceStyle.label(.body)
        name.text = title; name.numberOfLines = 0
        name.setContentCompressionResistancePriority(.defaultLow, for: .horizontal)
        name.accessibilityIdentifier = identifier + "-name"
        addArrangedSubview(name)
        if !moveActions.isEmpty {
            let reorder = ModellingFormButton(title: "", symbol: "ellipsis", label: ModellingFormText.named("reorder", name: title),
                                              identifier: identifier + "-reorder", iconOnly: true)
            reorder.menu = UIMenu(children: moveActions); reorder.showsMenuAsPrimaryAction = true
            addArrangedSubview(reorder)
        }
        let removeButton = ModellingFormButton(title: "", symbol: "xmark", label: ModellingFormText.named("remove_selection", name: title),
                                               identifier: identifier + "-remove", iconOnly: true)
        removeButton.addAction(UIAction { _ in remove() }, for: .primaryActionTriggered)
        addArrangedSubview(removeButton)
    }
    required init(coder: NSCoder) { super.init(coder: coder) }
}
