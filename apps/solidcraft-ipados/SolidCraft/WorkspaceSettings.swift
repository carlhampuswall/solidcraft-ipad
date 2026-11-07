import UIKit

enum WorkspaceSettingsCategory: String, CaseIterable {
    case appearance, workspace, navigation, selection, modelling, sketching, view, projects

    var title: String { NativeText.value("ui.ipados.settings." + rawValue) }
}

struct WorkspaceSettingsSection {
    let category: WorkspaceSettingsCategory
    let items: [UIMenuElement]

    var actions: [UIAction] {
        items.flatMap { item -> [UIAction] in
            if let action = item as? UIAction { return [action] }
            return (item as? UIMenu)?.children.compactMap { $0 as? UIAction } ?? []
        }
    }
}

/// A native preferences form. The workspace supplies only preference actions;
/// CAD commands and toolbar customization menus stay in the tool rail.
final class WorkspaceSettingsController: UITableViewController, UISearchResultsUpdating {
    private var sections: [WorkspaceSettingsSection] = []
    private let search = UISearchController(searchResultsController: nil)
    private var query = ""

    init() {
        super.init(style: .insetGrouped)
        title = WorkspaceStyle.text("settings")
        traitOverrides.userInterfaceLevel = .base
        preferredContentSize = CGSize(width: 540, height: 640)
    }
    required init?(coder: NSCoder) { nil }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.accessibilityIdentifier = "workspace-settings-popup"
        tableView.backgroundColor = WorkspaceStyle.background
        let background = UIView()
        background.backgroundColor = WorkspaceStyle.background
        tableView.backgroundView = background
        tableView.keyboardDismissMode = .onDrag
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 60
        navigationItem.rightBarButtonItem = UIBarButtonItem(systemItem: .done, primaryAction: UIAction { [weak self] _ in
            self?.dismiss(animated: true)
        })
        search.obscuresBackgroundDuringPresentation = false
        search.searchResultsUpdater = self
        navigationItem.searchController = search
        navigationItem.hidesSearchBarWhenScrolling = false
        definesPresentationContext = true
    }

    func render(_ sections: [WorkspaceSettingsSection]) {
        self.sections = sections
        guard isViewLoaded else { return }
        let offset = tableView.contentOffset
        tableView.reloadData()
        tableView.layoutIfNeeded()
        tableView.setContentOffset(offset, animated: false)
        for page in navigationController?.viewControllers.compactMap({ $0 as? WorkspaceSettingsChoicesController }) ?? [] {
            if let menu = sections.flatMap(\.items).compactMap({ $0 as? UIMenu }).first(where: { $0.title == page.title }) {
                page.render(menu.children.compactMap { $0 as? UIAction })
            }
        }
    }

    private var displayed: [WorkspaceSettingsSection] {
        guard !query.isEmpty else { return sections }
        return sections.compactMap { section in
            let items = section.items.filter { item in
                section.category.title.localizedStandardContains(query) || item.title.localizedStandardContains(query) ||
                    (item as? UIAction)?.subtitle?.localizedStandardContains(query) == true
            }
            return items.isEmpty ? nil : WorkspaceSettingsSection(category: section.category, items: items)
        }
    }

    func updateSearchResults(for searchController: UISearchController) {
        query = searchController.searchBar.text?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        tableView.reloadData()
    }

    override func numberOfSections(in tableView: UITableView) -> Int { displayed.count }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { displayed[section].items.count }
    override func tableView(_ tableView: UITableView, titleForHeaderInSection section: Int) -> String? { displayed[section].category.title }
    override func tableView(_ tableView: UITableView, willDisplayHeaderView view: UIView, forSection section: Int) {
        (view as? UITableViewHeaderFooterView)?.textLabel?.numberOfLines = 1
    }

    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let item = displayed[indexPath.section].items[indexPath.row]
        let cell = UITableViewCell(style: .subtitle, reuseIdentifier: nil)
        var content = cell.defaultContentConfiguration()
        content.text = item.title
        content.textProperties.numberOfLines = 1
        content.textProperties.lineBreakMode = .byTruncatingTail
        content.secondaryTextProperties.numberOfLines = 1
        content.secondaryTextProperties.lineBreakMode = .byTruncatingTail
        content.textProperties.color = WorkspaceStyle.foreground
        content.secondaryTextProperties.color = WorkspaceStyle.secondary
        cell.accessibilityIdentifier = "workspace-setting-" + item.title
        cell.backgroundColor = WorkspaceStyle.fieldBackground
        if let action = item as? UIAction {
            content.secondaryText = action.subtitle
            let toggle = UISwitch()
            toggle.isOn = action.state == .on
            toggle.isEnabled = !action.attributes.contains(.disabled)
            toggle.accessibilityLabel = action.title
            toggle.accessibilityHint = action.subtitle
            toggle.accessibilityIdentifier = cell.accessibilityIdentifier
            toggle.addAction(UIAction { [weak self, weak toggle] _ in
                guard let toggle else { return }
                action.performWithSender(toggle, target: nil)
                self?.tableView.reloadData()
            }, for: .valueChanged)
            cell.accessoryView = toggle
            cell.contentView.alpha = toggle.isEnabled ? 1 : 0.45
        } else if let menu = item as? UIMenu {
            content.secondaryText = menu.children.compactMap { $0 as? UIAction }.first(where: { $0.state == .on })?.title
            cell.accessoryType = .disclosureIndicator
        }
        cell.contentConfiguration = content
        return cell
    }

    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        let item = displayed[indexPath.section].items[indexPath.row]
        if let action = item as? UIAction, !action.attributes.contains(.disabled) {
            action.performWithSender(tableView.cellForRow(at: indexPath), target: nil)
        } else if let menu = item as? UIMenu {
            let choices = WorkspaceSettingsChoicesController(title: menu.title, actions: menu.children.compactMap { $0 as? UIAction })
            navigationController?.pushViewController(choices, animated: true)
        }
    }

    override var keyCommands: [UIKeyCommand]? {
        [UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(close))]
    }
    @objc private func close() { dismiss(animated: true) }
}

