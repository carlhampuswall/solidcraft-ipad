import UIKit

enum ProjectionText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.projection." + key) }
}

/// Thin source/geometry chooser. The overview uses engine-authored samples only.
final class SketchProjectionController: UITableViewController {
    private let sources: [SketchProjectionSource]
    private(set) var selection: SketchProjectionSelection
    private let document: String
    private let revision: UInt64
    private let sketch: UInt64
    private let overview = SketchProjectionOverview()
    private var busy = false
    private var failure: String?
    var onApply: (([String: Any], @escaping (Result<Void, BridgeError>) -> Void) -> Void)?
    private lazy var applyButton = UIBarButtonItem(title: ProjectionText.text("project"), style: .done, target: self, action: #selector(apply))

    init(sources: [SketchProjectionSource], source: SketchProjectionSource, document: String, revision: UInt64, sketch: UInt64) {
        self.sources = sources; selection = SketchProjectionSelection(source: source)
        self.document = document; self.revision = revision; self.sketch = sketch
        super.init(style: .insetGrouped)
        title = ProjectionText.text("title")
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad()
        view.accessibilityIdentifier = "workspace-sketch-projection"
        navigationItem.leftBarButtonItem = UIBarButtonItem(systemItem: .cancel, primaryAction: UIAction { [weak self] _ in self?.dismiss(animated: true) })
        navigationItem.rightBarButtonItem = applyButton
        tableView.allowsSelection = true
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 52
        overview.frame = CGRect(x: 0, y: 0, width: 400, height: 180)
        overview.accessibilityLabel = ProjectionText.text("overview")
        overview.accessibilityIdentifier = "projection-overview"
        overview.onChoose = { [weak self] id in self?.toggle(id) }
        tableView.tableHeaderView = overview
        update()
    }
    override func numberOfSections(in tableView: UITableView) -> Int { 3 }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int {
        section == 0 ? sources.count : section == 1 ? selection.source.entities.count + 1 : 2
    }
    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        ProjectionText.text(section == 0 ? "source" : section == 1 ? "geometry" : "options")
    }
    override func tableView(_ tableView: UITableView, titleForFooterInSection section: Int) -> String? {
        if section == 0 { return ProjectionText.text("source_help") }
        if section == 1 { return ProjectionText.text("geometry_help") }
        return failure ?? ProjectionText.text("options_help")
    }
    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = UITableViewCell(style: .default, reuseIdentifier: nil)
        var content = cell.defaultContentConfiguration()
        if indexPath.section == 0 {
            let source = sources[indexPath.row]
            content.text = source.name
            cell.accessoryType = selection.source.id == source.id ? .checkmark : .none
            cell.accessibilityIdentifier = "projection-source-\(source.id)"
        } else if indexPath.section == 1 {
            if indexPath.row == 0 {
                content.text = ProjectionText.text("all")
                cell.accessoryType = selection.all ? .checkmark : .none
                cell.accessibilityIdentifier = "projection-all"
            } else {
                let entity = selection.source.entities[indexPath.row - 1]
                content.text = ProjectionText.text(entity.type) + " · " + entity.id
                cell.accessoryType = selection.selected.contains(entity.id) ? .checkmark : .none
                cell.accessibilityIdentifier = "projection-entity-" + entity.id
            }
        } else {
            let construction = indexPath.row == 0
            content.text = ProjectionText.text(construction ? "construction" : "linked")
            let control = UISwitch()
            control.isOn = construction ? selection.construction : selection.linked
            control.isEnabled = !busy
            control.accessibilityLabel = content.text
            control.accessibilityIdentifier = construction ? "projection-construction" : "projection-linked"
            control.addAction(UIAction { [weak self, weak control] _ in
                guard let self, let control, !self.busy else { return }
                if construction { self.selection.construction = control.isOn } else { self.selection.linked = control.isOn }
            }, for: .valueChanged)
            cell.accessoryView = control; cell.selectionStyle = .none
        }
        if cell.accessoryType == .checkmark { cell.accessibilityTraits.insert(.selected) }
        content.textProperties.numberOfLines = 0
        cell.contentConfiguration = content
        return cell
    }
    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        guard !busy else { return }
        if indexPath.section == 0 { selection.choose(sources[indexPath.row]); update() }
        else if indexPath.section == 1 {
            if indexPath.row == 0 { selection.toggleAll(); update() }
            else { toggle(selection.source.entities[indexPath.row - 1].id) }
        }
    }
    private func toggle(_ id: String) {
        guard !busy else { return }
        selection.toggle(id); update()
    }
    private func update() {
        overview.source = selection.source; overview.selected = selection.selected; overview.setNeedsDisplay()
        applyButton.isEnabled = !busy && !selection.selected.isEmpty
        navigationItem.leftBarButtonItem?.isEnabled = !busy
        isModalInPresentation = busy
        tableView.isUserInteractionEnabled = !busy
        tableView.reloadData()
    }
    @objc private func apply() {
        guard !busy, !selection.selected.isEmpty, let onApply else { return }
        busy = true; failure = nil; update()
        onApply(selection.parameters(document: document, revision: revision, sketch: sketch)) { [weak self] result in
            guard let self else { return }
            self.busy = false
            switch result {
            case .success: self.dismiss(animated: true)
            case .failure(let error): self.failure = error.message; self.update()
            }
        }
    }
}

