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
    var onExport: ((String) -> Void)?
    var onShare: (() -> Void)?
    var onRename: ((ProjectRecord, String) -> Void)?
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
        navigationItem.rightBarButtonItems?.first?.accessibilityIdentifier = "project-new"
        navigationItem.rightBarButtonItems?.last?.accessibilityIdentifier = "project-import"
        let export = UIBarButtonItem(title: ProjectText.text("export"), menu: UIMenu(children: ["step", "solidcraft", "stl", "obj", "3mf"].map { format in
            UIAction(title: ProjectText.text("export_" + (format == "solidcraft" ? "native" : format))) { [weak self] _ in
                guard let self, !self.busy, self.current != nil else { return }
                self.onExport?(format)
            }
        }))
        export.accessibilityIdentifier = "project-export"
        let share = UIBarButtonItem(title: ProjectText.text("share_step"), style: .plain, target: self, action: #selector(shareFile))
        share.accessibilityIdentifier = "project-share"
        toolbarItems = [export, UIBarButtonItem(systemItem: .flexibleSpace), share]
        navigationController?.setToolbarHidden(false, animated: false)
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
        updateActions()
    }

    func setBusy(_ busy: Bool) {
        self.busy = busy
        isModalInPresentation = busy
        updateActions()
        tableView.allowsSelection = !busy
        search.searchBar.isUserInteractionEnabled = !busy
    }

    private func updateActions() {
        navigationItem.leftBarButtonItem?.isEnabled = !busy
        navigationItem.rightBarButtonItems?.forEach { $0.isEnabled = !busy }
        toolbarItems?.forEach { $0.isEnabled = !busy && current != nil }
    }

    func showError(_ error: BridgeError, openForRepair: (() -> Void)? = nil) {
        setBusy(false)
        let message = openForRepair == nil ? error.description : ProjectText.text("repair_hint") + "\n\n" + error.description
        let alert = UIAlertController(title: ProjectText.text("error"), message: message, preferredStyle: .alert)
        if let openForRepair {
            alert.addAction(UIAlertAction(title: ProjectText.text("open_for_repair"), style: .default) { _ in openForRepair() })
        }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("close"), style: openForRepair == nil ? .default : .cancel))
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

    override func tableView(_ tableView: UITableView, titleForFooterInSection section: Int) -> String? {
        guard let project = projects.first(where: { $0.id == current }) else { return nil }
        return ProjectText.text("current_actions").replacingOccurrences(of: "{name}", with: project.name)
    }

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
            let rename = UIAction(title: ProjectText.text("rename")) { [weak self] _ in self?.askName(renaming: project) }
            let duplicate = UIAction(title: ProjectText.text("duplicate")) { [weak self] _ in self?.askName(copying: project) }
            let delete = UIAction(title: ProjectText.text("delete"), attributes: project.id == self.current ? [.destructive, .disabled] : [.destructive]) { [weak self] _ in
                guard let self else { return }
                let alert = UIAlertController(title: ProjectText.text("delete"), message: ProjectText.text("delete_warning").replacingOccurrences(of: "{name}", with: project.name), preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
                alert.addAction(UIAlertAction(title: ProjectText.text("delete"), style: .destructive) { [weak self] _ in self?.onDelete?(project) })
                self.present(alert, animated: true)
            }
            return UIMenu(children: [rename, duplicate, delete])
        }
    }

    override func tableView(_ tableView: UITableView, trailingSwipeActionsConfigurationForRowAt indexPath: IndexPath) -> UISwipeActionsConfiguration? {
        guard !busy, filtered.indices.contains(indexPath.row) else { return nil }
        let project = filtered[indexPath.row]
        let rename = UIContextualAction(style: .normal, title: ProjectText.text("rename")) { [weak self] _, _, completion in
            completion(true)
            self?.askName(renaming: project)
        }
        let configuration = UISwipeActionsConfiguration(actions: [rename])
        configuration.performsFirstActionWithFullSwipe = false
        return configuration
    }

    @objc private func close() { onClose?() }
    @objc private func newProject() { askName(copying: nil) }
    @objc private func importFile() { guard !busy else { return }; onImport?() }
    @objc private func shareFile() { guard !busy, current != nil else { return }; onShare?() }

    private func askName(copying source: ProjectRecord? = nil, renaming project: ProjectRecord? = nil) {
        guard !busy else { return }
        let alert = UIAlertController(title: ProjectText.text(project != nil ? "rename" : (source == nil ? "new" : "duplicate")), message: ProjectText.text("invalid_name"), preferredStyle: .alert)
        alert.addTextField { field in
            field.placeholder = ProjectText.text("name")
            field.text = project?.name ?? source.map { ProjectText.text("copy_name").replacingOccurrences(of: "{name}", with: $0.name) }
            field.autocapitalizationType = .sentences
            field.accessibilityIdentifier = "project-name"
        }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        let create = UIAlertAction(title: ProjectText.text(project == nil ? "create" : "rename"), style: .default) { [weak self, weak alert] _ in
            guard let self else { return }
            do {
                let name = try ProjectLibrary.validName(alert?.textFields?.first?.text ?? "")
                if let project { self.onRename?(project, name) }
                else { self.onCreate?(name, source) }
            }
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
