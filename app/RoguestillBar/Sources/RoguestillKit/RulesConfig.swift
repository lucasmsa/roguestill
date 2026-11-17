import Foundation

public struct RulesConfig: Codable, Equatable, Sendable {
    public var noitaSavePath: String?
    public var rule: [GameRule]

    enum CodingKeys: String, CodingKey {
        case noitaSavePath = "noita_save_path"
        case rule
    }
}

public struct GameRule: Codable, Equatable, Sendable, Identifiable {
    public struct Target: Codable, Equatable, Sendable {
        public var process: String
    }

    public struct Budget: Codable, Equatable, Sendable {
        public var kind: String
        public var limitMinutes: Int
        public var resetHour: Int
        public var idlePauseMinutes: Int
        public var limitRuns: Int?

        enum CodingKeys: String, CodingKey {
            case kind
            case limitMinutes = "limit_minutes"
            case resetHour = "reset_hour"
            case idlePauseMinutes = "idle_pause_minutes"
            case limitRuns = "limit_runs"
        }
    }

    public struct Action: Codable, Equatable, Sendable {
        public var warnSeconds: Int
        public var kill: Bool
        public var relaunch: String

        enum CodingKeys: String, CodingKey {
            case warnSeconds = "warn_seconds"
            case kill, relaunch
        }
    }

    public var name: String
    public var target: Target
    public var adapter: String?
    public var budget: Budget
    public var action: Action
    public var lockChanges: Bool
    public var id: String { name }

    enum CodingKeys: String, CodingKey {
        case name, target, adapter, budget, action
        case lockChanges = "lock_changes"
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        target = try c.decode(Target.self, forKey: .target)
        adapter = try c.decodeIfPresent(String.self, forKey: .adapter)
        budget = try c.decode(Budget.self, forKey: .budget)
        action = try c.decode(Action.self, forKey: .action)
        lockChanges = try c.decodeIfPresent(Bool.self, forKey: .lockChanges) ?? false
    }

    public init(name: String, target: Target, adapter: String?, budget: Budget, action: Action, lockChanges: Bool = false) {
        self.name = name
        self.target = target
        self.adapter = adapter
        self.budget = budget
        self.action = action
        self.lockChanges = lockChanges
    }

    public static func new(name: String, process: String) -> GameRule {
        GameRule(name: name, target: Target(process: process), adapter: nil,
                 budget: Budget(kind: "daily_time", limitMinutes: 60, resetHour: 0, idlePauseMinutes: 5, limitRuns: nil),
                 action: Action(warnSeconds: 10, kill: true, relaunch: "kill_again"))
    }
}

public protocol ConfigClient: Sendable {
    func load() async throws -> RulesConfig
    func save(_ config: RulesConfig) async throws
}

public struct ConfigError: Error, CustomStringConvertible, Sendable {
    public let description: String
}

public struct ProcessConfigClient: ConfigClient {
    private let executable: URL

    public init(executable: URL) { self.executable = executable }

    public func load() async throws -> RulesConfig {
        let output = try run(["config"], input: nil)
        return try JSONDecoder().decode(RulesConfig.self, from: output)
    }

    public func save(_ config: RulesConfig) async throws {
        _ = try run(["config-set"], input: try JSONEncoder().encode(config))
    }

    private func run(_ arguments: [String], input: Data?) throws -> Data {
        let process = Process()
        process.executableURL = executable
        process.arguments = arguments
        let stdout = Pipe(), stderr = Pipe(), stdin = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        process.standardInput = stdin
        try process.run()
        if let input { stdin.fileHandleForWriting.write(input) }
        try stdin.fileHandleForWriting.close()
        let data = stdout.fileHandleForReading.readDataToEndOfFile()
        let errors = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw ConfigError(description: String(decoding: errors, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines))
        }
        return data
    }
}
