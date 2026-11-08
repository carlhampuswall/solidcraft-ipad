import Foundation

/// Native file organisation only. Each design remains the upstream native
/// container; directory UUIDs keep equally named imports from overwriting work.
struct ProjectRecord: Equatable {
    let id: UUID
    let file: URL
    let modified: Date
    var name: String { file.deletingPathExtension().lastPathComponent }
    var recoveryFile: URL { URL(fileURLWithPath: file.path + ".recovery") }
}

struct ProjectReferenceUpdate {
    var changed = false
    var failed = false
    var warnings: [String] = []
}

/// Native file access only. CAD references and their cached display remain in
/// the engine's DesignLink; bookmarks retain permission to read Files sources.
extension ProjectLibrary {
    private struct ReferenceBookmarks: Codable {
        var version = 1
        var files: [String: Data] = [:]
    }
    private var referenceBookmarksFile: URL { root.appendingPathComponent(".reference-bookmarks.json") }

    private func referenceBookmarks() throws -> ReferenceBookmarks {
        guard FileManager.default.fileExists(atPath: referenceBookmarksFile.path) else { return ReferenceBookmarks() }
        let info = try referenceBookmarksFile.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
        guard info.isRegularFile == true, info.isSymbolicLink != true, let size = info.fileSize, size <= 8 * 1024 * 1024 else {
            throw Self.failure("reference_unavailable")
        }
        let bookmarks = try JSONDecoder().decode(ReferenceBookmarks.self, from: Data(contentsOf: referenceBookmarksFile))
        guard bookmarks.version == 1, bookmarks.files.count <= 4096,
              bookmarks.files.allSatisfy({ $0.key.hasPrefix("/") && $0.key.utf8.count < 4096 && $0.value.count <= 64 * 1024 }) else {
            throw Self.failure("reference_unavailable")
        }
        return bookmarks
    }

    private func managedReferenceID(_ source: URL) -> UUID? {
        let directory = source.standardizedFileURL.deletingLastPathComponent()
        guard directory.deletingLastPathComponent() == root.resolvingSymlinksInPath().standardizedFileURL else { return nil }
        return UUID(uuidString: directory.lastPathComponent)
    }

    /// UUID directories preserve library identity across rename, sort, archive
    /// and folder moves. A removed source is never replaced by a same-name file.
    func referenceURL(_ source: URL) throws -> URL {
        try resolvedReference(source).url
    }

    private func resolvedReference(_ source: URL) throws -> (url: URL, stale: Bool) {
        if let id = managedReferenceID(source) {
            guard let project = try projects().first(where: { $0.id == id }) else { throw Self.failure("reference_unavailable") }
            return (project.file, false)
        }
        guard let bookmark = try referenceBookmarks().files[source.path] else { return (source, false) }
        var stale = false
        let url = try URL(resolvingBookmarkData: bookmark, options: [.withoutUI], relativeTo: nil, bookmarkDataIsStale: &stale)
        return (url, stale)
    }

