import Foundation
import QuartzCore

struct BridgeError: Error, CustomStringConvertible {
    let code: String
    private let sourceMessage: String
    init(code: String, message: String) { self.code = code; sourceMessage = message }
    var message: String { sourceMessage.hasPrefix("ui.ipados.") ? NativeText.value(sourceMessage) : sourceMessage }
    var description: String { "\(code): \(message)" }
}

/// Read-only presentation data from the authoritative Rust session. These values
/// never become a second document model or a source of modelling semantics.
struct WorkspaceState: Decodable {
    let title: String
    let document_id: String
    let revision: UInt64
    let geometry_revision: UInt64
    let can_undo: Bool
    let can_redo: Bool
    let dirty: Bool?
    let rows: [WorkspaceRow]
    let tools: [WorkspaceTool]
    let active_tool: String
    let selection: [WorkspaceEntity]
    let selection_edge_length_mm: Double?
    let radius: WorkspaceRadius?
    let sketch: WorkspaceSketch?
    let planes: [WorkspacePlane]?
    let modelling: ModellingPresentation?
    let assembly: AssemblyPresentation?
}

struct WorkspacePlane: Decodable { let name: String }

/// Presentation-only projection of the native sketch inspection/selection APIs.
struct WorkspaceSketch: Decodable {
    let id: UInt64
    let name: String
    let status: String
    let solved: Bool
    let dof: Int
    let fully_constrained: Bool
    let failing: [String]
    let constraints: [WorkspaceConstraint]
    let eligible_constraints: [WorkspaceConstraintAction]?
    let eligible_dimensions: [String]?
    let profiles: [WorkspaceProfile]
    let curves: [WorkspaceSketchCurve]
    let report: Report?
    struct Report: Decodable { let folded: [String]; let oversized: Int? }
}
struct WorkspaceSketchCurve: Decodable { let id: String; let type: String }
struct WorkspaceProfile: Decodable { let index: Int; let area_mm2: Double }
struct WorkspaceConstraint: Decodable {
    let id: String
    let name: String
    let dimension: Bool
    let driven: Bool
    let param: String?
    let expression: String?
    let entities: [String]
}
struct WorkspaceConstraintAction: Decodable {
    let command: String
    let params: [String: PresentationJSON]
    var dictionary: [String: Any] { params.mapValues(\.value) }
}
typealias ModellingState = ModellingResponse<WorkspaceState>

struct SketchInputPresentation: Decodable, Equatable {
    struct Snap: Decodable, Equatable { let kind: String; let at: [Double]; let persistent: Bool }
    struct Feedback: Decodable, Equatable { let width: Double; let height: Double; let radius: Double }
    let prompt: String
    let snap: Snap?
    let snapping: Bool
    let auto_constraints: Bool
    let tool: String?
    let feedback: Feedback?
    let finish_chain_at: [Double]?
}
struct SketchAnnotationPresentation: Decodable {
    let id: String
    let points: [Double]
    let value: Double
    let units: String
    let driven: Bool
    let selected: Bool
    let entity: WorkspaceEntity
    let dimension_type: String?
}
struct SketchConstraintIndicator: Decodable { let id: String; let name: String; let points: [Double]; let selected: Bool }
struct ReferenceLabelPresentation: Decodable {
    let name: String
    let entity: WorkspaceEntity
    let position: [Double]
    let selected: Bool
}
struct SketchDependents: Decodable { let deleted_with_it: [String]; let would_fail: [String] }

struct WorkspaceRow: Decodable {
    let id: String
    let parent_id: String?
    let label: String
    let kind: String
    let entity: WorkspaceEntity?
    let selected: Bool
    let assembly: AssemblyRowPresentation?
    let browser: BrowserRowPresentation?
}

struct BrowserRowPresentation: Decodable {
    struct Command: Decodable {
        let command: String
        let params: [String: PresentationJSON]
    }
    let icon: String
    let localised: Bool?
    let container: Bool?
    let visible: Bool?
    let drawn: Bool?
    let visibility: [String: PresentationJSON]?
    let rename: Command?
    let component: UInt64?
    let folder: String?
    let group: UInt64?
    let key: String?
}

struct WorkspaceTool: Decodable {
    let id: String
    let enabled: Bool
    let reason: String?
}

struct WorkspaceRadius: Decodable {
    let feature: UInt64
    let expression: String
    let value: Double
    let units: String
}

struct NumericState: Decodable {
    let token: UInt64?
    let expression: String?
    let value: Double?
    let units: String
    let preview: Bool
    let committed: Bool
    let workspace: WorkspaceState
    let viewport: NumericViewport?
}

struct NumericViewport: Decodable {
    struct Handle: Decodable { let radius: Double }
    let volume_mm3: Double?
    let preview_volume_mm3: Double?
    let handle: Handle?
}

/// The existing engine selection schema, retained only for rendering and sending
/// a browser row back through the viewport's single selection path.
struct WorkspaceEntity: Decodable, Equatable {
    enum Identifier: Decodable, Equatable {
        case number(UInt64)
        case text(String)
        init(from decoder: Decoder) throws {
            let value = try decoder.singleValueContainer()
            if let number = try? value.decode(UInt64.self) { self = .number(number) }
            else { self = .text(try value.decode(String.self)) }
        }
        var value: Any {
            switch self { case .number(let value): return value; case .text(let value): return value }
        }
        var readout: String {
            switch self { case .number(let value): return String(value); case .text(let value): return value }
        }
    }
    let type: String
    let name: String?
    let body: String?
    let index: Int?
    let point: [Double]?
    let id: Identifier?
    let path: [UInt64]?
    let sketch: UInt64?
    var dictionary: [String: Any] {
        var value: [String: Any] = ["type": type]
        if let name { value["name"] = name }
        if let body { value["body"] = body }
        if let index { value["index"] = index }
        if let point { value["point"] = point }
        if let id { value["id"] = id.value }
        if let path { value["path"] = path }
        if let sketch { value["sketch"] = sketch }
        return value
    }
    var readout: String {
        let identity = index.map(String.init) ?? id?.readout ?? name ?? path?.map(String.init).joined(separator: "/") ?? ""
        return [type, body ?? name ?? "", identity].filter { !$0.isEmpty }.joined(separator: " · ")
    }
}

/// Immutable handle, one CAD worker, one input/render worker. Only surface calls enter
/// LifecycleGate; the main-thread background suspension cannot wait for CAD evaluation.
final class NativeSession {
    var onFailure: ((BridgeError) -> Void)?
    var onFrame: ((Bool, UInt64?, UInt64?, UInt64?, String?, [Double]?, Bool, Bool, String) -> Void)?
    var onStatus: ((String) -> Void)?
    var onWorkspace: ((WorkspaceState) -> Void)?
    var onProjectChange: ((ProjectRecord) -> Void)?
    private(set) var currentProject: ProjectRecord?
    private var projectBusy = false
    private var autosave: DispatchWorkItem?
    private var autosaveFailure: (document: String, revision: UInt64)?
    private let fileExport = DispatchQueue(label: "SolidCraft.export", qos: .userInitiated, autoreleaseFrequency: .workItem)
    private static let selectedProjectKey = "SolidCraft.projects.selected"
    var onSketchInput: ((SketchInputPresentation?) -> Void)?
    var onSketchAnnotations: (([SketchAnnotationPresentation]) -> Void)?
    var onConstraintIndicators: (([SketchConstraintIndicator]) -> Void)?
    var onReferenceLabels: (([ReferenceLabelPresentation]) -> Void)?
    var onViewPresentation: ((ViewControlsPresentation) -> Void)?
    var onModelValue: ((ModelValuePresentation?) -> Void)?
    var onToolWorkCompleted: (() -> Void)?
    var onModelDismissed: ((UInt64) -> Void)?
    private let engine = DispatchQueue(label: "SolidCraft.engine", qos: .userInitiated, autoreleaseFrequency: .workItem)
    private let display = DispatchQueue(label: "SolidCraft.viewport", qos: .userInteractive, autoreleaseFrequency: .workItem)
    private let state = BridgeState()
    private let lifecycle = LifecycleGate()
    private var alive = true // main only