private final class SketchProjectionOverview: UIView {
    var source: SketchProjectionSource?
    var selected: Set<String> = []
    var onChoose: ((String) -> Void)?
    override init(frame: CGRect) {
        super.init(frame: frame)
        backgroundColor = .secondarySystemGroupedBackground
        addGestureRecognizer(UITapGestureRecognizer(target: self, action: #selector(pick(_:))))
        isAccessibilityElement = true
    }
    required init?(coder: NSCoder) { nil }
    private func paths() -> [(String, [CGPoint])] {
        guard let source else { return [] }
        let lines = source.entities.map { entity in (entity.id, entity.polyline.compactMap { values -> CGPoint? in
            guard values.count == 2, values[0].isFinite, values[1].isFinite else { return nil }
            return CGPoint(x: values[0], y: values[1])
        }) }
        let points = lines.flatMap { $0.1 }
        guard let minX = points.map(\.x).min(), let maxX = points.map(\.x).max(),
              let minY = points.map(\.y).min(), let maxY = points.map(\.y).max() else { return [] }
        let scale = min(max(1, bounds.width - 40) / max(1, maxX - minX), max(1, bounds.height - 30) / max(1, maxY - minY))
        return lines.map { id, points in (id, points.map {
            CGPoint(x: bounds.midX + ($0.x - (minX + maxX) / 2) * scale, y: bounds.midY - ($0.y - (minY + maxY) / 2) * scale)
        }) }
    }
    override func draw(_ rect: CGRect) {
        for (id, points) in paths() {
            guard let first = points.first else { continue }
            let path = UIBezierPath()
            if points.count == 1 { path.append(UIBezierPath(ovalIn: CGRect(x: first.x - 3, y: first.y - 3, width: 6, height: 6))) }
            else { path.move(to: first); for point in points.dropFirst() { path.addLine(to: point) } }
            (selected.contains(id) ? (tintColor ?? .systemBlue) : UIColor.secondaryLabel).setStroke()
            path.lineWidth = selected.contains(id) ? 3 : 1.5; path.stroke()
        }
    }
    @objc private func pick(_ gesture: UITapGestureRecognizer) {
        let point = gesture.location(in: self)
        var best: (String, CGFloat)?
        for (id, points) in paths() {
            for (a, b) in zip(points, points.count == 1 ? points : Array(points.dropFirst())) {
                let dx = b.x - a.x, dy = b.y - a.y
                let length = dx * dx + dy * dy
                let t = length > 0 ? min(1, max(0, ((point.x - a.x) * dx + (point.y - a.y) * dy) / length)) : 0
                let distance = hypot(point.x - a.x - t * dx, point.y - a.y - t * dy)
                if distance < 18 && distance < (best?.1 ?? .greatestFiniteMagnitude) { best = (id, distance) }
            }
        }
        if let best { onChoose?(best.0) }
    }
}