    /// Read the actual source under provider coordination, without staging a
    /// copy. External source permission is durable; save destinations stay local.
    func withProjectReference<T>(_ source: URL, remember: Bool = false, _ operation: (URL) throws -> T) throws -> T {
        let reference = try resolvedReference(source)
        let resolved = reference.url
        guard resolved.isFileURL, resolved.path.utf8.count < 4096,
              Self.nativeExtensions.contains(resolved.pathExtension.lowercased()) else { throw Self.failure("reference_unavailable") }
        let access = resolved.startAccessingSecurityScopedResource()
        defer { if access { resolved.stopAccessingSecurityScopedResource() } }
        if managedReferenceID(resolved) == nil {
            var bookmarks = try referenceBookmarks()
            if remember || (bookmarks.files[source.path] != nil && (reference.stale || resolved.path != source.path)) {
                let bookmark = try resolved.bookmarkData(options: [.minimalBookmark], includingResourceValuesForKeys: nil, relativeTo: nil)
                guard bookmark.count <= 64 * 1024 else { throw Self.failure("reference_unavailable") }
                if bookmarks.files[source.path] != bookmark || bookmarks.files[resolved.path] != bookmark {
                    bookmarks.files[source.path] = bookmark
                    bookmarks.files[resolved.path] = bookmark
                    guard bookmarks.files.count <= 4096 else { throw Self.failure("reference_unavailable") }
                    let data = try JSONEncoder().encode(bookmarks)
                    guard data.count <= 8 * 1024 * 1024 else { throw Self.failure("reference_unavailable") }
                    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
                    try data.write(to: referenceBookmarksFile, options: .atomic)
                }
            }
        }
        var coordinationError: NSError?
        var result: Result<T, Error> = .failure(Self.failure("reference_unavailable"))
        NSFileCoordinator().coordinate(readingItemAt: resolved, options: [], error: &coordinationError) { readable in
            result = Result {
                let info = try readable.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
                guard info.isRegularFile == true, info.isSymbolicLink != true,
                      let size = info.fileSize, size > 0, Int64(size) <= Self.maximumImportBytes else { throw Self.failure("import_size") }
                return try operation(readable)
            }
        }
        if let coordinationError { throw coordinationError }
        return try result.get()
    }

    /// Decode the existing command envelope; no native document representation.
    static func commandValue(_ data: Data) throws -> [String: Any] {
        let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        let result = json?["result"] as? [String: Any]
        let command = result?["command"] as? [String: Any]
        guard let value = (command?["value"] as? [String: Any]) ?? command ?? result else { throw failure("reference_unavailable") }
        return value
    }

    /// Issue only existing engine commands. Unchanged sources create no undo
    /// entries; each changed link preserves its component/occurrence identity.
    func refreshProjectReferences(command: (String, [String: Any]) throws -> Data) throws -> ProjectReferenceUpdate {
        struct Link: Decodable { let component: UInt64; let file: String }
        let value = try Self.commandValue(command("component.links", [:]))
        let data = try JSONSerialization.data(withJSONObject: value["links"] ?? [])
        let links = try JSONDecoder().decode([Link].self, from: data)
        guard links.count <= 1024 else { throw Self.failure("reference_unavailable") }
        var report = ProjectReferenceUpdate()
        for link in links {
            do {
                try withProjectReference(URL(fileURLWithPath: link.file)) { readable in
                    let reply: Data
                    if readable.standardizedFileURL != URL(fileURLWithPath: link.file).standardizedFileURL {
                        reply = try command("component.relink", ["component": link.component, "path": readable.path])
                    } else {
                        // Recheck while the source's security scope and provider
                        // coordination are held. The engine caches file digests.
                        let current = try Self.commandValue(command("component.links", [:]))["links"] as? [[String: Any]] ?? []
                        guard current.first(where: { ($0["component"] as? UInt64) == link.component })?["changed"] as? Bool == true else { return }
                        reply = try command("component.update_links", ["component": link.component])
                    }
                    report.changed = true
                    report.warnings += try Self.commandValue(reply)["warnings"] as? [String] ?? []
                }
            } catch {
                report.failed = true
                let message = (error as NSError).localizedDescription
                report.warnings.append(message.hasPrefix("ui.ipados.") ? message :
                    URL(fileURLWithPath: link.file).lastPathComponent + ": " + String(describing: error).prefix(1024))
            }
        }
        report.warnings = Array(report.warnings.prefix(32))
        return report
    }
}

enum ProjectSort: String, Codable, CaseIterable {
    case modified, nameAscending, nameDescending

    func sorted(_ projects: [ProjectRecord]) -> [ProjectRecord] {
        projects.sorted {
            if self == .modified {
                return $0.modified == $1.modified ? $0.id.uuidString < $1.id.uuidString : $0.modified > $1.modified
            }
            let order = $0.name.localizedStandardCompare($1.name)
            if order == .orderedSame { return $0.id.uuidString < $1.id.uuidString }
            return self == .nameDescending ? order == .orderedDescending : order == .orderedAscending
        }
    }
}