    func start(completion: @escaping (Result<String, BridgeError>) -> Void) {
        engine.async { [weak self, state] in
            let result = Result { () throws -> (String, ProjectRecord?) in
                // Diagnostic drivers retain their disposable fixture and never
                // read, replace or autosave a person's library project.
                let diagnostic = ProcessInfo.processInfo.arguments.contains { $0.hasPrefix("--") && $0.contains("-check") }
                if diagnostic {
                    let data = try state.request(Data("{\"kind\":\"fixture\"}".utf8), lane: .engine)
                    return (String(decoding: data, as: UTF8.self), nil)
                }
                let library = try ProjectLibrary.applicationLibrary()
                let projects = try library.projects()
                let selected = UserDefaults.standard.string(forKey: Self.selectedProjectKey)
                let restore = UserDefaults.standard.object(forKey: "SolidCraft.projects.restoreLast") as? Bool ?? true
                if restore, let project = projects.first(where: { $0.id.uuidString == selected }) ?? projects.first {
                    let data = try Self.replaceProject(state, source: project.file, project: project)
                    return (String(decoding: data, as: UTF8.self), project)
                }
                let support = library.root.deletingLastPathComponent()
                let legacy = support.appendingPathComponent("Modelling.solidcraft")
                let migrateLegacy = restore && FileManager.default.fileExists(atPath: legacy.path)
                let project = try library.reserve(name: migrateLegacy ? NativeText.value("ui.ipados.projects.recovered") : NativeText.value("ui.ipados.projects.starter"))
                do {
                    let data: Data
                    if migrateLegacy {
                        data = try Self.replaceProject(state, source: legacy, project: project)
                    } else {
                        _ = try state.request(Data("{\"kind\":\"fixture\"}".utf8), lane: .engine)
                        data = try Self.fileCommand(state, "file.save_as", params: ["path": project.file.path])
                    }
                    return (String(decoding: data, as: UTF8.self), project)
                } catch { try? library.remove(project); throw error }
            }.mapError(BridgeState.error)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive else { return }
                if case .success(let value) = result, let project = value.1 { self.selectProject(project) }
                completion(result.map { $0.0 })
                self.refreshWorkspace()
                self.redraw()
            }
        }
    }

    func command(_ command: String, params: [String: Any] = [:],
                 completion: ((Result<Void, BridgeError>) -> Void)? = nil) {
        guard alive else { return }
        let isView = command.hasPrefix("view.") || command.hasPrefix("select.") || command.hasPrefix("tool.") || command == "input.cancel"
        let lane: BridgeState.Lane = isView ? .viewport : .engine
        let queue = isView ? display : engine
        let activity = isView ? nil : UUID()
        do {
            let data = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": command, "params": params])
            if let activity { commandActivities.insert(activity); publishActivityStatus() }
            queue.async { [weak self, state] in
                do {
                    let reply = try state.request(data, lane: lane)
                    let viewPresentation = isView ? (try? JSONDecoder().decode(ViewControlsPresentation.Envelope.self, from: reply).result) : nil
                    // Some viewport controls (Finish spline and saved snapping)
                    // enqueue real CAD work without another Pencil sample.
                    if isView && (command.hasPrefix("tool.") || command.hasPrefix("select.")) { self?.drainToolWork() }
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive else { return }
                        if let activity { self.commandActivities.remove(activity); self.publishActivityStatus() }
                        if !command.hasPrefix("view.") { self.refreshWorkspace() }
                        if let viewPresentation { self.onViewPresentation?(viewPresentation) }
                        completion?(.success(()))
                        self.redraw()
                    }
                } catch {
                    let failure = BridgeState.error(error)
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive else { return }
                        if let activity { self.commandActivities.remove(activity); self.publishActivityStatus() }
                        self.onFailure?(failure)
                        completion?(.failure(failure))
                    }
                }
            }
        } catch {
            let failure = BridgeState.error(error)
            onFailure?(failure)
            completion?(.failure(failure))
        }
    }

    // Main-thread presentation only. One delayed reveal avoids flashing status
    // during a fast Pencil preview; no progress callback requests a render frame.
    private var commandActivities: Set<UUID> = []
    private var toolActivityTicket: UInt64?
    private var toolActivityToken: UUID?
    private var toolActivityVisible = false
    private var publishedActivityStatus = ""

    private func publishActivityStatus() {
        let busy = lifecycle.ticket != nil && (!commandActivities.isEmpty || toolActivityVisible)
        let message = busy ? NativeText.value("ui.ipados.updating") : ""
        guard message != publishedActivityStatus else { return }
        publishedActivityStatus = message
        onStatus?(message)
    }

    private func beginToolActivity(ticket: UInt64) {
        guard alive, lifecycle.accepts(ticket), toolActivityTicket != ticket else { return }
        toolActivityTicket = ticket
        let token = UUID()
        toolActivityToken = token
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) { [weak self] in
            guard let self, self.alive, self.lifecycle.accepts(ticket), self.toolActivityToken == token else { return }
            self.toolActivityVisible = true
            self.publishActivityStatus()
        }
    }

    private func endToolActivity(ticket: UInt64) {
        guard toolActivityTicket == ticket else { return }
        toolActivityTicket = nil
        toolActivityToken = nil
        toolActivityVisible = false
        publishActivityStatus()
    }

    /// Main-thread request coalescing bounds presentation reads. Rust derives the
    /// tree only when document geometry changes; ordinary navigation never requests it.
    private var workspacePending = false
    private var workspaceAgain = false
    private var numericGeneration: UInt64 = 0 // main; invalidate old editor callbacks on begin/cancel/suspend
    private var modellingGeneration: UInt64 = 0
    private var lastWorkspaceDocument: String?
    private var lastWorkspaceRevision: UInt64?
    private var lastWorkspaceSelection: [WorkspaceEntity]?
    private var lastWorkspaceTool: String?

    func refreshWorkspace() {
        guard alive, let ticket = lifecycle.ticket else { return }
        if workspacePending { workspaceAgain = true; return }
        workspacePending = true
        let expected = numericGeneration
        engine.async { [weak self, state, lifecycle] in
            guard lifecycle.accepts(ticket) else {
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    self.workspacePending = false
                    if self.workspaceAgain { self.workspaceAgain = false; self.refreshWorkspace() }
                }
                return
            }
            let result = Result { () throws -> WorkspaceState in
                let data = try state.request(Data("{\"kind\":\"workspace\"}".utf8), lane: .engine)
                return try JSONDecoder().decode(WorkspaceEnvelope.self, from: data).result
            }.mapError(BridgeState.error)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive else { return }
                self.workspacePending = false
                if lifecycle.accepts(ticket), self.numericGeneration == expected {
                    switch result {
                    case .success(let workspace): self.onWorkspace?(workspace); self.scheduleAutosave(workspace)
                    case .failure(let failure): self.onFailure?(failure)
                    }
                }
                if self.workspaceAgain { self.workspaceAgain = false; self.refreshWorkspace() }
            }
        }
    }

    private struct WorkspaceEnvelope: Decodable { let result: WorkspaceState }
    private struct NumericEnvelope: Decodable { let result: NumericState }
    private struct ModellingEnvelope: Decodable { let result: ModellingState }

    private static func fileCommand(_ state: BridgeState, _ command: String, params: [String: Any]) throws -> Data {
        try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": command, "params": params]), lane: .engine)
    }

    private static func replaceProject(_ state: BridgeState, source: URL?, project: ProjectRecord) throws -> Data {
        var request: [String: Any] = ["kind": "document", "name": project.name, "destination": project.file.path]
        if let source { request["source"] = source.path }
        return try state.request(JSONSerialization.data(withJSONObject: request), lane: .engine)
    }

    private func selectProject(_ project: ProjectRecord) {
        currentProject = project
        autosaveFailure = nil
        UserDefaults.standard.set(project.id.uuidString, forKey: Self.selectedProjectKey)
        onProjectChange?(project)
    }

    private func projectTask<T>(_ operation: @escaping (BridgeState, ProjectLibrary, ProjectRecord?) throws -> T,
                                completion: @escaping (Result<T, BridgeError>) -> Void) {
        guard alive, !projectBusy else {
            completion(.failure(BridgeError(code: "busy", message: "ui.ipados.projects.busy"))); return
        }
        autosave?.cancel(); autosave = nil
        projectBusy = true
        let current = currentProject
        let activity = UUID(); commandActivities.insert(activity); publishActivityStatus()
        engine.async { [weak self, state] in
            let result = Result { try operation(state, ProjectLibrary.applicationLibrary(), current) }.mapError(BridgeState.error)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive else { return }
                self.projectBusy = false
                self.commandActivities.remove(activity); self.publishActivityStatus()
                completion(result)
                self.refreshWorkspace(); self.redraw()
            }
        }
    }

    private static func saveProject(_ state: BridgeState, _ project: ProjectRecord?) throws {
        if let project {
            _ = try state.request(JSONSerialization.data(withJSONObject: ["kind": "save_document", "path": project.file.path]), lane: .engine)
        }
    }

    func listProjects(completion: @escaping (Result<[ProjectRecord], BridgeError>) -> Void) {
        projectTask({ _, library, _ in try library.projects() }, completion: completion)
    }

    func saveProject(completion: @escaping (Result<Void, BridgeError>) -> Void) {
        projectTask({ state, _, current in
            guard current != nil else { throw ProjectLibrary.failure("invalid_project") }
            try Self.saveProject(state, current)
        }, completion: completion)
    }

    /// Save first; never discard a dirty project merely to browse another one.
    func openProject(_ project: ProjectRecord, completion: @escaping (Result<Void, BridgeError>) -> Void) {
        projectTask({ state, library, current in
            guard try library.projects().contains(where: { $0.id == project.id && $0.file == project.file }) else { throw ProjectLibrary.failure("invalid_project") }
            try Self.saveProject(state, current)
            if current?.id != project.id { _ = try Self.replaceProject(state, source: project.file, project: project) }
            return project
        }) { [weak self] result in
            if case .success(let project) = result { self?.selectProject(project) }
            completion(result.map { _ in () })
        }
    }

    func createProject(name: String, copying source: ProjectRecord? = nil, completion: @escaping (Result<Void, BridgeError>) -> Void) {
        projectTask({ state, library, current in
            try Self.saveProject(state, current)
            let project = try library.reserve(name: name)
            do { _ = try Self.replaceProject(state, source: source?.file, project: project); return project }
            catch { try? library.remove(project); throw error }
        }) { [weak self] result in
            if case .success(let project) = result { self?.selectProject(project) }
            completion(result.map { _ in () })
        }
    }

    func deleteProject(_ project: ProjectRecord, completion: @escaping (Result<Void, BridgeError>) -> Void) {
        projectTask({ _, library, current in
            guard current?.id != project.id else { throw ProjectLibrary.failure("delete_current") }
            try library.remove(project)
        }, completion: completion)
    }

    func importProject(from source: URL, insert: Bool, completion: @escaping (Result<[String], BridgeError>) -> Void) {
        projectTask({ state, library, current in
            try Self.saveProject(state, current)
            let staged = try ProjectLibrary.stageImport(source)
            defer { try? FileManager.default.removeItem(at: staged.deletingLastPathComponent()) }
            let data: Data
            var created: ProjectRecord?
            if insert {
                guard !ProjectLibrary.nativeExtensions.contains(source.pathExtension.lowercased()) else { throw ProjectLibrary.failure("insert_native") }
                let command = ["step", "stp"].contains(source.pathExtension.lowercased()) ? "file.insert_step" : "file.insert_mesh"
                data = try Self.fileCommand(state, command, params: ["path": staged.path])
                // The import is already an undoable command. A later autosave
                // failure leaves it dirty and visible; it never erases it.
            } else {
                let project = try library.reserve(name: source.deletingPathExtension().lastPathComponent)
                do { data = try Self.replaceProject(state, source: staged, project: project); created = project }
                catch { try? library.remove(project); throw error }
            }
            let json = try JSONSerialization.jsonObject(with: data) as? [String: Any]
            let result = json?["result"] as? [String: Any]
            let command = result?["command"] as? [String: Any]
            let value = (command?["value"] as? [String: Any]) ?? command
            return (created, value?["warnings"] as? [String] ?? [])
        }) { [weak self] result in
            if case .success(let value) = result, let project = value.0 { self?.selectProject(project) }
            completion(result.map { $0.1 })
        }
    }

    func exportProject(format: String, completion: @escaping (Result<URL, BridgeError>) -> Void) {
        projectTask({ state, _, current in
            guard ["solidcraft", "step", "stl", "obj", "3mf"].contains(format) else { throw ProjectLibrary.failure("unsupported_export") }
            try Self.saveProject(state, current)
            guard let current else { throw ProjectLibrary.failure("invalid_project") }
            let directory = FileManager.default.temporaryDirectory.appendingPathComponent("SolidCraft-Export-" + UUID().uuidString, isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            let output = directory.appendingPathComponent(current.name).appendingPathExtension(format)
            // Keep a frozen container next to the project so native relative
            // component links retain their base directory. The export worker
            // owns a disposable engine, never the live document or viewport.
            let frozen = current.file.deletingLastPathComponent().appendingPathComponent(".Export-" + UUID().uuidString + ".solidcraft")
            do {
                try FileManager.default.copyItem(at: current.file, to: format == "solidcraft" ? output : frozen)
                return (output, format == "solidcraft" ? nil : frozen)
            } catch { try? FileManager.default.removeItem(at: directory); throw error }
        }) { [weak self] result in
            guard let self else { return }
            switch result {
            case .failure(let error): completion(.failure(error))
            case .success(let files):
                guard let frozen = files.1 else { completion(.success(files.0)); return }
                let activity = UUID(); self.commandActivities.insert(activity); self.publishActivityStatus()
                self.fileExport.async { [weak self] in
                    let exported = Result { () throws -> URL in
                        let copy = BridgeState()
                        defer { copy.close(); try? FileManager.default.removeItem(at: frozen) }
                        _ = try Self.fileCommand(copy, "doc.open", params: ["path": frozen.path, "read_only": true])
                        _ = try Self.fileCommand(copy, "file.export", params: ["path": files.0.path, "format": format])
                        return files.0
                    }.mapError(BridgeState.error)
                    if case .failure = exported { try? FileManager.default.removeItem(at: files.0.deletingLastPathComponent()) }
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive else {
                            try? FileManager.default.removeItem(at: files.0.deletingLastPathComponent()); return
                        }
                        self.commandActivities.remove(activity); self.publishActivityStatus()
                        completion(exported)
                    }
                }
            }
        }
    }

    private func scheduleAutosave(_ workspace: WorkspaceState) {
        autosave?.cancel(); autosave = nil
        guard workspace.dirty == true, currentProject != nil, !projectBusy,
              autosaveFailure?.document != workspace.document_id || autosaveFailure?.revision != workspace.geometry_revision else { return }
        let work = DispatchWorkItem { [weak self] in
            self?.saveProject { [weak self] result in
                switch result {
                case .failure(let error):
                    self?.autosaveFailure = (workspace.document_id, workspace.geometry_revision)
                    self?.onFailure?(error)
                case .success: self?.autosaveFailure = nil
                }
            }
        }
        autosave = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5, execute: work)
    }

    func requestModelling(action: String, token: UInt64? = nil, operation: String? = nil, feature: UInt64? = nil,
                          params: [String: PresentationJSON]? = nil,
                          completion: @escaping (Result<ModellingState, BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        if action == "begin" || action == "cancel" { modellingGeneration &+= 1 }
        let expected = modellingGeneration
        var request: [String: Any] = ["kind": "model", "action": action]
        if let token { request["token"] = token }
        if let operation { request["operation"] = operation }
        if let feature { request["feature"] = feature }
        if let params { request["params"] = params.mapValues(\.value) }
        do {
            let data = try JSONSerialization.data(withJSONObject: request)
            engine.async { [weak self, state, lifecycle] in
                guard lifecycle.accepts(ticket) else { return }
                let result = Result { () throws -> ModellingState in
                    let reply = try state.request(data, lane: .engine)
                    return try JSONDecoder().decode(ModellingEnvelope.self, from: reply).result
                }.mapError(BridgeState.error)
                guard lifecycle.accepts(ticket) else {
                    if action == "begin", case .success(let model) = result, let lateToken = model.token {
                        do {
                            let cancel = try JSONSerialization.data(withJSONObject: ["kind": "model", "action": "cancel", "token": lateToken])
                            _ = try state.request(cancel, lane: .engine)
                        } catch { NSLog("%@", BridgeState.error(error).description) }
                    }
                    return
                }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, lifecycle.accepts(ticket), self.modellingGeneration == expected else { return }
                    completion(result)
                    guard lifecycle.accepts(ticket), self.modellingGeneration == expected else { return }
                    if case .success(let model) = result { self.onWorkspace?(model.workspace); self.scheduleAutosave(model.workspace) }
                    self.redraw()
                }
            }
        } catch { completion(.failure(BridgeState.error(error))) }
    }

    private func observeWorkspaceContext(_ viewport: BridgeState.ViewportReport) {
        if let view = viewport.viewPresentation { onViewPresentation?(view) }
        if let token = viewport.input?.dismissed_model_token { onModelDismissed?(token) }
        onSketchInput?(viewport.input?.sketch)
        let changed = lastWorkspaceDocument != viewport.document_id
            || lastWorkspaceRevision != viewport.geometry_revision
            || lastWorkspaceSelection != viewport.selection
            || lastWorkspaceTool != viewport.input?.tool
        guard changed else { return }
        lastWorkspaceDocument = viewport.document_id
        lastWorkspaceRevision = viewport.geometry_revision
        lastWorkspaceSelection = viewport.selection
        lastWorkspaceTool = viewport.input?.tool
        refreshWorkspace()
    }

    func selectEntity(_ entity: [String: Any], additive: Bool,
                      completion: ((Result<Void, BridgeError>) -> Void)? = nil) {
        command("select.entity", params: ["entity": entity, "additive": additive], completion: completion)
    }

    /// The host requests automatic previews and explicit commits. Rust owns tokens,
    /// expression parsing, units, stale-selection checks and the single history action.
    /// Validation failure is returned to the field, without replacing its draft.
    func requestNumeric(action: String, token: UInt64? = nil, expression: String? = nil, dimension: String? = nil,
                        completion: @escaping (Result<NumericState, BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        if action == "begin" || action == "cancel" { numericGeneration &+= 1 }
        let expected = numericGeneration
        var request: [String: Any] = ["kind": "numeric", "action": action]
        if let token { request["token"] = token }
        if let expression { request["expression"] = expression }
        if let dimension { request["dimension"] = dimension }
        do {
            let data = try JSONSerialization.data(withJSONObject: request)
            engine.async { [weak self, state, lifecycle] in
                // A queued begin must not recreate an editor after resign-active
                // has synchronously cancelled Rust's current numeric transaction.
                guard lifecycle.accepts(ticket) else { return }
                let result = Result { () throws -> NumericState in
                    let reply = try state.request(data, lane: .engine)
                    return try JSONDecoder().decode(NumericEnvelope.self, from: reply).result
                }.mapError(BridgeState.error)
                guard lifecycle.accepts(ticket) else {
                    if action == "begin", case .success(let numeric) = result, let lateToken = numeric.token {
                        // No lifecycle/render lock is held during this engine call.
                        // A stale token means suspension already performed cleanup.
                        do {
                            let cancel = try JSONSerialization.data(withJSONObject: ["kind": "numeric", "action": "cancel", "token": lateToken])
                            _ = try state.request(cancel, lane: .engine)
                        } catch {
                            let failure = BridgeState.error(error)
                            if failure.code != "stale_edit" { NSLog("%@", failure.description) }
                        }
                    }
                    return
                }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, lifecycle.accepts(ticket), self.numericGeneration == expected else { return }
                    completion(result)
                    // The completion can synchronously cancel or start another edit.
                    guard lifecycle.accepts(ticket), self.numericGeneration == expected else { return }
                    if case .success(let numeric) = result { self.onWorkspace?(numeric.workspace); self.scheduleAutosave(numeric.workspace) }
                    self.redraw()
                }
            }
        } catch { completion(.failure(BridgeState.error(error))) }
    }

    func previewSketchPlane(_ plane: Any, completion: @escaping (Result<Void, BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        do {
            let data = try JSONSerialization.data(withJSONObject: ["kind": "sketch_plane", "plane": plane])
            engine.async { [weak self, state, lifecycle] in
                guard lifecycle.accepts(ticket) else { return }
                let result = Result { _ = try state.request(data, lane: .engine) }.mapError(BridgeState.error)
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, lifecycle.accepts(ticket) else { return }
                    completion(result); self.redraw()
                }
            }
        } catch { completion(.failure(BridgeState.error(error))) }
    }

    func cancelSketchPlane() {
        guard alive else { return }
        engine.async { [weak self, state] in
            do { _ = try state.request(Data("{\"kind\":\"sketch_plane_cancel\"}".utf8), lane: .engine) }
            catch { self?.failure(error) }
            DispatchQueue.main.async { [weak self] in self?.redraw() }
        }
    }

    func sketchDependents(_ id: Any, completion: @escaping (Result<SketchDependents, BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        do {
            let data = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": "timeline.dependents", "params": ["feature": id]])
            engine.async { [weak self, state, lifecycle] in
                guard lifecycle.accepts(ticket) else { return }
                let result = Result { try JSONDecoder().decode(EngineCommandEnvelope<SketchDependents>.self, from: state.request(data, lane: .engine)).result.command.value }.mapError(BridgeState.error)
                DispatchQueue.main.async { [weak self] in guard let self, self.alive, lifecycle.accepts(ticket) else { return }; completion(result) }
            }
        } catch { completion(.failure(BridgeState.error(error))) }
    }

    func evaluateTransformExpressions(_ fields: [(name: String, expression: String, kind: String)],
                                      completion: @escaping (Result<[String: Double], BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        engine.async { [weak self, state, lifecycle] in
            let result = Result { () throws -> [String: Double] in
                var values: [String: Double] = [:]
                for field in fields {
                    guard lifecycle.accepts(ticket) else { throw BridgeError(code: "stale_edit", message: "ui.ipados.workspace.invalidated") }
                    let data = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": "expr.evaluate", "params": ["expression": field.expression, "kind": field.kind]])
                    let evaluated = try JSONDecoder().decode(EngineCommandEnvelope<ExpressionReply>.self, from: state.request(data, lane: .engine)).result.command.value
                    guard evaluated.ok, let value = evaluated.value, value.isFinite else {
                        throw BridgeError(code: "numeric_validation", message: evaluated.error ?? "ui.ipados.workspace.numeric_required")
                    }
                    values[field.name] = value
                }
                return values
            }.mapError(BridgeState.error)
            DispatchQueue.main.async { [weak self] in
                guard let self, self.alive, lifecycle.accepts(ticket) else { return }; completion(result)
            }
        }
    }
    private struct ExpressionReply: Decodable { let ok: Bool; let value: Double?; let error: String? }

    func proposeSketchTransform(command: String, params: [String: Any], commit: Bool,
                                completion: @escaping (Result<Void, BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        do {
            let data = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": "tool.propose", "params": ["command": command, "params": params, "commit": commit]])
            display.async { [weak self, state, lifecycle] in
                guard lifecycle.accepts(ticket) else { return }
                do {
                    _ = try state.request(data, lane: .viewport)
                    self?.engine.async { [weak self, state, lifecycle] in
                        guard lifecycle.accepts(ticket) else { return }
                        let result = Result { () throws -> Void in
                            let data = try state.request(Data("{\"kind\":\"tool_work\"}".utf8), lane: .engine)
                            let work = try JSONDecoder().decode(SketchToolWorkEnvelope.self, from: data).result
                            guard work.accepts(commit: commit) else {
                                throw BridgeError(code: "stale_edit", message: "ui.ipados.workspace.invalidated")
                            }
                        }.mapError(BridgeState.error)
                        DispatchQueue.main.async { [weak self] in
                            guard let self, self.alive, lifecycle.accepts(ticket) else { return }
                            completion(result); if commit { self.refreshWorkspace() }; self.redraw()
                        }
                    }
                } catch {
                    let failure = BridgeState.error(error)
                    DispatchQueue.main.async { completion(.failure(failure)) }
                }
            }
        } catch { completion(.failure(BridgeState.error(error))) }
    }

    func present(layer: CAMetalLayer, width: UInt32, height: UInt32, scale: Double) {
        guard alive, let ticket = lifecycle.ticket else { return }
        display.async { [weak self, state, lifecycle] in
            do {
                let data = try JSONSerialization.data(withJSONObject: ["kind": "viewport", "metrics": [
                    "width_points": Double(width) / scale, "height_points": Double(height) / scale, "scale": scale
                ]])
                _ = try state.request(data, lane: .viewport)
                _ = try lifecycle.rendering(ticket: ticket, state: state) {
                    try state.present(layer: layer, width: width, height: height)
                }
                DispatchQueue.main.async { [weak self] in self?.redraw() }
            } catch { self?.failure(error) }
        }
    }

    private let input = InputMailbox()
    private var toolBusy = false // display queue; at most one queued/running CAD job

    func record(_ sample: InputSample) {
        guard alive, let ticket = lifecycle.ticket else { return }
        guard let inputToken = input.append(sample) else { return }
        display.async { [weak self, state, lifecycle, input] in
            guard let batch = input.take(token: inputToken) else { return }
            guard lifecycle.accepts(ticket) else { return }
            do {
                if batch.cancelled { _ = try state.request(Data("{\"kind\":\"command\",\"command\":\"input.cancel\",\"params\":{}}".utf8), lane: .viewport) }
                let events = try JSONEncoder().encode(batch.samples)
                _ = try state.request(Data("{\"kind\":\"input_batch\",\"events\":".utf8) + events + Data("}".utf8), lane: .viewport)
                self?.drainToolWork()
                DispatchQueue.main.async { [weak self] in self?.redraw() }
            } catch { self?.cancelInput(); self?.failure(error) }
        }
    }

    /// Runs on display. The Rust mailbox replaces pending previews, while this gate
    /// bounds work on the engine queue. Cancellation invalidates in-flight tickets.
    private func drainToolWork() {
        guard !toolBusy, let ticket = lifecycle.ticket else { return }
        do {
            let data = try state.request(Data("{\"kind\":\"snapshot\"}".utf8), lane: .viewport)
            guard try JSONDecoder().decode(ToolPending.self, from: data).result.tool_pending else { return }
            toolBusy = true
            DispatchQueue.main.async { [weak self] in self?.beginToolActivity(ticket: ticket) }
            engine.async { [weak self, state, lifecycle] in
                var failure: BridgeError?
                if lifecycle.accepts(ticket) {
                    do { _ = try state.request(Data("{\"kind\":\"tool_work\"}".utf8), lane: .engine) }
                    catch { failure = BridgeState.error(error) }
                }
                let completedFailure = failure
                self?.display.async { [weak self] in
                    guard let self else { return }
                    self.toolBusy = false
                    self.drainToolWork()
                    let stillBusy = self.toolBusy
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive, lifecycle.accepts(ticket) else { return }
                        if !stillBusy { self.endToolActivity(ticket: ticket) }
                        if let completedFailure { self.onFailure?(completedFailure) }
                        self.onToolWorkCompleted?()
                        self.redraw()
                    }
                }
            }
        } catch { failure(error) }
    }

    private struct ToolPending: Decodable {
        struct Pending: Decodable { let tool_pending: Bool }
        let result: Pending
    }

    func cancelInput() {
        input.clear()
        display.async { [weak self, state] in
            do { _ = try state.request(Data("{\"kind\":\"command\",\"command\":\"input.cancel\",\"params\":{}}".utf8), lane: .viewport) }
            catch { self?.failure(error) }
            DispatchQueue.main.async { [weak self] in self?.redraw() }
        }
    }

    func activate() { lifecycle.activate(); publishActivityStatus() }

    func suspend() {
        autosave?.cancel(); autosave = nil
        if let project = currentProject, !projectBusy {
            engine.async { [weak self, state] in
                do { try Self.saveProject(state, project) }
                catch {
                    let failure = BridgeState.error(error)
                    DispatchQueue.main.async { [weak self] in if self?.alive == true { self?.onFailure?(failure) } }
                }
            }
        }
        input.clear()
        numericGeneration &+= 1
        modellingGeneration &+= 1
        workspaceAgain = false
        lastWorkspaceDocument = nil
        lastWorkspaceRevision = nil
        lastWorkspaceSelection = nil
        lastWorkspaceTool = nil
        toolActivityTicket = nil
        toolActivityToken = nil
        toolActivityVisible = false
        let result = Result { try lifecycle.suspend { try state.suspend() } }
        publishActivityStatus()
        if case .failure(let error) = result, alive { onFailure?(BridgeState.error(error)) }
    }

    func stop() {
        guard alive else { return }
        suspend()
        alive = false
        onFailure = nil
        onFrame = nil
        onStatus = nil
        onWorkspace = nil; onProjectChange = nil
        onModelDismissed = nil
        onSketchInput = nil; onSketchAnnotations = nil; onConstraintIndicators = nil; onReferenceLabels = nil; onViewPresentation = nil; onModelValue = nil
        // Engine work finishes before the display queue releases the retained layer/handle.
        engine.async { [state, lifecycle, display] in
            display.async { lifecycle.exclusive { state.close() } }
        }
    }

    private var redrawAgain = false
    private var redrawPending = false // main only; coalesce bursty touch callbacks
    private func redraw(retries: Int = 2) {
        guard alive, let ticket = lifecycle.ticket else { return }
        if redrawPending { redrawAgain = true; return }
        redrawPending = true
        display.async { [weak self, state, lifecycle] in
            do {
                let frame = try lifecycle.rendering(ticket: ticket, state: state) { try state.redraw() }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    self.redrawPending = false
                    guard self.lifecycle.accepts(ticket), let frame, let frame else { return }
                    if let viewport = frame.viewport {
                        self.observeWorkspaceContext(viewport)
                    }
                    self.onFrame?(frame.presented == true, frame.geometry_revision, frame.viewport?.viewport_epoch, frame.viewport?.camera_epoch, frame.viewport?.document_id, frame.viewport?.metrics.map { [$0.width_points, $0.height_points, $0.scale] }, frame.viewport?.preview == true, frame.viewport?.preview_stale == true, frame.viewport?.readout ?? "")
                    if frame.presented == true {
                        self.onSketchAnnotations?(frame.viewport?.annotations ?? [])
                        self.onConstraintIndicators?(frame.viewport?.constraint_indicators ?? [])
                        self.onReferenceLabels?(frame.viewport?.reference_labels ?? [])
                        self.onModelValue?(frame.viewport?.model_value)
                    }
                    #if DEBUG
                    if frame.presented == true, let next = self.sketchCheckAfterFrame {
                        self.sketchCheckAfterFrame = nil; next()
                    }
                    if frame.presented == true, let viewport = frame.viewport, let check = self.selectionCheckFrame,
                       viewport.document_id == check.report.document_id,
                       viewport.geometry_revision == check.report.geometry_revision,
                       viewport.viewport_epoch == check.report.viewport_epoch,
                       viewport.camera_epoch == check.report.camera_epoch,
                       viewport.selection == check.report.selection {
                        self.selectionCheckFrame = nil
                        check.completion(.success(viewport.selection))
                    }
                    #endif
                    if self.redrawAgain { self.redrawAgain = false; self.redraw() }
                    if frame.presented != true, retries > 0 {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.08) { [weak self] in self?.redraw(retries: retries - 1) }
                    }
                }
            } catch {
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    self.redrawPending = false
                    self.onFailure?(BridgeState.error(error))
                    // Bounded resource recreation; the CAD handle and immutable scene survive.
                    if retries > 0, self.lifecycle.accepts(ticket) {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { [weak self] in self?.redraw(retries: retries - 1) }
                    }
                }
            }
        }
    }

    #if DEBUG
    private var sketchCheckAfterFrame: (() -> Void)?
    private var sketchCheckToken: UInt64?
    private var sketchCheckDimension: WorkspaceConstraint?
    private var sketchCheckCompletion: ((WorkspaceState) -> Void)?
    private var selectionCheckGeneration: UInt64 = 0
    private var selectionCheckFrame: (report: BridgeState.ViewportReport, completion: (Result<[WorkspaceEntity], BridgeError>) -> Void)?

    /// Synthetic finger contacts in the production native bridge. Completion
    /// waits for the exact resulting frame; this is not physical touch input.
    func developmentTwist(_ angle: Double, enabled: Bool, completion: @escaping (Result<Double, BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else { return }
        display.async { [weak self, state, lifecycle] in
            do {
                func snapshot() throws -> (BridgeState.ViewportReport, [String: Any]) {
                    let data = try state.request(Data("{\"kind\":\"snapshot\"}".utf8), lane: .viewport)
                    guard let envelope = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                          let result = envelope["result"] as? [String: Any] else {
                        throw BridgeError(code: "twist_check", message: "Missing snapshot")
                    }
                    return (try JSONDecoder().decode(DevelopmentViewport.self, from: data).result, result)
                }
                let (before, beforeJSON) = try snapshot()
                guard let metrics = before.metrics, let roll = before.camera?.roll else {
                    throw BridgeError(code: "twist_check", message: "Missing camera/metrics")
                }
                let x = metrics.width_points / 2, y = metrics.height_points / 2
                let events: [(InputSample.Phase, UInt64, Double, Double)] = [
                    (.began, 9200, x - 100, y), (.began, 9201, x + 100, y),
                    (.moved, 9201, x - 100 + 200 * cos(angle), y + 200 * sin(angle)),
                    (.ended, 9201, x - 100 + 200 * cos(angle), y + 200 * sin(angle)), (.ended, 9200, x - 100, y)
                ]
                for (index, value) in events.enumerated() {
                    let (stamp, _) = try snapshot()
                    var sample = InputSample(source: .finger, phase: value.0, id: value.1,
                        x_points: value.2, y_points: value.3, x_pixels: value.2 * metrics.scale, y_pixels: value.3 * metrics.scale,
                        scale: metrics.scale, timestamp: ProcessInfo.processInfo.systemUptime + Double(index) * 0.01,
                        pressure: nil, altitude: nil, geometry_revision: stamp.geometry_revision, consumed: false, shift: false, button: 0, scroll: 0)
                    sample.viewport_epoch = stamp.viewport_epoch; sample.camera_epoch = stamp.camera_epoch; sample.document_id = stamp.document_id
                    sample.display_preview = stamp.preview ?? false
                    _ = try state.request(Data("{\"kind\":\"input\",\"event\":".utf8) + JSONEncoder().encode(sample) + Data("}".utf8), lane: .viewport)
                }
                let (after, afterJSON) = try snapshot()
                let delta = (after.camera?.roll ?? .nan) - roll
                let input = afterJSON["input"] as? [String: Any]
                guard abs(delta - (enabled ? angle : 0)) < 0.000001,
                      before.document_id == after.document_id, before.geometry_revision == after.geometry_revision,
                      before.selection == after.selection, input?["contacts"] as? Int == 0,
                      (beforeJSON["volume_mm3"] as? Double) == (afterJSON["volume_mm3"] as? Double) else {
                    throw BridgeError(code: "twist_check", message: "Angle, document, selection or contact cleanup failed: \(delta)")
                }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, lifecycle.accepts(ticket) else { return }
                    self.selectionCheckFrame = (after, { result in completion(result.map { _ in delta }) })
                    self.redraw()
                }
            } catch {
                let error = BridgeState.error(error)
                DispatchQueue.main.async { completion(.failure(error)) }
            }
        }
    }

    /// Synthetic actual samples through the production mailbox, followed by a
    /// matching presented frame. This never claims physical Pencil delivery.
    func developmentSelectionTap(_ world: [Double]?, completion: @escaping (Result<[WorkspaceEntity], BridgeError>) -> Void) {
        guard alive, let ticket = lifecycle.ticket else {
            completion(.failure(BridgeError(code: "selection_taps_check", message: "Viewport is inactive"))); return
        }
        let expected = selectionCheckGeneration
        display.async { [weak self, state, lifecycle] in
            do {
                let report = try JSONDecoder().decode(DevelopmentViewport.self, from: state.request(Data("{\"kind\":\"snapshot\"}".utf8), lane: .viewport)).result
                guard let metrics = report.metrics else { throw BridgeError(code: "selection_taps_check", message: "Viewport metrics are missing") }
                var point = [30.0, 30.0]
                if let world {
                    let data = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": "view.project", "params": ["point": world]])
                    point = try JSONDecoder().decode(DevelopmentProjection.self, from: state.request(data, lane: .viewport)).result.points
                }
                guard point.count == 2 else { throw BridgeError(code: "selection_taps_check", message: "Face projection is missing") }
                let target = point
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive, lifecycle.accepts(ticket), self.selectionCheckGeneration == expected else { return }
                    let contact = UInt64(9100) + self.selectionCheckGeneration
                    let time = ProcessInfo.processInfo.systemUptime
                    let samples: [(InputSample.Phase, UInt64, Double, Double)] = [
                        (.hover, 0, -16, -12), (.hover, 0, 0, 0), (.cancelled, 0, 0, 0),
                        (.began, contact, 0, 0), (.moved, contact, 6, 0), (.ended, contact, 8, 0)
                    ]
                    for (index, value) in samples.enumerated() {
                        let x = target[0] + value.2, y = target[1] + value.3
                        var sample = InputSample(source: .pencil, phase: value.0, id: value.1,
                            x_points: x, y_points: y, x_pixels: x * metrics.scale, y_pixels: y * metrics.scale,
                            scale: metrics.scale, timestamp: time + Double(index) * 0.001,
                            pressure: nil, altitude: nil, geometry_revision: report.geometry_revision,
                            consumed: false, shift: false, button: 0, scroll: 0)
                        sample.viewport_epoch = report.viewport_epoch; sample.camera_epoch = report.camera_epoch
                        sample.document_id = report.document_id; sample.display_preview = report.preview ?? false
                        self.record(sample)
                    }
                    self.display.async { [weak self, state] in
                        let result = Result {
                            try JSONDecoder().decode(DevelopmentViewport.self, from: state.request(Data("{\"kind\":\"snapshot\"}".utf8), lane: .viewport)).result
                        }.mapError(BridgeState.error)
                        DispatchQueue.main.async { [weak self] in
                            guard let self, self.alive, lifecycle.accepts(ticket), self.selectionCheckGeneration == expected else { return }
                            switch result {
                            case .success(let updated): self.selectionCheckFrame = (updated, completion); self.redraw()
                            case .failure(let error): completion(.failure(error))
                            }
                        }
                    }
                }
            } catch {
                let failure = BridgeState.error(error)
                DispatchQueue.main.async { completion(.failure(failure)) }
            }
        }
    }

    func endDevelopmentSelectionTapsCheck() {
        selectionCheckGeneration &+= 1
        selectionCheckFrame = nil
    }

    /// Opt-in device smoke: real engine, synthetic Pencil samples and native
    /// numeric APIs. It never claims physical touch, Pencil or UI tap coverage.
    func developmentSketchCheck(completion: @escaping (WorkspaceState) -> Void) {
        sketchCheckCompletion = completion
        developmentSketchStep(0)
    }

    private func developmentSketchStep(_ index: Int) {
        guard alive else { return }
        let advance: () -> Void = { [weak self] in
            guard let self else { return }
            NSLog("SolidCraft Step 6 synthetic sketch check %d", index)
            self.sketchCheckAfterFrame = { [weak self] in self?.developmentSketchStep(index + 1) }
            self.redraw()
        }
        let done: (Result<Void, BridgeError>) -> Void = { [weak self] result in
            switch result { case .success: advance(); case .failure(let error): self?.failure(error) }
        }
        switch index {
        case 0: command("sketch.create", params: ["plane": "XY", "name": "Step 6 rectangle"], completion: done)
        case 1: command("tool.set", params: ["tool": "sketch.rectangle.two_point"], completion: done)
        case 2: developmentSketchTap([0, 0, 0], completion: done)
        case 3: developmentSketchTap([40, 30, 0], completion: done)
        case 4: command("tool.set", params: ["tool": "select"], completion: done)
        case 5, 8: selectEntity(["type": "sketch_curve", "id": index == 5 ? "l1" : "l2"], additive: false, completion: done)
        case 6, 9, 15:
            requestNumeric(action: "begin", dimension: index == 15 ? nil : "length") { [weak self] result in
                guard let self else { return }
                do {
                    let numeric = try result.get()
                    guard let token = numeric.token else { throw BridgeError(code: "sketch_check", message: "Numeric editor returned no token") }
                    self.sketchCheckToken = token; advance()
                } catch { self.failure(error) }
            }
        case 7, 10, 16:
            let expression = index == 7 ? "40 mm" : (index == 10 ? "30 mm" : "60 mm")
            requestNumeric(action: "commit", token: sketchCheckToken, expression: expression) { [weak self] result in
                guard let self else { return }
                do {
                    let numeric = try result.get()
                    guard numeric.committed else { throw BridgeError(code: "sketch_check", message: "Numeric dimension did not commit") }
                    self.sketchCheckToken = nil
                    if index == 7 { self.sketchCheckDimension = numeric.workspace.sketch?.constraints.first(where: { $0.dimension && !$0.driven }) }
                    advance()
                } catch { self.failure(error) }
            }
        case 11: command("sketch.finish", completion: done)
        case 12: command("sketch.edit", params: ["sketch": "Step 6 rectangle"], completion: done)
        case 13: command("tool.cancel", completion: done)
        case 14:
            guard let dimension = sketchCheckDimension else { failure(BridgeError(code: "sketch_check", message: "Width dimension missing")); return }
            selectEntity(["type": "sketch_constraint", "id": dimension.id], additive: false, completion: done)
        case 17: command("edit.undo", completion: done)
        case 18: command("edit.redo", completion: done)
        case 19:
            guard let dimension = sketchCheckDimension else { failure(BridgeError(code: "sketch_check", message: "Width dimension missing")); return }
            selectEntity(["type": "sketch_constraint", "id": dimension.id], additive: false, completion: done)
        default:
            engine.async { [weak self, state] in
                do {
                    let data = try state.request(Data("{\"kind\":\"workspace\"}".utf8), lane: .engine)
                    let workspace = try JSONDecoder().decode(WorkspaceEnvelope.self, from: data).result
                    guard let sketch = workspace.sketch, sketch.status.lowercased() == "solved", sketch.profiles.count == 1,
                          let area = sketch.profiles.first?.area_mm2, abs(area - 1800) < 0.001,
                          sketch.constraints.contains(where: { $0.id == self?.sketchCheckDimension?.id && $0.expression == "60 mm" }) else {
                        throw BridgeError(code: "sketch_check", message: "Reopened 60 × 30 profile or retained width expression failed")
                    }
                    DispatchQueue.main.async { [weak self] in
                        guard let self, self.alive else { return }
                        self.onWorkspace?(workspace); self.sketchCheckCompletion?(workspace); self.sketchCheckCompletion = nil
                        NSLog("SolidCraft Step 6 sketch check PASS (real engine; synthetic Pencil input and numeric editor APIs; hardware unverified)")
                    }
                } catch { self?.failure(error) }
            }
        }
    }

    func developmentSketchTap(_ world: [Double], completion: @escaping (Result<Void, BridgeError>) -> Void) {
        display.async { [weak self, state] in
            do {
                let project = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": "view.project", "params": ["point": world]]), lane: .viewport)
                let p = try JSONDecoder().decode(DevelopmentProjection.self, from: project).result
                let report = try JSONDecoder().decode(DevelopmentViewport.self, from: state.request(Data("{\"kind\":\"snapshot\"}".utf8), lane: .viewport)).result
                guard p.points.count == 2 else { throw BridgeError(code: "sketch_check", message: "Missing viewport projection") }
                for phase in [InputSample.Phase.began, .ended] {
                    var sample = InputSample(source: .pencil, phase: phase, id: 6000,
                        x_points: p.points[0], y_points: p.points[1], x_pixels: p.points[0] * p.scale, y_pixels: p.points[1] * p.scale,
                        scale: p.scale, timestamp: ProcessInfo.processInfo.systemUptime, pressure: nil, altitude: nil,
                        geometry_revision: p.geometry_revision, consumed: false, shift: false, button: 0, scroll: 0)
                    sample.viewport_epoch = report.viewport_epoch; sample.camera_epoch = report.camera_epoch; sample.document_id = report.document_id
                    sample.display_preview = report.preview ?? false
                    let payload = Data("{\"kind\":\"input\",\"event\":".utf8) + (try JSONEncoder().encode(sample)) + Data("}".utf8)
                    _ = try state.request(payload, lane: .viewport)
                }
                self?.engine.async { [weak self, state] in
                    let result = Result { _ = try state.request(Data("{\"kind\":\"tool_work\"}".utf8), lane: .engine) }.mapError(BridgeState.error)
                    DispatchQueue.main.async { [weak self] in guard self?.alive == true else { return }; completion(result) }
                }
            } catch {
                let failure = BridgeState.error(error)
                DispatchQueue.main.async { completion(.failure(failure)) }
            }
        }
    }

    private var workspaceCheckToken: UInt64?
    private var workspaceCheckVolume: Double = 0
    private var workspaceCheckCommittedVolume: Double = 0

    /// Opt-in, software-injected bridge workflow. This exercises real commands,
    /// numeric preview and history; it does not represent UI taps or Pencil hardware.
    func developmentWorkspaceCheck() { developmentWorkspaceStep(0) }

    private func developmentWorkspaceStep(_ index: Int) {
        guard alive else { return }
        let advance: () -> Void = { [weak self] in
            NSLog("SolidCraft Step 5 synthetic workspace check %d", index)
            self?.developmentWorkspaceStep(index + 1)
        }
        let commandDone: (Result<Void, BridgeError>) -> Void = { [weak self] result in
            switch result { case .success: advance(); case .failure(let error): self?.failure(error) }
        }
        switch index {
        case 0, 8, 11:
            selectEntity(["type": "body", "name": "Post"], additive: false, completion: commandDone)
        case 1:
            command("tool.set", params: ["tool": "post_radius"], completion: commandDone)
        case 2, 9, 12:
            engine.async { [weak self, state] in
                let result = Result { () throws -> WorkspaceState in
                    let data = try state.request(Data("{\"kind\":\"workspace\"}".utf8), lane: .engine)
                    return try JSONDecoder().decode(WorkspaceEnvelope.self, from: data).result
                }.mapError(BridgeState.error)
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    do {
                        let workspace = try result.get()
                        let expected = index == 12 ? 12.0 : 7.0
                        guard let radius = workspace.radius, abs(radius.value - expected) < 0.001,
                              workspace.rows.contains(where: { $0.kind == "body" && $0.label == "Post" && $0.selected }) else {
                            throw BridgeError(code: "workspace_check", message: "Browser/inspector selection or history radius mismatch")
                        }
                        self.onWorkspace?(workspace)
                        advance()
                    } catch { self.failure(error) }
                }
            }
        case 3, 13:
            requestNumeric(action: "begin") { [weak self] result in
                guard let self, self.alive else { return }
                do {
                    let numeric = try result.get()
                    guard let token = numeric.token, let volume = numeric.viewport?.volume_mm3,
                          numeric.workspace.can_undo == false else {
                        throw BridgeError(code: "workspace_check", message: "Numeric begin or history isolation failed")
                    }
                    self.workspaceCheckToken = token
                    if index == 3 { self.workspaceCheckVolume = volume }
                    advance()
                } catch { self.failure(error) }
            }
        case 4:
            requestNumeric(action: "preview", token: workspaceCheckToken, expression: "12 +") { [weak self] result in
                switch result {
                case .failure(let error) where error.code == "numeric_validation": advance()
                case .failure(let error): self?.failure(error)
                case .success: self?.failure(BridgeError(code: "workspace_check", message: "Incomplete expression was accepted"))
                }
            }
        case 5, 6, 14, 15:
            let action = index == 6 ? "commit" : (index == 15 ? "cancel" : "preview")
            let expression: String? = index == 15 ? nil : (index == 14 ? "14 mm" : "12 mm")
            requestNumeric(action: action, token: workspaceCheckToken, expression: expression) { [weak self] result in
                guard let self, self.alive else { return }
                do {
                    let numeric = try result.get()
                    if index == 5 || index == 14 {
                        guard numeric.preview, let preview = numeric.viewport?.preview_volume_mm3,
                              let committed = numeric.viewport?.volume_mm3, preview > committed else {
                            throw BridgeError(code: "workspace_check", message: "Real numeric geometry preview did not grow")
                        }
                    }
                    if index == 6 {
                        guard numeric.committed, !numeric.preview, numeric.token == nil,
                              let volume = numeric.viewport?.volume_mm3, volume > self.workspaceCheckVolume,
                              let radius = numeric.viewport?.handle?.radius, abs(radius - 12) < 0.001 else {
                            throw BridgeError(code: "workspace_check", message: "Numeric confirmation did not commit geometry")
                        }
                        self.workspaceCheckCommittedVolume = volume
                    }
                    if index == 15 {
                        guard !numeric.preview, numeric.token == nil,
                              let volume = numeric.viewport?.volume_mm3,
                              abs(volume - self.workspaceCheckCommittedVolume) < 0.001,
                              let radius = numeric.viewport?.handle?.radius, abs(radius - 12) < 0.001 else {
                            throw BridgeError(code: "workspace_check", message: "Cancellation changed committed geometry")
                        }
                    }
                    advance()
                } catch { self.failure(error) }
            }
        case 7: command("edit.undo", completion: commandDone)
        case 10: command("edit.redo", completion: commandDone)
        default:
            workspaceCheckToken = nil
            NSLog("SolidCraft Step 5 workspace check PASS (synthetic bridge input; UI and hardware unverified)")
        }
    }

    /// Step 4 software-injected device workflow. Never describes physical Pencil
    /// verification. It exercises the same typed boundary, real preview and history.
    func developmentInputCheck() {
        let steps: [(String, [String: Any])] = [
            ("view.isometric", [:]), ("tool.set", ["tool": "post_radius"]),
            ("sample.pencil.began", ["id": 2000, "point": [17,4,35]]),
            ("sample.pencil.moved", ["id": 2000, "point": [22,4,35]]),
            ("work.preview", [:]),
            ("sample.finger.began", ["id": 2001, "point": [-10,0,10]]),
            ("sample.finger.began", ["id": 2002, "point": [30,0,10]]),
            ("sample.finger.moved", ["id": 2002, "point": [34,3,10]]),
            ("sample.pencil.moved", ["id": 2000, "point": [30,4,35]]),
            ("check.suspended", [:]),
            ("sample.finger.ended", ["id": 2001, "point": [-10,0,10]]),
            ("sample.finger.ended", ["id": 2002, "point": [34,3,10]]),
            ("sample.pencil.moved", ["id": 2000, "point": [22,4,35]]),
            ("sample.pencil.ended", ["id": 2000, "point": [22,4,35]]),
            ("work.commit", [:]), ("edit.undo", [:]), ("check.undo", [:]),
            ("edit.redo", [:]), ("check.redo", [:]),
            ("sample.pencil.began", ["id": 2000, "point": [22,4,35]]),
            ("sample.pencil.moved", ["id": 2000, "point": [26,4,35]]),
            ("work.preview", [:]), ("input.cancel", [:]), ("check.cancelled", [:])
        ]
        developmentInputStep(steps, index: 0)
    }
    private func developmentInputStep(_ steps: [(String, [String: Any])], index: Int) {
        guard alive, steps.indices.contains(index) else {
            NSLog("SolidCraft Step 4 input check PASS (synthetic input; hardware unverified)")
            return
        }
        let (command, params) = steps[index]
        engine.async { [weak self, state] in
            do {
                var payload: Data
                if command.hasPrefix("sample.") {
                    let parts = command.split(separator: ".")
                    let project = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": "view.project", "params": ["point": params["point"] ?? []]]), lane: .engine)
                    let p = try JSONDecoder().decode(DevelopmentProjection.self, from: project).result
                    guard p.points.count == 2 else { throw BridgeError(code: "input_check", message: "missing projection") }
                    let sample = InputSample(source: parts[1] == "pencil" ? .pencil : .finger,
                        phase: parts[2] == "began" ? .began : (parts[2] == "ended" ? .ended : .moved),
                        id: (params["id"] as? UInt64) ?? UInt64(params["id"] as? Int ?? 0),
                        x_points: p.points[0], y_points: p.points[1], x_pixels: p.points[0] * p.scale, y_pixels: p.points[1] * p.scale,
                        scale: p.scale, timestamp: ProcessInfo.processInfo.systemUptime, pressure: nil, altitude: nil,
                        geometry_revision: p.geometry_revision, consumed: false, shift: false, button: 0, scroll: 0)
                    payload = Data("{\"kind\":\"input\",\"event\":".utf8) + (try JSONEncoder().encode(sample)) + Data("}".utf8)
                } else if command.hasPrefix("work.") {
                    payload = Data("{\"kind\":\"tool_work\"}".utf8)
                } else if command.hasPrefix("check.") {
                    // Snapshot on the viewport lane is read-only, including during edits.
                    payload = Data("{\"kind\":\"snapshot\"}".utf8)
                } else {
                    payload = try JSONSerialization.data(withJSONObject: ["kind": "command", "command": command, "params": params])
                }
                let lane: BridgeState.Lane = command.hasPrefix("check.") ? .viewport : .engine
                // Checks use the display queue so the viewport response lane remains serial.
                let reply: Data
                if lane == .viewport { reply = try self?.display.sync { try state.request(payload, lane: .viewport) } ?? Data() }
                else { reply = try state.request(payload, lane: .engine) }
                if command.hasPrefix("check.") || command.hasPrefix("work.") {
                    let envelope = try JSONSerialization.jsonObject(with: reply) as? [String: Any]
                    let result = envelope?["result"] as? [String: Any] ?? [:]
                    let report = (result["viewport"] as? [String: Any]) ?? result
                    let input = report["input"] as? [String: Any] ?? [:]
                    let handle = report["handle"] as? [String: Any] ?? [:]
                    let radius = handle["radius"] as? Double ?? 0
                    let volume = report["volume_mm3"] as? Double ?? 0
                    let preview = report["preview_volume_mm3"] as? Double
                    var valid = true
                    if command == "work.preview" { valid = preview.map { $0 > volume } ?? false }
                    if command == "work.commit" { valid = result["committed"] as? Bool == true && abs(radius - 12) < 0.001 }
                    if command == "check.suspended" { valid = input["edit"] as? String == "suspended" && abs((input["radius"] as? Double ?? 0) - 12) < 0.001 }
                    if command == "check.undo" { valid = abs(radius - 7) < 0.001 }
                    if command == "check.redo" || command == "check.cancelled" { valid = abs(radius - 12) < 0.001 && preview == nil }
                    guard valid else { throw BridgeError(code: "input_check", message: command + " assertion failed") }
                    NSLog("SolidCraft Step 4 %@: radius=%.6f committed=%.6f mm3 preview=%@", command, radius, volume, String(describing: preview))
                }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    self.redraw()
                    NSLog("SolidCraft Step 4 check %d: %@", index, command)
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.7) { [weak self] in self?.developmentInputStep(steps, index: index + 1) }
                }
            } catch { self?.failure(error) }
        }
    }

    /// Opt-in repeatable device smoke workflow. Samples are synthetic: this verifies the
    /// native boundary and renderer, never actual finger/Pencil hardware or hover support.
    func developmentCheck() {
        let steps: [(String, [String: Any])] = [
            ("view.navigate", ["motion": "orbit", "delta": [45, 20]]),
            ("view.navigate", ["motion": "pan", "delta": [24, 12], "viewport": [800, 600]]),
            ("view.navigate", ["motion": "zoom", "factor": 0.8]),
            ("view.fit", [:]),
            ("view.front", [:]), ("view.back", [:]), ("view.left", [:]), ("view.right", [:]),
            ("view.bottom", [:]), ("view.isometric", [:]), ("view.top", [:]),
            ("tap.body", ["point": [-8, 0, 10]]), ("tap.face", ["point": [-8, 0, 10]]),
            ("tap.edge", ["point": [0, -15, 10]]), ("tap.vertex", ["point": [-20, -15, 10]]),
            ("select.multiple", ["enabled": true]), ("tap.face", ["point": [-8, 0, 10]]),
            ("select.clear", [:]), ("view.perspective", ["enabled": true]),
            ("view.isometric", [:]),
            ("timeline.edit", ["feature": "Viewport post", "set": ["height": "40"]]),
            ("edit.undo", [:]), ("edit.redo", [:])
        ]
        developmentStep(steps, index: 0)
    }

    private func developmentStep(_ steps: [(String, [String: Any])], index: Int) {
        guard alive, steps.indices.contains(index) else {
            NSLog("SolidCraft Step 3 development check complete (synthetic input)")
            return
        }
        let (command, params) = steps[index]
        let isTap = command.hasPrefix("tap.")
        let isDisplay = isTap || command.hasPrefix("view.") || command.hasPrefix("select.") || command.hasPrefix("tool.") || command == "input.cancel"
        let queue = isDisplay ? display : engine
        queue.async { [weak self, state] in
            do {
                if isTap {
                    let filter = String(command.dropFirst(4))
                    if ProcessInfo.processInfo.arguments.contains("--auto-face-extrude-check") {
                        _ = try state.request(Data("{\"kind\":\"command\",\"command\":\"select.clear\",\"params\":{}}".utf8), lane: .viewport)
                    }
                    _ = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": "select.filter", "params": ["filter": filter]]), lane: .viewport)
                    let project = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": "view.project", "params": params]), lane: .viewport)
                    let projected = try JSONDecoder().decode(DevelopmentProjection.self, from: project).result
                    let p = projected.points
                    guard p.count == 2 else { throw BridgeError(code: "projection", message: "expected two coordinates") }
                    for phase in [InputSample.Phase.began, .ended] {
                        let sample = InputSample(source: .pencil, phase: phase, id: 1000,
                            x_points: p[0], y_points: p[1], x_pixels: p[0] * projected.scale, y_pixels: p[1] * projected.scale,
                            scale: projected.scale, timestamp: ProcessInfo.processInfo.systemUptime, pressure: nil, altitude: nil,
                            geometry_revision: projected.geometry_revision, consumed: false, shift: false, button: 0, scroll: 0)
                        let event = try JSONEncoder().encode(sample)
                        let reply = try state.request(Data("{\"kind\":\"input\",\"event\":".utf8) + event + Data("}".utf8), lane: .viewport)
                        if phase == .ended {
                            let report = try JSONDecoder().decode(DevelopmentPick.self, from: reply).result
                            guard report.selection.last?.type == filter else {
                                throw BridgeError(code: "development_pick", message: "expected " + filter)
                            }
                            NSLog("SolidCraft Step 3 synthetic pick: %@", report.readout)
                        }
                    }
                } else {
                    _ = try state.request(JSONSerialization.data(withJSONObject: ["kind": "command", "command": command, "params": params]), lane: isDisplay ? .viewport : .engine)
                }
                DispatchQueue.main.async { [weak self] in
                    guard let self, self.alive else { return }
                    NSLog("SolidCraft Step 3 check %d: %@", index, command)
                    self.redraw()
                    if index == 12, ProcessInfo.processInfo.arguments.contains("--viewport-check-hold-face") { return }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in self?.developmentStep(steps, index: index + 1) }
                }
            } catch { self?.failure(error) }
        }
    }
    private struct DevelopmentPick: Decodable { let result: BridgeState.ViewportReport }
    private struct DevelopmentProjection: Decodable {
        struct Point: Decodable { let points: [Double]; let scale: Double; let geometry_revision: UInt64 }
        let result: Point
    }
    private struct DevelopmentViewport: Decodable { let result: BridgeState.ViewportReport }
    #endif

    private func failure(_ error: Error) {
        let failure = BridgeState.error(error)
        DispatchQueue.main.async { [weak self] in
            guard let self, self.alive else { return }
            self.onFailure?(failure)
        }
    }

    deinit {
        do { try lifecycle.suspend { try state.suspend() } }
        catch { NSLog("%@", BridgeState.error(error).description) }
        engine.async { [state, lifecycle, display] in display.async { lifecycle.exclusive { state.close() } } }
    }
}

