import Foundation

public struct Snapshot: Codable, Equatable, Sendable {
    public enum Kind: String, Codable, Sendable {
        case connected, daemonDown, unsupportedVersion
    }

    public let kind: Kind
    public let status: Status?
    public let updatedAt: Date

    public init(kind: Kind, status: Status?, updatedAt: Date) {
        self.kind = kind
        self.status = status
        self.updatedAt = updatedAt
    }

    public init(connection: ConnectionState, now: Date = Date()) {
        switch connection {
        case .connected(let status, let at): self.init(kind: .connected, status: status, updatedAt: at)
        case .daemonDown(let lastKnown, let since): self.init(kind: .daemonDown, status: lastKnown, updatedAt: since)
        case .unsupportedVersion: self.init(kind: .unsupportedVersion, status: nil, updatedAt: now)
        }
    }

    public static func read(from url: URL) -> Snapshot? {
        guard let data = try? Data(contentsOf: url) else { return nil }
        let decoder = JSONDecoder()
        decoder.dateDecodingStrategy = .secondsSince1970
        return try? decoder.decode(Snapshot.self, from: data)
    }

    public func write(to url: URL) throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .secondsSince1970
        try encoder.encode(self).write(to: url, options: .atomic)
    }
}

public let appGroupID = "group.com.lucasmsa.run-limiter"

public func snapshotURL() -> URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupID)?
        .appendingPathComponent("snapshot.json")
}

public func shouldReloadWidget(previous: Snapshot?, next: Snapshot, lastReload: Date?, now: Date) -> Bool {
    guard let previous, let lastReload else { return true }
    if previous.kind != next.kind { return true }
    let shape = { (s: Snapshot) in s.status?.rules.map { [$0.state, String($0.runsToday), String($0.blockedSeq)] } }
    if shape(previous) != shape(next) { return true }
    return now.timeIntervalSince(lastReload) >= 60
}