/// Original artwork identifiers and user-selected Unicode emoji. Neither form
/// references files; existing folder identifiers keep their saved representation.
struct ProjectLibraryIcon: Codable, Hashable {
    let rawValue: String
    init(_ rawValue: String) { self.rawValue = rawValue }
    static let folder = Self("folder"), project = Self("project"), library = Self("library")
    static let box = Self("box"), component = Self("component")
    static let common: [Self] = [.folder, .project, .library, .box, .component]
    static let emojiChoices: [Self] = [
        "📁", "📂", "🗂️", "🗃️", "📦", "🏠", "🏗️", "🛠️", "🔧", "⚙️", "🔩", "🧰",
        "🚗", "🚲", "✈️", "🚀", "⛵️", "🤖", "💡", "🔌", "🔋", "💻", "📱", "🎮",
        "🎨", "📐", "✏️", "🧪", "🔬", "🌱", "🌳", "🌍", "☀️", "⭐️", "❤️", "🔥",
        "💧", "🪑", "🛏️", "💍", "🎁", "🎵", "🧩", "🏆", "📚", "🍽️", "☕️", "🐱", "🐶", "🔒", "✅"
    ].compactMap { emoji($0) }

    var emojiText: String? {
        guard rawValue.hasPrefix("emoji:") else { return nil }
        return String(rawValue.dropFirst(6))
    }

    static func emoji(_ input: String) -> Self? {
        let value = input.trimmingCharacters(in: .whitespacesAndNewlines)
        return validEmoji(value) ? Self("emoji:" + value) : nil
    }

    private static func validEmoji(_ value: String) -> Bool {
        guard value.count == 1, value.utf8.count <= 128 else { return false }
        let scalars = value.unicodeScalars
        let hasBase = scalars.contains { ($0.properties.isEmoji && $0.value > 127) || $0.value == 0x20E3 }
        return hasBase && scalars.allSatisfy {
            $0.properties.isEmoji || $0.value == 0x200D || $0.value == 0xFE0F || $0.value == 0x20E3
                || (0xE0020...0xE007F).contains($0.value)
        }
    }

    var isValid: Bool {
        if let emojiText { return Self.validEmoji(emojiText) }
        let bytes = Array(rawValue.utf8)
        return !bytes.isEmpty && bytes.count <= 96 && bytes.first != 95 && bytes.allSatisfy {
            (97...122).contains($0) || (48...57).contains($0) || $0 == 95
        }
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        self.init(try container.decode(String.self))
        guard isValid else { throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid icon identifier") }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(rawValue)
    }
}

/// App-local picker preferences, shared by projects and folders. Counts record
/// successful applications, independently of the design's current membership.
struct ProjectIconPreferences {
    static let key = "SolidCraft.projects.iconPreferences"
    private let defaults: UserDefaults
    private struct State: Codable {
        var version = 1
        var favorites: [ProjectLibraryIcon] = []
        var uses: [String: Int] = [:]
    }

    init(defaults: UserDefaults = .standard) { self.defaults = defaults }

    private static func supported(_ icon: ProjectLibraryIcon) -> Bool {
        icon.isValid && (icon.emojiText != nil || ProjectLibraryIcon.common.contains(icon))
    }

    private func load() -> State {
        guard let data = defaults.data(forKey: Self.key), data.count <= 2 * 1024 * 1024,
              let decoded = try? JSONDecoder().decode(State.self, from: data), decoded.version == 1 else { return State() }
        var result = State()
        var seen: Set<ProjectLibraryIcon> = []
        result.favorites = Array(decoded.favorites.filter { Self.supported($0) && seen.insert($0).inserted }.prefix(4096))
        let counts = decoded.uses.filter { $0.value > 0 && Self.supported(ProjectLibraryIcon($0.key)) }
            .sorted { $0.value == $1.value ? $0.key < $1.key : $0.value > $1.value }.prefix(4096)
        for (key, count) in counts { result.uses[key] = min(count, 1_000_000) }
        return result
    }