private final class WorkspaceSettingsChoicesController: UITableViewController {
    private var actions: [UIAction]
    init(title: String, actions: [UIAction]) {
        self.actions = actions
        super.init(style: .insetGrouped)
        traitOverrides.userInterfaceLevel = .base
        self.title = title
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidLoad() {
        super.viewDidLoad()
        tableView.backgroundColor = WorkspaceStyle.background
        let background = UIView()
        background.backgroundColor = WorkspaceStyle.background
        tableView.backgroundView = background
        tableView.rowHeight = UITableView.automaticDimension
        tableView.estimatedRowHeight = 52
    }
    func render(_ actions: [UIAction]) { self.actions = actions; if isViewLoaded { tableView.reloadData() } }
    override func tableView(_ tableView: UITableView, numberOfRowsInSection section: Int) -> Int { actions.count }
    override func tableView(_ tableView: UITableView, cellForRowAt indexPath: IndexPath) -> UITableViewCell {
        let action = actions[indexPath.row]
        let cell = UITableViewCell(style: .default, reuseIdentifier: nil)
        var content = cell.defaultContentConfiguration()
        content.text = action.title
        content.textProperties.numberOfLines = 1
        content.textProperties.lineBreakMode = .byTruncatingTail
        content.textProperties.color = WorkspaceStyle.foreground
        cell.contentConfiguration = content
        cell.backgroundColor = WorkspaceStyle.fieldBackground
        cell.accessoryType = action.state == .on ? .checkmark : .none
        cell.isUserInteractionEnabled = !action.attributes.contains(.disabled)
        cell.contentView.alpha = cell.isUserInteractionEnabled ? 1 : 0.45
        return cell
    }
    override func tableView(_ tableView: UITableView, didSelectRowAt indexPath: IndexPath) {
        tableView.deselectRow(at: indexPath, animated: true)
        let action = actions[indexPath.row]
        guard !action.attributes.contains(.disabled) else { return }
        action.performWithSender(tableView.cellForRow(at: indexPath), target: nil)
    }
}