private final class LifecycleGate {
    private let lock = NSLock()
    private var active = false
    private var generation: UInt64 = 0
    private var hasRenderer = false
    var ticket: UInt64? { lock.lock(); defer { lock.unlock() }; return active ? generation : nil }

    func activate() { lock.lock(); active = true; generation &+= 1; lock.unlock() }
    func suspend(_ operation: () throws -> Void) throws {
        lock.lock(); defer { lock.unlock() }
        active = false; generation &+= 1
        try operation()
    }
    func rendering<T>(ticket: UInt64, state: BridgeState, _ operation: () throws -> T) rethrows -> T? {
        lock.lock(); defer { hasRenderer = state.hasSurface; lock.unlock() }
        guard active, ticket == generation else { return nil }
        return try operation()
    }
    func exclusive(_ operation: () -> Void) { lock.lock(); defer { lock.unlock() }; operation(); hasRenderer = false }
    func accepts(_ ticket: UInt64) -> Bool { lock.lock(); defer { lock.unlock() }; return active && ticket == generation }
}

private final class BridgeState {
    enum Lane { case engine, viewport, surface }
    private let handle: UInt64 = sc_session_create()
    private var layer: CAMetalLayer? // display queue, or lifecycle lock during suspension
    private var width: UInt32 = 0
    private var height: UInt32 = 0
    private var closed = false
    private var attached = false
    var hasSurface: Bool { layer != nil }