    private func save(_ state: State) {
        guard let data = try? JSONEncoder().encode(state), data.count <= 2 * 1024 * 1024 else { return }
        defaults.set(data, forKey: Self.key)
    }

    var favorites: [ProjectLibraryIcon] { load().favorites }
    func useCount(_ icon: ProjectLibraryIcon) -> Int { load().uses[icon.rawValue] ?? 0 }

    func toggleFavorite(_ icon: ProjectLibraryIcon) {
        guard Self.supported(icon) else { return }
        var state = load()
        if let index = state.favorites.firstIndex(of: icon) { state.favorites.remove(at: index) }
        else if state.favorites.count < 4096 { state.favorites.append(icon) }
        save(state)
    }

    func addFavorite(_ icon: ProjectLibraryIcon) {
        guard Self.supported(icon) else { return }
        var state = load()
        guard !state.favorites.contains(icon), state.favorites.count < 4096 else { return }
        state.favorites.append(icon)
        save(state)
    }

    func recordUse(_ icon: ProjectLibraryIcon) {
        guard Self.supported(icon) else { return }
        var state = load()
        state.uses[icon.rawValue] = min((state.uses[icon.rawValue] ?? 0) + 1, 1_000_000)
        if state.uses.count > 4096 {
            let leastUsed = state.uses.sorted { $0.value == $1.value ? $0.key > $1.key : $0.value < $1.value }.first
            if let leastUsed { state.uses.removeValue(forKey: leastUsed.key) }
        }
        save(state)
    }

    func sections(base: [ProjectLibraryIcon], selected: ProjectLibraryIcon? = nil) -> (favorites: [ProjectLibraryIcon], icons: [ProjectLibraryIcon]) {
        let state = load()
        var candidates = base
        candidates += state.uses.keys.sorted().map(ProjectLibraryIcon.init)
        if let selected { candidates.append(selected) }
        var seen: Set<ProjectLibraryIcon> = []
        let pinned = Set(state.favorites)
        let icons = candidates.filter { Self.supported($0) && !pinned.contains($0) && seen.insert($0).inserted }
            .enumerated().sorted { a, b in
                let first = state.uses[a.element.rawValue] ?? 0, second = state.uses[b.element.rawValue] ?? 0
                return first == second ? a.offset < b.offset : first > second
            }.map(\.element)
        return (state.favorites, icons)
    }
}

struct ProjectFolder: Codable, Equatable {
    let id: UUID
    var name: String
    var parent: UUID?
    var icon: ProjectLibraryIcon = .folder
    var archived = false
}

struct ProjectPlacement: Codable, Equatable {
    var folder: UUID?
    var archived = false
    var icon: ProjectLibraryIcon = .project

    init(folder: UUID? = nil, archived: Bool = false, icon: ProjectLibraryIcon = .project) {
        self.folder = folder; self.archived = archived; self.icon = icon
    }

    private enum CodingKeys: String, CodingKey { case folder, archived, icon }
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        folder = try values.decodeIfPresent(UUID.self, forKey: .folder)
        archived = try values.decodeIfPresent(Bool.self, forKey: .archived) ?? false
        icon = try values.decodeIfPresent(ProjectLibraryIcon.self, forKey: .icon) ?? .project
    }
}

enum ProjectLibraryItem: Equatable {
    case folder(UUID)
    case project(UUID)
}

/// Host library metadata only: no native design container or file path changes.
struct ProjectOrganisation: Codable, Equatable {
    var version = 1
    var folders: [ProjectFolder] = []
    var placements: [String: ProjectPlacement] = [:]
    var sorts: [String: ProjectSort] = [:]

    func folder(_ id: UUID?) -> ProjectFolder? { folders.first { $0.id == id } }
    func placement(_ id: UUID) -> ProjectPlacement { placements[id.uuidString] ?? ProjectPlacement() }
    private var folderIndex: [UUID: ProjectFolder] { Dictionary(folders.map { ($0.id, $0) }, uniquingKeysWith: { first, _ in first }) }

