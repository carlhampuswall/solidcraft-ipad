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

/// References to existing, original SolidCraft artwork, never user-supplied paths.
enum ProjectFolderIcon: String, Codable, CaseIterable {
    case folder, project, library, box, component
}

struct ProjectFolder: Codable, Equatable {
    let id: UUID
    var name: String
    var parent: UUID?
    var icon: ProjectFolderIcon = .folder
    var archived = false
}

struct ProjectPlacement: Codable, Equatable {
    var folder: UUID?
    var archived = false
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
            guard try ProjectLibrary.validName(value.name) == value.name,
                  value.parent.map({ ids.contains($0) }) ?? true else { throw ProjectLibrary.failure("invalid_organisation") }
            var visited: Set<UUID> = [value.id]
            var next = value.parent
            while let id = next {
                guard visited.count <= 64, visited.insert(id).inserted else { throw ProjectLibrary.failure("folder_cycle") }
                next = index[id]?.parent
            }
        }
        guard placements.allSatisfy({ UUID(uuidString: $0.key)?.uuidString == $0.key && ($0.value.folder.map { ids.contains($0) } ?? true) }),
              sorts.keys.allSatisfy({ $0 == "root" || $0 == "archive" || UUID(uuidString: $0).map { ids.contains($0) } == true }) else {
            throw ProjectLibrary.failure("invalid_organisation")
        }
    }
}

enum ProjectLibraryChange {
    case createFolder(String, parent: UUID?)
    case renameFolder(UUID, String)
    case folderIcon(UUID, ProjectFolderIcon)
    case moveFolder(UUID, to: UUID?)
    case moveProject(UUID, to: UUID?)
    case archiveFolder(UUID, Bool)
    case archiveProject(UUID, Bool)
    case sort(ProjectSort, folder: UUID?, archive: Bool)
}

struct ProjectLibraryContents {
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