    func present(layer: CAMetalLayer, width: UInt32, height: UInt32) throws {
        if !attached || self.layer !== layer {
            try check(sc_surface_attach(handle, Unmanaged.passUnretained(layer).toOpaque(), width, height), lane: .surface)
            self.layer = layer; attached = true
        } else if self.width != width || self.height != height {
            try check(sc_surface_resize(handle, width, height), lane: .surface)
        }
        self.width = width; self.height = height
    }

    func redraw() throws -> FrameReport? {
        guard handle != 0, let layer else { return nil }
        if !attached { try present(layer: layer, width: width, height: height) }
        do {
            try check(sc_surface_draw(handle), lane: .surface)
            return try JSONDecoder().decode(Envelope<FrameReport>.self, from: response(.surface)).result
        } catch { attached = false; throw error }
    }
    func suspend() throws {
        guard handle != 0 else { return }
        try check(sc_surface_suspend(handle), lane: .surface)
        attached = false
    }
    func close() {
        guard !closed else { return }
        closed = true
        _ = sc_surface_suspend(handle)
        _ = sc_session_destroy(handle)
        layer = nil; attached = false
    }
    func request(_ data: Data, lane: Lane) throws -> Data {
        guard handle != 0 else { throw BridgeError(code: "session_create", message: "handle=0") }
        let status = data.withUnsafeBytes { buffer in
            let bytes = buffer.bindMemory(to: UInt8.self)
            return lane == .engine ? sc_request(handle, bytes.baseAddress, bytes.count) : sc_view_request(handle, bytes.baseAddress, bytes.count)
        }
        try check(status, lane: lane)
        return try response(lane)
    }
    private func check(_ status: Int32, lane: Lane) throws {
        guard status != 0 else { return }
        if let data = try? response(lane), let envelope = try? JSONDecoder().decode(Envelope<Empty>.self, from: data), let error = envelope.error {
            throw BridgeError(code: error.code, message: error.message)
        }
        throw BridgeError(code: "bridge", message: "status=\(status)")
    }
    private func response(_ lane: Lane) throws -> Data {
        let count: Int
        switch lane {
        case .engine: count = sc_response_len(handle)
        case .viewport: count = sc_view_response_len(handle)
        case .surface: count = sc_surface_response_len(handle)
        }
        guard count > 0, count <= 4_194_304 else { throw BridgeError(code: "response_size", message: "bytes=\(count)") }
        var data = Data(count: count)
        let status = data.withUnsafeMutableBytes { buffer -> Int32 in
            let bytes = buffer.bindMemory(to: UInt8.self)
            switch lane {
            case .engine: return sc_response_copy(handle, bytes.baseAddress, bytes.count)
            case .viewport: return sc_view_response_copy(handle, bytes.baseAddress, bytes.count)
            case .surface: return sc_surface_response_copy(handle, bytes.baseAddress, bytes.count)
            }
        }
        guard status == 0 else { throw BridgeError(code: "response_copy", message: "status=\(status)") }
        return data
    }
    static func error(_ error: Error) -> BridgeError { (error as? BridgeError) ?? BridgeError(code: "native", message: error.localizedDescription) }
    private struct Empty: Decodable {}
    private struct Envelope<T: Decodable>: Decodable {
        struct Failure: Decodable { let code: String; let message: String }
        let ok: Bool
        let error: Failure?
        let result: T?
    }
    struct FrameReport: Decodable {
        let presented: Bool?
        let geometry_revision: UInt64?
        let viewport: ViewportReport?
    }
    struct ViewportReport: Decodable {
        typealias Entity = WorkspaceEntity
        struct Input: Decodable { let tool: String; let edit: String?; let sketch: SketchInputPresentation?; let dismissed_model_token: UInt64? }
        let input: Input?
        let annotations: [SketchAnnotationPresentation]?
        let constraint_indicators: [SketchConstraintIndicator]?
        let reference_labels: [ReferenceLabelPresentation]?
        let origin: ViewControlsPresentation.Origin?
        let grid: ViewControlsPresentation.Grid?
        let planar: Bool?
        let camera: ViewControlsPresentation.Camera?
        var viewPresentation: ViewControlsPresentation? {
            guard let origin, let grid, let planar, let camera else { return nil }
            return ViewControlsPresentation(origin: origin, grid: grid, planar: planar, camera: camera)
        }
        let model_value: ModelValuePresentation?
        let selection: [Entity]
        let hover: Entity?
        let geometry_revision: UInt64?
        let updating: Bool
        struct Metrics: Decodable { let width_points: Double; let height_points: Double; let scale: Double }
        let metrics: Metrics?
        let preview: Bool?
        let preview_stale: Bool?
        let camera_epoch: UInt64?
        let viewport_epoch: UInt64?
        let document_id: String?
        var readout: String {
            let picks = selection.isEmpty ? NativeText.value("ui.ipados.no_selection") : selection.prefix(4).map(\.readout).joined(separator: " | ")
            let preselect = hover.map { " · \(NativeText.value("ui.ipados.hover")): \($0.readout)" } ?? ""
            return "r\(geometry_revision ?? 0) · \(picks)\(preselect)" + (updating ? " · \(NativeText.value("ui.ipados.updating"))" : "")
        }
    }
}
