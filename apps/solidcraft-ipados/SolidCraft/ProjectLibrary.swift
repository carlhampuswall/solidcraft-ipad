import Foundation

/// Native file organisation only. Each design remains the upstream native
/// container; directory UUIDs keep equally named imports from overwriting work.
struct ProjectRecord: Equatable {
    let id: UUID
    let file: URL
    let modified: Date
    var name: String { file.deletingPathExtension().lastPathComponent }
}

struct ProjectLibrary {
    let root: URL
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
            return ProjectRecord(id: id, file: file, modified: values.contentModificationDate ?? .distantPast)
        }.sorted { $0.modified == $1.modified ? $0.id.uuidString < $1.id.uuidString : $0.modified > $1.modified }
    }

    func reserve(name: String) throws -> ProjectRecord {
        let name = try Self.validName(name)
        let id = UUID()
        let directory = root.appendingPathComponent(id.uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        return ProjectRecord(id: id, file: directory.appendingPathComponent(name).appendingPathExtension("solidcraft"), modified: Date())
    }

    func remove(_ project: ProjectRecord) throws {
        // Re-derive every destructive path from the library and checked UUID.
        let directory = root.appendingPathComponent(project.id.uuidString, isDirectory: true)
        guard project.file.deletingLastPathComponent().standardizedFileURL == directory.standardizedFileURL else { throw Self.failure("invalid_project") }
        try FileManager.default.removeItem(at: directory)
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