    /// Recheck stable identities against the latest library before accepting a
    /// local drop. Row positions can change during search, sorting or refresh.
    func moveChange(for item: ProjectLibraryItem, to destination: UUID?, existingProjects: Set<UUID>) -> ProjectLibraryChange? {
        guard destination == nil || folder(destination) != nil, !isArchived(folder: destination) else { return nil }
        switch item {
        case .project(let id):
            guard existingProjects.contains(id), !isArchived(project: id), placement(id).folder != destination else { return nil }
            return .moveProject(id, to: destination)
        case .folder(let id):
            guard let source = folder(id), !isArchived(folder: id), source.parent != destination,
                  !ancestors(of: destination).contains(where: { $0.id == id }) else { return nil }
            return .moveFolder(id, to: destination)
        }
    }

    func ancestors(of id: UUID?) -> [ProjectFolder] {
        ancestors(of: id, in: folderIndex)
    }

    private func ancestors(of id: UUID?, in index: [UUID: ProjectFolder]) -> [ProjectFolder] {
        var result: [ProjectFolder] = []
        var next = id
        var visited = Set<UUID>()
        while let id = next, let value = index[id], visited.insert(value.id).inserted {
            result.append(value); next = value.parent
        }
        return result
    }

    func isArchived(folder id: UUID?) -> Bool { ancestors(of: id).contains { $0.archived } }
    func isArchived(project id: UUID) -> Bool {
        let value = placement(id)
        return value.archived || isArchived(folder: value.folder)
    }

    func path(_ id: UUID?) -> String { ancestors(of: id).reversed().map(\.name).joined(separator: " / ") }
    func sort(folder: UUID?, archive: Bool) -> ProjectSort { sorts[Self.sortKey(folder, archive)] ?? .modified }
    static func sortKey(_ folder: UUID?, _ archive: Bool) -> String { folder?.uuidString ?? (archive ? "archive" : "root") }

    func visibleFolders(in parent: UUID?, archive: Bool, query: String = "") -> [ProjectFolder] {
        let index = folderIndex
        return folders.filter { value in
            let parents = ancestors(of: value.parent, in: index)
            let parentArchived = parents.contains { $0.archived }
            let matchesLocation = parent.map { value.parent == $0 || (!query.isEmpty && parents.contains { $0.id == parent }) }
                ?? (query.isEmpty ? (archive ? value.archived && !parentArchived : value.parent == nil) : true)
            return matchesLocation && (value.archived || parentArchived) == archive
                && (query.isEmpty || value.name.localizedCaseInsensitiveContains(query))
        }.sorted {
            let order = $0.name.localizedStandardCompare($1.name)
            if order == .orderedSame { return $0.id.uuidString < $1.id.uuidString }
            return sort(folder: parent, archive: archive) == .nameDescending ? order == .orderedDescending : order == .orderedAscending
        }
    }

    func visibleProjects(_ projects: [ProjectRecord], in parent: UUID?, archive: Bool, query: String = "") -> [ProjectRecord] {
        let index = folderIndex
        return sort(folder: parent, archive: archive).sorted(projects.filter { project in
            let value = placement(project.id)
            let parents = ancestors(of: value.folder, in: index)
            let parentArchived = parents.contains { $0.archived }
            let matchesLocation = parent.map { value.folder == $0 || (!query.isEmpty && parents.contains { $0.id == parent }) }
                ?? (query.isEmpty ? (archive ? value.archived && !parentArchived : value.folder == nil) : true)
            return matchesLocation && (value.archived || parentArchived) == archive
                && (query.isEmpty || project.name.localizedCaseInsensitiveContains(query))
        })
    }

