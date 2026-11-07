import UIKit
import UniformTypeIdentifiers

enum ProjectText {
    static func text(_ key: String) -> String { NativeText.value("ui.ipados.projects." + key) }
}

/// A native library of design files, independent of the model's component tree.
/// Callbacks reach the one serial Rust session; the browser owns no CAD state.
final class ProjectBrowserController: UITableViewController, UISearchResultsUpdating, UITableViewDragDelegate, UITableViewDropDelegate, UIDropInteractionDelegate {
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
    private var projectIDs: Set<UUID> = []
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
    private let dragOwner = UUID()
    private let parentDropView = UIView()
    private let parentDropLabel = UILabel()
#if DEBUG
    private var checkedDragging = false
#endif

    private struct DraggedItem {
        let owner: UUID
        let item: ProjectLibraryItem
    }

    init() { super.init(style: .insetGrouped) }
    required init?(coder: NSCoder) { super.init(coder: coder) }

#if DEBUG
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        if !checkedDragging, ProcessInfo.processInfo.arguments.contains("--project-drag-check") {
            checkedDragging = true
            checkDraggingOnDevice()
        }
    }
#endif

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = WorkspaceStyle.background
        tableView.accessibilityIdentifier = "project-browser"
        tableView.dragDelegate = self
        tableView.dropDelegate = self
        tableView.dragInteractionEnabled = true
        tableView.keyboardDismissMode = .onDrag
        parentDropLabel.font = .preferredFont(forTextStyle: .body)
        parentDropLabel.adjustsFontForContentSizeCategory = true
        parentDropLabel.numberOfLines = 0
        parentDropLabel.textColor = .secondaryLabel
        parentDropLabel.textAlignment = .center
        parentDropView.accessibilityIdentifier = "project-parent-drop"
        parentDropView.isAccessibilityElement = true
        parentDropView.addSubview(parentDropLabel)
        parentDropView.addInteraction(UIDropInteraction(delegate: self))
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
        projects = contents.projects; projectIDs = Set(projects.map(\.id))
        organisation = contents.organisation; self.current = current
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
        tableView.dragInteractionEnabled = !busy && !showingArchive
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
        updateParentDropTarget()
        tableView.reloadData()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        updateParentDropTarget()
    }

    private var parentDestination: UUID? { organisation.folder(folder)?.parent }

    private func updateParentDropTarget() {
        guard !showingArchive, folder != nil || !query.isEmpty else {
            if tableView.tableHeaderView != nil { tableView.tableHeaderView = nil }
            return
        }
        let text = ProjectText.text("drop_to_folder").replacingOccurrences(of: "{folder}", with: location(parentDestination))
        parentDropLabel.text = text
        parentDropView.accessibilityLabel = text
        let width = tableView.bounds.width
        let size = parentDropLabel.sizeThatFits(CGSize(width: max(1, width - 32), height: .greatestFiniteMagnitude))
        let height = max(56, size.height + 24)
        parentDropLabel.frame = CGRect(x: 16, y: 12, width: max(1, width - 32), height: height - 24)
        if tableView.tableHeaderView !== parentDropView || parentDropView.frame.size != CGSize(width: width, height: height) {
            parentDropView.frame = CGRect(x: 0, y: 0, width: width, height: height)
            tableView.tableHeaderView = parentDropView
        }
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
        guard !busy, !tableView.hasActiveDrag else { return }
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
        endTextEntry()
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

    private func endTextEntry() {
        // Keep the Find query, but never focus text entry for a hold or drag.
        search.searchBar.searchTextField.resignFirstResponder()
        search.searchBar.resignFirstResponder()
        view.endEditing(true)
    }

    private func draggedItem(at indexPath: IndexPath) -> UIDragItem? {
        guard !busy, !showingArchive else { return nil }
        let item: ProjectLibraryItem
        let name: String
        if indexPath.section == 0, folders.indices.contains(indexPath.row) {
            let value = folders[indexPath.row]
            item = .folder(value.id); name = value.name
        } else if indexPath.section == 1, filtered.indices.contains(indexPath.row) {
            let value = filtered[indexPath.row]
            item = .project(value.id); name = value.name
        } else { return nil }
        endTextEntry()
        let drag = UIDragItem(itemProvider: NSItemProvider(object: name as NSString))
        drag.localObject = DraggedItem(owner: dragOwner, item: item)
        return drag
    }

    func tableView(_ tableView: UITableView, itemsForBeginning session: UIDragSession, at indexPath: IndexPath) -> [UIDragItem] {
        draggedItem(at: indexPath).map { [$0] } ?? []
    }

    func tableView(_ tableView: UITableView, itemsForAddingTo session: UIDragSession, at indexPath: IndexPath, point: CGPoint) -> [UIDragItem] { [] }
    func tableView(_ tableView: UITableView, dragSessionAllowsMoveOperation session: UIDragSession) -> Bool { true }
    func tableView(_ tableView: UITableView, dragSessionIsRestrictedToDraggingApplication session: UIDragSession) -> Bool { true }

    private func moveForDrop(_ items: [UIDragItem], to destination: UUID?) -> ProjectLibraryChange? {
        guard !busy, !showingArchive, items.count == 1,
              let dragged = items.first?.localObject as? DraggedItem, dragged.owner == dragOwner else { return nil }
        return organisation.moveChange(for: dragged.item, to: destination, existingProjects: projectIDs)
    }

    private func folderDrop(at point: CGPoint, items: [UIDragItem]) -> (IndexPath, ProjectLibraryChange)? {
        guard let index = tableView.indexPathForRow(at: point), index.section == 0, folders.indices.contains(index.row),
              let change = moveForDrop(items, to: folders[index.row].id) else { return nil }
        return (index, change)
    }

    func tableView(_ tableView: UITableView, canHandle session: UIDropSession) -> Bool {
        !busy && !showingArchive && session.localDragSession != nil && session.items.count == 1
            && (session.items.first?.localObject as? DraggedItem)?.owner == dragOwner
    }

    func tableView(_ tableView: UITableView, dropSessionDidUpdate session: UIDropSession, withDestinationIndexPath destinationIndexPath: IndexPath?) -> UITableViewDropProposal {
        guard self.tableView(tableView, canHandle: session), folderDrop(at: session.location(in: tableView), items: session.items) != nil else {
            return UITableViewDropProposal(operation: .forbidden)
        }
        return UITableViewDropProposal(operation: .move, intent: .insertIntoDestinationIndexPath)
    }

    func tableView(_ tableView: UITableView, performDropWith coordinator: UITableViewDropCoordinator) {
        guard self.tableView(tableView, canHandle: coordinator.session),
              let item = coordinator.items.first else { return }
        commitFolderDrop(coordinator.session.items, at: coordinator.session.location(in: tableView)) { index in
            if let cell = tableView.cellForRow(at: index) {
                coordinator.drop(item.dragItem, intoRowAt: index, rect: cell.bounds)
            }
        }
    }

    private func commitFolderDrop(_ items: [UIDragItem], at point: CGPoint, animate: (IndexPath) -> Void) {
        guard let (index, move) = folderDrop(at: point, items: items) else { return }
        animate(index)
        change(move)
    }

    func dropInteraction(_ interaction: UIDropInteraction, canHandle session: UIDropSession) -> Bool {
        tableView(tableView, canHandle: session)
    }

    func dropInteraction(_ interaction: UIDropInteraction, sessionDidUpdate session: UIDropSession) -> UIDropProposal {
        let allowed = tableView(tableView, canHandle: session) && moveForDrop(session.items, to: parentDestination) != nil
        parentDropView.backgroundColor = allowed ? .tertiarySystemFill : .clear
        return UIDropProposal(operation: allowed ? .move : .forbidden)
    }

    func dropInteraction(_ interaction: UIDropInteraction, performDrop session: UIDropSession) {
        parentDropView.backgroundColor = .clear
        guard tableView(tableView, canHandle: session), let move = moveForDrop(session.items, to: parentDestination) else { return }
        change(move)
    }

    func dropInteraction(_ interaction: UIDropInteraction, sessionDidExit session: UIDropSession) { parentDropView.backgroundColor = .clear }
    func dropInteraction(_ interaction: UIDropInteraction, sessionDidEnd session: UIDropSession) { parentDropView.backgroundColor = .clear }

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

#if DEBUG
private extension ProjectBrowserController {
    /// Synthetic sessions exercise the production delegates on physical UIKit.
    /// They do not claim to deliver a physical finger's drag gesture.
    final class CheckSession: NSObject, UIDragSession, UIDropSession {
        var items: [UIDragItem] = []
        var point = CGPoint.zero
        var local = true
        var localContext: Any?
        var localDragSession: UIDragSession? { local ? self : nil }
        var allowsMoveOperation: Bool { true }
        var isRestrictedToDraggingApplication: Bool { true }
        var progress = Progress(totalUnitCount: 0)
        var progressIndicatorStyle: UIDropSessionProgressIndicatorStyle = .none
        func location(in view: UIView) -> CGPoint { point }
        func hasItemsConforming(toTypeIdentifiers typeIdentifiers: [String]) -> Bool { false }
        func canLoadObjects(ofClass aClass: NSItemProviderReading.Type) -> Bool { false }
        func loadObjects(ofClass aClass: NSItemProviderReading.Type, completion: @escaping ([NSItemProviderReading]) -> Void) -> Progress {
            completion([]); return progress
        }
    }

    func checkDraggingOnDevice() {
        let savedContents = ProjectLibraryContents(projects: projects, organisation: organisation)
        let savedCurrent = current, savedFolder = folder, savedArchive = showingArchive, savedBusy = busy
        let savedQuery = search.searchBar.text, savedActive = search.isActive, savedOrganise = onOrganise
        // iPadOS can collapse the navigation search field to a magnifier. Use
        // a visible UIKit field to exercise keyboard dismissal independently
        // of that presentation, while checking the real Find query survives.
        let focusField = UITextField(frame: CGRect(x: 16, y: 16, width: 220, height: 44))
        focusField.accessibilityIdentifier = "project-drag-focus-fixture"
        func focusTextEntry() -> Bool {
            view.addSubview(focusField)
            return focusField.becomeFirstResponder()
        }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("project-drag-" + UUID().uuidString)
        var issues: [String] = []
        var captures: [String] = []
        let documents = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first
        func check(_ condition: Bool, _ problem: String) { if !condition { issues.append(problem) } }
        func capture(_ stage: String) throws {
            guard let documents else { throw ProjectLibrary.failure("missing_documents") }
            view.layoutIfNeeded(); tableView.layoutIfNeeded()
            let surface: UIView = view.window ?? view
            let image = UIGraphicsImageRenderer(bounds: surface.bounds).image { _ in surface.drawHierarchy(in: surface.bounds, afterScreenUpdates: true) }
            guard let png = image.pngData() else { throw ProjectLibrary.failure("screenshot_failed") }
            let filename = "project-drag-" + stage + ".png"
            try png.write(to: documents.appendingPathComponent(filename), options: .atomic)
            captures.append(filename)
        }
        defer {
            endTextEntry(); search.isActive = savedActive; search.searchBar.text = savedQuery
            focusField.removeFromSuperview()
            folder = savedFolder; showingArchive = savedArchive; onOrganise = savedOrganise
            display(savedContents, current: savedCurrent); setBusy(savedBusy)
            try? FileManager.default.removeItem(at: root)
        }
        do {
            let library = ProjectLibrary(root: root)
            let first = try library.organise(.createFolder("Workshop", parent: nil))
            guard let work = first.folders.first else { throw ProjectLibrary.failure("missing_test_folder") }
            let nested = try library.organise(.createFolder("Parts", parent: work.id))
            guard let parts = nested.folders.first(where: { $0.parent == work.id }) else { throw ProjectLibrary.failure("missing_test_folder") }
            let project = try library.reserve(name: "Drag fixture")
            try Data("browser-only fixture".utf8).write(to: project.file)
            folder = nil; showingArchive = false; search.searchBar.text = nil; setBusy(false)
            display(ProjectLibraryContents(projects: [project], organisation: nested), current: nil)
            onOrganise = { [weak self] move in
                do { self?.display(ProjectLibraryContents(projects: [project], organisation: try library.organise(move)), current: nil) }
                catch { issues.append(error.localizedDescription) }
            }
            check(tableView.dragDelegate === self && tableView.dropDelegate === self && tableView.dragInteractionEnabled, "Drag delegates are not enabled")
            let session = CheckSession()
            let projectRow = IndexPath(row: 0, section: 1), folderRow = IndexPath(row: 0, section: 0)
            // Start with text focused; a context menu must end it without clearing Find.
            search.searchBar.text = "Drag"; updateSearchResults(for: search)
            let field = focusField
            check(focusTextEntry(), "Could not focus the hold regression's text field")
            check(tableView(tableView, contextMenuConfigurationForRowAt: projectRow, point: .zero) != nil, "Project hold lost its menu")
            check(!field.isFirstResponder && query == "Drag" && presentedViewController == nil, "Project hold opened text entry or cleared Find")
            search.searchBar.text = nil; updateSearchResults(for: search)
            check(focusTextEntry(), "Could not focus the folder hold regression's text field")
            check(tableView(tableView, contextMenuConfigurationForRowAt: folderRow, point: .zero) != nil, "Folder hold lost its menu")
            check(!field.isFirstResponder && presentedViewController == nil, "Folder hold opened text entry")
            check(focusTextEntry(), "Could not focus the drag regression's text field")
            session.items = tableView(tableView, itemsForBeginning: session, at: projectRow)
            check(session.items.count == 1 && !field.isFirstResponder && presentedViewController == nil, "Project drag opened text entry or failed to start")
            focusField.removeFromSuperview()
            tableView.layoutIfNeeded()
            let rect = tableView.rectForRow(at: folderRow)
            session.point = CGPoint(x: rect.midX, y: rect.midY)
            let proposal = tableView(tableView, dropSessionDidUpdate: session, withDestinationIndexPath: folderRow)
            check(proposal.operation == .move && proposal.intent == .insertIntoDestinationIndexPath, "Folder row did not accept a move")
            var animated = false
            commitFolderDrop(session.items, at: session.point) { animated = $0 == folderRow }
            let moved = try library.organisation()
            check(animated && moved.placement(project.id).folder == work.id, "Project drop was not persisted")
            tableView(tableView, didSelectRowAt: folderRow)
            check(folder == work.id && filtered.first?.id == project.id, "Moved project did not appear inside the folder")
            try capture("inside-folder")
            guard let interaction = parentDropView.interactions.compactMap({ $0 as? UIDropInteraction }).first else { throw ProjectLibrary.failure("missing_parent_drop") }
            check(dropInteraction(interaction, sessionDidUpdate: session).operation == .move, "Parent target refused the project")
            dropInteraction(interaction, performDrop: session)
            check((try library.organisation()).placement(project.id).folder == nil, "Parent drop did not return the project to Projects")
            back()
            try capture("returned-to-projects")
            session.items = tableView(tableView, itemsForBeginning: session, at: folderRow)
            check(session.items.count == 1, "Folder drag failed to start")
            tableView.layoutIfNeeded()
            let folderRect = tableView.rectForRow(at: folderRow)
            session.point = CGPoint(x: folderRect.midX, y: folderRect.midY)
            check(tableView(tableView, dropSessionDidUpdate: session, withDestinationIndexPath: folderRow).operation == .forbidden, "Folder accepted itself")
            session.items = tableView(tableView, itemsForBeginning: session, at: projectRow)
            setBusy(true)
            check(!tableView.dragInteractionEnabled && !tableView(tableView, canHandle: session), "Busy browser accepted a drag")
            setBusy(false); session.local = false
            check(!tableView(tableView, canHandle: session), "External session accepted")
            session.local = true
            let other = ProjectBrowserController()
            other.display(ProjectLibraryContents(projects: [project], organisation: nested), current: nil)
            check(!other.tableView(other.tableView, canHandle: session), "Another browser accepted the local identity")
            session.items += session.items
            check(!tableView(tableView, canHandle: session), "Multiple items accepted by a single-item move")
            showingArchive = true; updateSearchResults(for: search)
            check(!tableView.dragInteractionEnabled && tableView(tableView, itemsForBeginning: session, at: projectRow).isEmpty, "Archive enabled dragging")
            check(try Data(contentsOf: project.file) == Data("browser-only fixture".utf8), "Dragging changed design bytes")
            check((try library.organisation()).folder(parts.id)?.parent == work.id, "Dragging changed unrelated folder membership")
        } catch { issues.append(error.localizedDescription) }
        do {
            guard let documents else { return }
            let report: [String: Any] = ["passed": issues.isEmpty, "issues": issues, "captures": captures,
                "completed_at": Date().timeIntervalSince1970, "synthetic_drag_sessions": true, "physical_contacts": false,
                "keyboard_focus_fixture": "visible UITextField; real Find query preserved",
                "candidate": Bundle.main.object(forInfoDictionaryKey: "SolidCraftCandidate") as? String ?? "unstamped"]
            try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
                .write(to: documents.appendingPathComponent("project-drag-check.json"), options: .atomic)
        } catch { print("Project drag report failed: \(error)") }
        print("Project drag check \(issues.isEmpty ? "PASS" : "FAIL"): \(issues)")
    }
}
#endif
