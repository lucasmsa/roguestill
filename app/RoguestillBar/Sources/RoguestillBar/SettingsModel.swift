import Foundation
import RoguestillKit
import ServiceManagement

@MainActor
final class SettingsModel: ObservableObject {
    @Published var config: RulesConfig?
    @Published var error: String?
    @Published var launchAtLogin = SMAppService.mainApp.status == .enabled
    private let client: any ConfigClient
    private var pending: Task<Void, Never>?
    private var lastSaved: RulesConfig?

    init(client: any ConfigClient = ProcessConfigClient(executable: cliURL())) {
        self.client = client
    }

    func load() async {
        do {
            let loaded = try await client.load()
            config = loaded
            lastSaved = loaded
            error = nil
        } catch {
            self.error = "Could not read rules: \(error)"
        }
    }

    func changed() {
        guard let current = config, current != lastSaved else { return }
        pending?.cancel()
        pending = Task { [weak self] in
            do { try await Task.sleep(for: .milliseconds(600)) } catch { return }
            await self?.save(current)
        }
    }

    func addGame(name: String, process: String) {
        let trimmed = (name.trimmingCharacters(in: .whitespaces), process.trimmingCharacters(in: .whitespaces))
        guard !trimmed.0.isEmpty, !trimmed.1.isEmpty else { return }
        config?.rule.append(.new(name: trimmed.0, process: trimmed.1))
    }

    func remove(_ index: Int) {
        guard let rules = config?.rule, rules.indices.contains(index), rules.count > 1 else { return }
        config?.rule.remove(at: index)
    }

    func setLaunchAtLogin(_ on: Bool) {
        do {
            if on { try SMAppService.mainApp.register() } else { try SMAppService.mainApp.unregister() }
        } catch {
            self.error = "Open at login failed: \(error)"
        }
        launchAtLogin = SMAppService.mainApp.status == .enabled
    }

    private func save(_ value: RulesConfig) async {
        do {
            try await client.save(value)
            lastSaved = value
            error = nil
        } catch {
            self.error = "Not saved: \(error)"
        }
    }
}

func cliURL() -> URL {
    if let override = ProcessInfo.processInfo.environment["ROGUESTILL_CLI"] { return URL(fileURLWithPath: override) }
    return Bundle.main.url(forAuxiliaryExecutable: "roguestill") ?? URL(fileURLWithPath: "/usr/local/bin/roguestill")
}