    func validate() throws {
        guard version == 1, folders.count <= 10_000, placements.count <= 100_000,
              Set(folders.map(\.id)).count == folders.count else { throw ProjectLibrary.failure("invalid_organisation") }
        let ids = Set(folders.map(\.id))
        let index = folderIndex
        for value in folders {
            guard try ProjectLibrary.validName(value.name) == value.name, value.icon.isValid,
                  value.parent.map({ ids.contains($0) }) ?? true else { throw ProjectLibrary.failure("invalid_organisation") }
            var visited: Set<UUID> = [value.id]
            var next = value.parent
            while let id = next {
                guard visited.count <= 64, visited.insert(id).inserted else { throw ProjectLibrary.failure("folder_cycle") }
                next = index[id]?.parent
            }
        }
        guard placements.allSatisfy({ UUID(uuidString: $0.key)?.uuidString == $0.key && $0.value.icon.isValid && ($0.value.folder.map { ids.contains($0) } ?? true) }),
              sorts.keys.allSatisfy({ $0 == "root" || $0 == "archive" || UUID(uuidString: $0).map { ids.contains($0) } == true }) else {
            throw ProjectLibrary.failure("invalid_organisation")
        }
    }
}

enum ProjectLibraryChange: Equatable {
    case createFolder(String, parent: UUID?)
    case renameFolder(UUID, String)
    case folderIcon(UUID, ProjectLibraryIcon)
    case projectIcon(UUID, ProjectLibraryIcon)
    case moveFolder(UUID, to: UUID?)
    case moveProject(UUID, to: UUID?)
    case archiveFolder(UUID, Bool)
    case archiveProject(UUID, Bool)
    case sort(ProjectSort, folder: UUID?, archive: Bool)
}

struct ProjectLibraryContents: Equatable {
    let projects: [ProjectRecord]
    let organisation: ProjectOrganisation
}

struct ProjectLibrary {
    let root: URL
    static let showBrowserOnLaunchKey = "SolidCraft.projects.showBrowserOnLaunch"
    static func showsBrowserOnLaunch(defaults: UserDefaults = .standard) -> Bool {
        defaults.object(forKey: showBrowserOnLaunchKey) as? Bool ?? true
    }
    static let maximumImportBytes: Int64 = 512 * 1024 * 1024
    static let nativeExtensions: Set<String> = ["sc", "solidcraft"]
    static let importExtensions = nativeExtensions.union(["step", "stp", "stl", "obj", "3mf"])

    static func applicationLibrary() throws -> Self {
        let support = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        return Self(root: support.appendingPathComponent("SolidCraft/Projects", isDirectory: true))
    }

    static func validName(_ input: String) throws -> String {
        let name = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.utf8.count <= 160, !name.hasPrefix("."),
              name.rangeOfCharacter(from: .controlCharacters) == nil,
              !name.contains("/"), !name.contains(":"), !name.contains("\\") else {
            throw failure("invalid_name")
        }
        return name
    }

    static func failure(_ key: String) -> NSError {
        NSError(domain: "SolidCraft.Projects", code: 1, userInfo: [NSLocalizedDescriptionKey: "ui.ipados.projects." + key])
    }

    private var organisationFile: URL { root.appendingPathComponent(".organisation.json") }

    func organisation() throws -> ProjectOrganisation {
        guard FileManager.default.fileExists(atPath: organisationFile.path) else { return ProjectOrganisation() }
        let values = try organisationFile.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
        guard values.isRegularFile == true, values.isSymbolicLink != true,
              let size = values.fileSize, size <= 4 * 1024 * 1024 else { throw Self.failure("invalid_organisation") }
        let value: ProjectOrganisation
        do { value = try JSONDecoder().decode(ProjectOrganisation.self, from: Data(contentsOf: organisationFile)) }
        catch { throw Self.failure("invalid_organisation") }
        try value.validate()
        return value
    }

