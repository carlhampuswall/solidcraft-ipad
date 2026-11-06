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
    var onOrganise: ((ProjectLibraryChange) -> Void)?
    var onImport: (() -> Void)?
    var onInsert: ((ProjectRecord) -> Void)?
    var onExport: ((String) -> Void)?
    var onShare: (() -> Void)?
    var onRename: ((ProjectRecord, String) -> Void)?
    var onDelete: ((ProjectRecord) -> Void)?
    var onClose: (() -> Void)?
    private var projects: [ProjectRecord] = []
    private var filtered: [ProjectRecord] = []
    private var folders: [ProjectFolder] = []
    private var organisation = ProjectOrganisation()
    private var folder: UUID?
    private var showingArchive = false
    var creationFolder: UUID? { showingArchive ? nil : folder }
    private var query: String { search.searchBar.text?.trimmingCharacters(in: .whitespacesAndNewlines) ?? "" }
    private var current: UUID?
    private let search = UISearchController(searchResultsController: nil)
    private let message = UILabel()
    private var busy = false

    init() { super.init(style: .insetGrouped) }
    required init?(coder: NSCoder) { super.init(coder: coder) }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.background
        tableView.accessibilityIdentifier = "project-browser"
        navigationItem.rightBarButtonItems = [
            UIBarButtonItem(title: ProjectText.text("new"), style: .plain, target: self, action: #selector(newProject)),
            UIBarButtonItem(title: ProjectText.text("import"), style: .plain, target: self, action: #selector(importFile)),
            UIBarButtonItem(title: ProjectText.text("organise"), menu: UIMenu(children: []))
        ]
        navigationItem.rightBarButtonItems?.first?.accessibilityIdentifier = "project-new"
        navigationItem.rightBarButtonItems?[1].accessibilityIdentifier = "project-import"
        navigationItem.rightBarButtonItems?.last?.accessibilityIdentifier = "project-organise"
        updateActionImages()
        registerForTraitChanges([UITraitUserInterfaceStyle.self, UITraitDisplayScale.self]) { (browser: ProjectBrowserController, _: UITraitCollection) in
            browser.updateActionImages()
            browser.tableView.reloadData()
        }
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
        display(ProjectLibraryContents(projects: projects, organisation: organisation), current: current)
    }

    func display(_ contents: ProjectLibraryContents, current: UUID?) {
        projects = contents.projects; organisation = contents.organisation; self.current = current
        if let folder, organisation.folder(folder) == nil { self.folder = nil }
        if let folder { showingArchive = organisation.isArchived(folder: folder) }
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
        navigationItem.leftBarButtonItems?.forEach { $0.isEnabled = !busy }
        navigationItem.rightBarButtonItems?.forEach { $0.isEnabled = !busy }
        navigationItem.rightBarButtonItems?.first?.isEnabled = !busy && !showingArchive
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
        folders = organisation.visibleFolders(in: folder, archive: showingArchive, query: query)
        filtered = organisation.visibleProjects(projects, in: folder, archive: showingArchive, query: query)
        message.text = ProjectText.text(!query.isEmpty ? "organisation_no_matches" : (showingArchive ? "archive_empty" : "folder_empty"))
        message.isHidden = !filtered.isEmpty || !folders.isEmpty
        updateNavigation()
        tableView.reloadData()
    }

    override func numberOfSections(in tableView: UITableView) -> Int { 2 }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { section == 0 ? folders.count : filtered.count }

    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? {
        if section == 0 { return folders.isEmpty ? nil : ProjectText.text("folders") }
        return filtered.isEmpty ? nil : ProjectText.text("title")
    }

    override func tableView(_ tableView: UITableView, titleForFooterInSection section: Int) -> String? {
        guard section == 1, let project = projects.first(where: { $0.id == current }) else { return nil }
        return ProjectText.text("current_actions").replacingOccurrences(of: "{name}", with: project.name)
    }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let cell = tableView.dequeueReusableCell(withIdentifier: "project") ?? UITableViewCell(style: .subtitle, reuseIdentifier: "project")
        if indexPath.section == 0 {
            guard folders.indices.contains(indexPath.row) else { return cell }
            let value = folders[indexPath.row]
            var content = cell.defaultContentConfiguration()
            content.text = value.name
            content.secondaryText = query.isEmpty ? nil : location(value.parent)
            content.image = WorkspaceIcon.image(value.icon.rawValue, traits: traitCollection)
            cell.contentConfiguration = content
            cell.accessoryType = .disclosureIndicator
            cell.accessibilityIdentifier = "project-folder-" + value.id.uuidString
            return cell
        }
        guard filtered.indices.contains(indexPath.row) else { return cell }
        let project = filtered[indexPath.row]
        var content = cell.defaultContentConfiguration()
        content.text = project.name
        content.secondaryText = DateFormatter.localizedString(from: project.modified, dateStyle: .medium, timeStyle: .short)
            + (query.isEmpty ? "" : " · " + location(organisation.placement(project.id).folder))
        content.image = WorkspaceIcon.image("project", traits: traitCollection)
        cell.contentConfiguration = content
        cell.accessoryType = project.id == current ? .checkmark : .disclosureIndicator
        cell.accessibilityIdentifier = "project-" + project.id.uuidString
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        guard !busy else { return }
        if indexPath.section == 0 {
            guard folders.indices.contains(indexPath.row) else { return }
            folder = folders[indexPath.row].id
            search.searchBar.text = nil
            updateSearchResults(for: search)
            return
        }
        guard filtered.indices.contains(indexPath.row) else { return }
        onOpen?(filtered[indexPath.row])
    }

    override func tableView(_ tableView: UITableView, contextMenuConfigurationForRowAt indexPath: IndexPath, point: CGPoint) -> UIContextMenuConfiguration? {
        guard !busy else { return nil }
        if indexPath.section == 0 {
            guard folders.indices.contains(indexPath.row) else { return nil }
            let value = folders[indexPath.row]
            return UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { [weak self] _ in self?.folderMenu(value) }
        }
        guard filtered.indices.contains(indexPath.row) else { return nil }
        let project = filtered[indexPath.row]
        return UIContextMenuConfiguration(identifier: nil, previewProvider: nil) { [weak self] _ in
            guard let self else { return nil }
            let rename = UIAction(title: ProjectText.text("rename")) { [weak self] _ in self?.askName(renaming: project) }
            let insert = UIAction(title: ProjectText.text("insert"), attributes: self.current == nil || project.id == self.current ? .disabled : []) { [weak self] _ in
                guard let self, !self.busy else { return }
                self.onInsert?(project)
            }
            let duplicate = UIAction(title: ProjectText.text("duplicate"), attributes: self.showingArchive ? .disabled : []) { [weak self] _ in self?.askName(copying: project) }
            let archive = self.archiveAction(project: project.id)
            let delete = UIAction(title: ProjectText.text("delete"), attributes: project.id == self.current ? [.destructive, .disabled] : [.destructive]) { [weak self] _ in
                guard let self else { return }
                let alert = UIAlertController(title: ProjectText.text("delete"), message: ProjectText.text("delete_warning").replacingOccurrences(of: "{name}", with: project.name), preferredStyle: .alert)
                alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
                alert.addAction(UIAlertAction(title: ProjectText.text("delete"), style: .destructive) { [weak self] _ in self?.onDelete?(project) })
                self.present(alert, animated: true)
            }
            return UIMenu(children: [insert, rename, duplicate, self.moveMenu(project: project.id), archive, delete])
        }
    }

    override func tableView(_ tableView: UITableView, trailingSwipeActionsConfigurationForRowAt indexPath: IndexPath) -> UISwipeActionsConfiguration? {
        guard !busy else { return nil }
        if indexPath.section == 0 {
            guard folders.indices.contains(indexPath.row) else { return nil }
            let id = folders[indexPath.row].id
            let action = UIContextualAction(style: .normal, title: ProjectText.text(showingArchive ? "restore" : "archive")) { [weak self] _, _, completion in
                completion(true)
                guard let self else { return }
                self.change(.archiveFolder(id, !self.showingArchive))
            }
            let configuration = UISwipeActionsConfiguration(actions: [action])
            configuration.performsFirstActionWithFullSwipe = false
            return configuration
        }
        guard filtered.indices.contains(indexPath.row) else { return nil }
        let project = filtered[indexPath.row]
        let rename = UIContextualAction(style: .normal, title: ProjectText.text("rename")) { [weak self] _, _, completion in
            completion(true)
            self?.askName(renaming: project)
        }
        let archive = UIContextualAction(style: .normal, title: ProjectText.text(showingArchive ? "restore" : "archive")) { [weak self] _, _, completion in
            completion(true)
            guard let self else { return }
            self.change(.archiveProject(project.id, !self.showingArchive))
        }
        let configuration = UISwipeActionsConfiguration(actions: [archive, rename])
        configuration.performsFirstActionWithFullSwipe = false
        return configuration
    }

    @objc private func close() { onClose?() }
    @objc private func newProject() { guard !showingArchive else { return }; askName(copying: nil) }
    @objc private func importFile() { guard !busy else { return }; onImport?() }
    @objc private func shareFile() { guard !busy, current != nil else { return }; onShare?() }

    private func location(_ folder: UUID?) -> String {
        folder == nil ? ProjectText.text("title") : organisation.path(folder)
    }

    private func change(_ change: ProjectLibraryChange) {
        guard !busy else { return }
        onOrganise?(change)
    }

    private func updateActionImages() {
        for (index, key) in ["new", "import"].enumerated() {
            guard let button = navigationItem.rightBarButtonItems?[index] else { continue }
            let image = WorkspaceIcon.image(key, traits: traitCollection)
            button.image = image
            button.title = image == nil ? ProjectText.text(key) : nil
            button.accessibilityLabel = ProjectText.text(key)
        }
    }

    private func updateNavigation() {
        if let currentFolder = organisation.folder(folder) {
            title = (showingArchive ? ProjectText.text("archive_title") + " / " : "") + currentFolder.name
            navigationItem.prompt = currentFolder.parent == nil ? nil : location(currentFolder.parent)
        } else {
            title = ProjectText.text(showingArchive ? "archive_title" : "title")
            navigationItem.prompt = nil
        }
        let close = UIBarButtonItem(title: WorkspaceStyle.text("close"), style: .plain, target: self, action: #selector(close))
        if folder != nil || showingArchive {
            let back = UIBarButtonItem(title: ProjectText.text("folder_back"), style: .plain, target: self, action: #selector(back))
            back.accessibilityIdentifier = "project-folder-back"
            navigationItem.leftBarButtonItems = [back, close]
        } else { navigationItem.leftBarButtonItems = [close] }
        let sort = organisation.sort(folder: folder, archive: showingArchive)
        let sortMenu = UIMenu(title: ProjectText.text("sort"), children: ProjectSort.allCases.map { value in
            UIAction(title: ProjectText.text("sort_" + value.rawValue), state: sort == value ? .on : .off) { [weak self] _ in
                guard let self else { return }
                self.change(.sort(value, folder: self.folder, archive: self.showingArchive))
            }
        })
        var actions: [UIMenuElement] = [sortMenu]
        if !showingArchive {
            actions.append(UIAction(title: ProjectText.text("new_folder")) { [weak self] _ in self?.askFolderName() })
        }
        if let currentFolder = organisation.folder(folder) {
            actions.append(folderMenu(currentFolder))
        }
        actions.append(UIAction(title: ProjectText.text(showingArchive ? "show_projects" : "show_archive")) { [weak self] _ in
            guard let self, !self.busy else { return }
            self.showingArchive.toggle(); self.folder = nil; self.search.searchBar.text = nil
            self.updateSearchResults(for: self.search)
        })
        navigationItem.rightBarButtonItems?.last?.menu = UIMenu(children: actions)
        updateActions()
    }

    @objc private func back() {
        guard !busy else { return }
        if let value = organisation.folder(folder) {
            folder = showingArchive && !organisation.isArchived(folder: value.parent) ? nil : value.parent
        } else { showingArchive = false; folder = nil }
        search.searchBar.text = nil
        updateSearchResults(for: search)
    }

    private func folderMenu(_ value: ProjectFolder) -> UIMenu {
        let rename = UIAction(title: ProjectText.text("rename_folder")) { [weak self] _ in self?.askFolderName(renaming: value) }
        let icons = UIMenu(title: ProjectText.text("folder_icon"), children: ProjectFolderIcon.allCases.map { icon in
            UIAction(title: ProjectText.text("icon_" + icon.rawValue), image: WorkspaceIcon.image(icon.rawValue, traits: traitCollection),
                     state: value.icon == icon ? .on : .off) { [weak self] _ in self?.change(.folderIcon(value.id, icon)) }
        })
        return UIMenu(title: value.name, children: [rename, icons, moveMenu(folder: value.id), archiveAction(folder: value.id)])
    }

    private func archiveAction(project: UUID? = nil, folder: UUID? = nil) -> UIAction {
        UIAction(title: ProjectText.text(showingArchive ? "restore" : "archive")) { [weak self] _ in
            guard let self else { return }
            if let project { self.change(.archiveProject(project, !self.showingArchive)) }
            else if let folder { self.change(.archiveFolder(folder, !self.showingArchive)) }
        }
    }

    private func moveMenu(project: UUID? = nil, folder: UUID? = nil) -> UIMenu {
        let currentParent = project.map { organisation.placement($0).folder } ?? organisation.folder(folder)?.parent
        let root = UIAction(title: ProjectText.text("title"), attributes: showingArchive || currentParent == nil ? .disabled : []) { [weak self] _ in
            if let project { self?.change(.moveProject(project, to: nil)) }
            else if let folder { self?.change(.moveFolder(folder, to: nil)) }
        }
        let destinations = organisation.folders.filter { value in
            !organisation.isArchived(folder: value.id) && !organisation.ancestors(of: value.id).contains { $0.id == folder }
        }.sorted { organisation.path($0.id).localizedStandardCompare(organisation.path($1.id)) == .orderedAscending }
        let actions = destinations.map { value in
            UIAction(title: organisation.path(value.id), image: WorkspaceIcon.image(value.icon.rawValue, traits: traitCollection),
                     attributes: showingArchive || currentParent == value.id ? .disabled : []) { [weak self] _ in
                if let project { self?.change(.moveProject(project, to: value.id)) }
                else if let folder { self?.change(.moveFolder(folder, to: value.id)) }
            }
        }
        return UIMenu(title: ProjectText.text("move_to_folder"), children: [root] + actions)
    }

    private func askFolderName(renaming value: ProjectFolder? = nil) {
        guard !busy else { return }
        let alert = UIAlertController(title: ProjectText.text(value == nil ? "new_folder" : "rename_folder"), message: ProjectText.text("folder_name_hint"), preferredStyle: .alert)
        alert.addTextField { field in
            field.placeholder = ProjectText.text("folder_name"); field.text = value?.name
            field.autocapitalizationType = .sentences
            field.accessibilityIdentifier = "project-folder-name"
        }
        alert.addAction(UIAlertAction(title: WorkspaceStyle.text("cancel"), style: .cancel))
        let save = UIAlertAction(title: ProjectText.text(value == nil ? "create" : "rename_folder"), style: .default) { [weak self, weak alert] _ in
            guard let self, let name = try? ProjectLibrary.validName(alert?.textFields?.first?.text ?? "") else { return }
            if let value { self.change(.renameFolder(value.id, name)) }
            else { self.change(.createFolder(name, parent: self.folder)) }
        }
        save.isEnabled = value != nil
        alert.textFields?.first?.addAction(UIAction { [weak save] action in
            guard let field = action.sender as? UITextField else { return }
            save?.isEnabled = (try? ProjectLibrary.validName(field.text ?? "")) != nil
        }, for: .editingChanged)
        alert.addAction(save)
        present(alert, animated: true)
    }

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
