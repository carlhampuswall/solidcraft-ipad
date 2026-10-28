import UIKit
import UniformTypeIdentifiers

enum ProjectText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.projects." + key) }
}

/// A native library of design files, independent of the model's component tree.
/// Callbacks reach the one serial Rust session; the browser owns no CAD state.
final class ProjectBrowserController: UITableViewController, UISearchResultsUpdating {
    var onOpen: ((ProjectRecord) -> Void)?
    var onCreate: ((String, ProjectRecord?) -> Void)?
    var onImport: (() -> Void)?
    var onDelete: ((ProjectRecord) -> Void)?
    var onClose: (() -> Void)?
    private var projects: [ProjectRecord] = []
    private var filtered: [ProjectRecord] = []
    private var current: UUID?
    private let search = UISearchController(searchResultsController: nil)
    private let message = UILabel()
    private var busy = false

    init() { super.init(style: .insetGrouped) }
    required init?(coder: NSCoder) { super.init(coder: coder) }

    override func viewDidLoad() {
        super.viewDidLoad()
        title = ProjectText.text("title")
        view.backgroundColor = WorkspaceStyle.background
        tableView.accessibilityIdentifier = "project-browser"
        navigationItem.leftBarButtonItem = UIBarButtonItem(title: WorkspaceStyle.text("close"), style: .plain, target: self, action: #selector(close))
        navigationItem.rightBarButtonItems = [
            UIBarButtonItem(title: ProjectText.text("new"), style: .plain, target: self, action: #selector(newProject)),
            UIBarButtonItem(title: ProjectText.text("import"), style: .plain, target: self, action: #selector(importFile))
        ]
        search.searchResultsUpdater = self
        search.obscuresBackgroundDuringPresentation = false
        search.searchBar.placeholder = ProjectText.text("search")
        navigationItem.searchController = search
        navigationItem.hidesSearchBarWhenScrolling = false
        message.numberOfLines = 0; message.textAlignment = .center
        message.font = .preferredFont(forTextStyle: .body); message.adjustsFontForContentSizeCategory = true
        message.textColor = .secondaryLabel
        tableView.backgroundView = message
        updateSearchResults(for: search)
    }

    func display(_ projects: [ProjectRecord], current: UUID?) {
        self.projects = projects; self.current = current
        loadViewIfNeeded(); updateSearchResults(for: search)
    }

    func setBusy(_ busy: Bool) {
        self.busy = busy
        isModalInPresentation = busy
        navigationItem.leftBarButtonItem?.isEnabled = !busy
        navigationItem.rightBarButtonItems?.forEach { $0.isEnabled = !busy }
        tableView.allowsSelection = !busy
        search.searchBar.isUserInteractionEnabled = !busy
    }

    func showError(_ error: BridgeError) {
        setBusy(false)
        let alert = UIAlertController(title: ProjectText.text("error"), message: error.description, preferredStyle: .alert)
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("close"), style: .default))
        present(alert, animated: true)
    }

    func updateSearchResults(for searchController: UISearchController) {
        let query = searchController.searchBar.text?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        filtered = projects.filter { query.isEmpty || $0.name.localizedCaseInsensitiveContains(query) }
        message.text = ProjectText.text(projects.isEmpty ? "empty" : "no_matches")
        message.isHidden = !filtered.isEmpty
        tableView.reloadData()
    }

    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { filtered.count }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "project") ?? UITableViewCell(style: .subtitle, reuseIdentifier: "project")
        guard filtered.indices.contains(indexPath.row) else { return cell }
        let project = filtered[indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = project.name
        content.secondaryText = DateFormatter.localizedString(from: project.modified, dateStyle: .medium, timeStyle: .short)
        content.image = WorkspaceIcon.image("folder", traits: traitCollection)
        cell.contentConfiguration = content
        cell.accessoryType = project.id == current ? .checkmark : .disclosureIndicator
        cell.accessibilityIdentifier = "project-" + project.id.uuidString
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        guard !busy, filtered.indices.contains(indexPath.row) else { return }
        onOpen?(filtered[indexPath.row])
    }

    override func tableView(_ tableView: UITableView, contextMenuConfigurationForRowAt indexPath: IndexPath, point: CGPoint) -> UIContextMenuConfiguration? {
        guard !busy, filtered.indices.contains(indexPath.row) else { return nil }
        let project = filtered[indexPath.row]
        return UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { [weak self] _ in
            guard let self else { return nil }
            let duplicate = UIAction(title: ProjectText.text("duplicate")) { [weak self] _ in self?.askName(copying: project) }
            let delete = UIAction(title: ProjectText.text("delete"), attributes: project.id == self.current ? [.destructive, .disabled] : [.destructive]) { [weak self] _ in
                guard let self else { return }
                let alert = UIAlertController(title: ProjectText.text("delete"), message: ProjectText.text("delete_warning").replacingOccurrences(of: "{name}", with: project.name), preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
                alert.addAction(UIAlertAction(title: ProjectText.text("delete"), style: .destructive) { [weak self] _ in self?.onDelete?(project) })
                self.present(alert, animated: true)
            }
            return UIMenu(children: [duplicate, delete])
        }
    }

    @objc private func close() { onClose?() }
    @objc private func newProject() { askName(copying: nil) }
    @objc private func importFile() { onImport?() }

    private func askName(copying source: ProjectRecord?) {
        let alert = UIAlertController(title: ProjectText.text(source == nil ? "new" : "duplicate"), message: ProjectText.text("invalid_name"), preferredStyle: .alert)
        alert.addTextField { field in
            field.placeholder = ProjectText.text("name")
            field.text = source.map { ProjectText.text("copy_name").replacingOccurrences(of: "{name}", with: $0.name) }
            field.autocapitalizationType = .sentences
            field.accessibilityIdentifier = "project-name"
        }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        let create = UIAlertAction(title: ProjectText.text("create"), style: .default) { [weak self, weak alert] _ in
            guard let self else { return }
            do { self.onCreate?(try ProjectLibrary.validName(alert?.textFields?.first?.text ?? ""), source) }
            catch { self.showError(BridgeError(code: "project", message: error.localizedDescription)) }
        }
        create.isEnabled = (try? ProjectLibrary.validName(alert.textFields?.first?.text ?? "")) != nil
        alert.textFields?.first?.addAction(UIAction { [weak create] action in
            guard let field = action.sender as? UITextField else { return }
            create?.isEnabled = (try? ProjectLibrary.validName(field.text ?? "")) != nil
        }, for: .editingChanged)
        alert.addAction(create)
        present(alert, animated: true)
    }
}