    @discardableResult
    func organise(_ change: ProjectLibraryChange) throws -> ProjectOrganisation {
        var value = try organisation()
        func folderIndex(_ id: UUID) throws -> Int {
            guard let index = value.folders.firstIndex(where: { $0.id == id }) else { throw Self.failure("invalid_folder") }
            return index
        }
        func destination(_ id: UUID?) throws {
            if let id { _ = try folderIndex(id) }
            guard !value.isArchived(folder: id) else { throw Self.failure("invalid_folder") }
        }
        func projectPlacement(_ id: UUID) throws -> ProjectPlacement {
            guard try projects().contains(where: { $0.id == id }) else { throw Self.failure("invalid_project") }
            return value.placement(id)
        }
        switch change {
        case .createFolder(let name, let parent):
            try destination(parent)
            value.folders.append(ProjectFolder(id: UUID(), name: try Self.validName(name), parent: parent))
        case .renameFolder(let id, let name):
            let index = try folderIndex(id)
            value.folders[index].name = try Self.validName(name)
        case .folderIcon(let id, let icon):
            let index = try folderIndex(id)
            value.folders[index].icon = icon
        case .projectIcon(let id, let icon):
            var placement = try projectPlacement(id)
            placement.icon = icon
            value.placements[id.uuidString] = placement
        case .moveFolder(let id, let parent):
            let index = try folderIndex(id)
            try destination(parent)
            guard !value.isArchived(folder: id), !value.ancestors(of: parent).contains(where: { $0.id == id }) else { throw Self.failure("folder_cycle") }
            value.folders[index].parent = parent
        case .moveProject(let id, let folder):
            try destination(folder)
            var placement = try projectPlacement(id)
            guard !value.isArchived(project: id) else { throw Self.failure("invalid_project") }
            placement.folder = folder
            value.placements[id.uuidString] = placement
        case .archiveFolder(let id, let archived):
            let index = try folderIndex(id)
            if !archived && value.isArchived(folder: value.folders[index].parent) { value.folders[index].parent = nil }
            value.folders[index].archived = archived
        case .archiveProject(let id, let archived):
            var placement = try projectPlacement(id)
            if !archived && value.isArchived(folder: placement.folder) { placement.folder = nil }
            placement.archived = archived
            value.placements[id.uuidString] = placement
        case .sort(let sort, let folder, let archive):
            if let folder { _ = try folderIndex(folder) }
            value.sorts[ProjectOrganisation.sortKey(folder, archive)] = sort
        }
        try saveOrganisation(value)
        return value
    }

    private func saveOrganisation(_ value: ProjectOrganisation) throws {
        try value.validate()
        let data = try JSONEncoder().encode(value)
        guard data.count <= 4 * 1024 * 1024 else { throw Self.failure("invalid_organisation") }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        try data.write(to: organisationFile, options: .atomic)
    }

    func contents() throws -> ProjectLibraryContents {
        try ProjectLibraryContents(projects: projects(), organisation: organisation())
    }

    func projects() throws -> [ProjectRecord] {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        return try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey], options: [.skipsHiddenFiles]).compactMap { directory in
            guard let id = UUID(uuidString: directory.lastPathComponent),
                  try directory.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey]).isDirectory == true,
                  try directory.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true else { return nil }
            let files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey, .isSymbolicLinkKey], options: [.skipsHiddenFiles])
                .filter { Self.nativeExtensions.contains($0.pathExtension.lowercased()) }
            guard files.count == 1, let file = files.first else { return nil }
            let values = try file.resourceValues(forKeys: [.contentModificationDateKey, .isRegularFileKey, .isSymbolicLinkKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true else { return nil }
            return ProjectRecord(id: id, file: file.resolvingSymlinksInPath().standardizedFileURL, modified: values.contentModificationDate ?? .distantPast)
        }.sorted { $0.modified == $1.modified ? $0.id.uuidString < $1.id.uuidString : $0.modified > $1.modified }
    }

    func reserve(name: String, folder: UUID? = nil) throws -> ProjectRecord {
        let name = try Self.validName(name)
        var organisation = try organisation()
        if let folder {
            guard organisation.folder(folder) != nil, !organisation.isArchived(folder: folder) else { throw Self.failure("invalid_folder") }
        }
        let id = UUID()
        let directory = root.appendingPathComponent(id.uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        if let folder {
            organisation.placements[id.uuidString] = ProjectPlacement(folder: folder)
            do { try saveOrganisation(organisation) }
            catch { try? FileManager.default.removeItem(at: directory); throw error }
        }
        return ProjectRecord(id: id, file: directory.resolvingSymlinksInPath().appendingPathComponent(name).appendingPathExtension("solidcraft"), modified: Date())
    }

    func remove(_ project: ProjectRecord) throws {
        // Re-derive every destructive path from the library and checked UUID.
        let directory = root.appendingPathComponent(project.id.uuidString, isDirectory: true)
        guard project.file.deletingLastPathComponent().standardizedFileURL == directory.resolvingSymlinksInPath().standardizedFileURL else { throw Self.failure("invalid_project") }
        try FileManager.default.removeItem(at: directory)
    }

    /// Keep identity, relative component links and the native extension in place.
    /// The current session can persist its new path without replacing its history.
    func rename(_ project: ProjectRecord, to input: String,
                persist: (ProjectRecord) throws -> Void = { _ in }) throws -> ProjectRecord {
        let name = try Self.validName(input)
        guard try projects().contains(where: { $0.id == project.id && $0.file == project.file }) else {
            throw Self.failure("invalid_project")
        }
        let file = project.file.deletingLastPathComponent().appendingPathComponent(name).appendingPathExtension(project.file.pathExtension)
        guard file != project.file else { return project }
        try FileManager.default.moveItem(at: project.file, to: file)
        let renamed = ProjectRecord(id: project.id, file: file, modified: project.modified)
        var movedRecovery = false
        do {
            if FileManager.default.fileExists(atPath: project.recoveryFile.path) {
                try FileManager.default.moveItem(at: project.recoveryFile, to: renamed.recoveryFile)
                movedRecovery = true
            }
            try persist(renamed)
        }
        catch {
            if movedRecovery { try FileManager.default.moveItem(at: renamed.recoveryFile, to: project.recoveryFile) }
            try FileManager.default.moveItem(at: file, to: project.file)
            throw error
        }
        return renamed
    }

    /// Copy a provider-coordinated import before invoking Rust. No external URL
    /// or security bookmark becomes a project's persistent save destination.
    static func stageImport(_ source: URL) throws -> URL {
        guard importExtensions.contains(source.pathExtension.lowercased()) else { throw failure("unsupported_import") }
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent("SolidCraft-Import-" + UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let destination = directory.appendingPathComponent(source.lastPathComponent)
        do {
            let access = source.startAccessingSecurityScopedResource()
            defer { if access { source.stopAccessingSecurityScopedResource() } }
            var coordinationError: NSError?
            var copyResult: Result<Void, Error> = .failure(failure("import_failed"))
            NSFileCoordinator().coordinate(readingItemAt: source, options: [], error: &coordinationError) { readable in
                copyResult = Result {
                    let values = try readable.resourceValues(forKeys: [.fileSizeKey, .isRegularFileKey, .isSymbolicLinkKey])
                    guard values.isRegularFile == true, values.isSymbolicLink != true,
                          let size = values.fileSize, size > 0, Int64(size) <= maximumImportBytes else { throw failure("import_size") }
                    let input = try FileHandle(forReadingFrom: readable)
                    defer { try? input.close() }
                    guard FileManager.default.createFile(atPath: destination.path, contents: nil) else { throw failure("import_failed") }
                    let output = try FileHandle(forWritingTo: destination)
                    defer { try? output.close() }
                    var copied: Int64 = 0
                    while let chunk = try input.read(upToCount: 64 * 1024), !chunk.isEmpty {
                        copied += Int64(chunk.count)
                        guard copied <= maximumImportBytes else { throw failure("import_size") }
                        try output.write(contentsOf: chunk)
                    }
                    guard copied > 0 else { throw failure("import_size") }
                    try output.synchronize()
                }
            }
            if let coordinationError { throw coordinationError }
            try copyResult.get()
            return destination
        } catch {
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
    }
}
