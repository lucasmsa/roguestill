import Foundation

public enum ConnectionState: Equatable, Sendable {
    case connected(Status, at: Date)
    case daemonDown(lastKnown: Status?, since: Date)
    case unsupportedVersion(Int)

    public var status: Status? {
        switch self {
        case .connected(let status, _): status
        case .daemonDown(let status, _): status
        case .unsupportedVersion: nil
        }
    }
}

public enum Event: Equatable, Sendable {
    case blocked(rule: String, target: String)
}

public func reduce(previous: ConnectionState, result: Result<Data, any Error>, at now: Date)
    -> (state: ConnectionState, events: [Event]) {
    struct Version: Decodable { let v: Int }
    do {
        let data = try result.get()
        let version = try JSONDecoder().decode(Version.self, from: data).v
        guard version == 1 else { return (.unsupportedVersion(version), []) }
        let status = try JSONDecoder().decode(Status.self, from: data)
        let events: [Event] = status.rules.compactMap { rule in
            guard let old = previous.status?.rules.first(where: { $0.name == rule.name }),
                  rule.blockedSeq > old.blockedSeq else { return nil }
            return .blocked(rule: rule.name, target: rule.target)
        }
        return (.connected(status, at: now), events)
    } catch {
        let since: Date
        if case .daemonDown(_, let date) = previous { since = date } else { since = now }
        return (.daemonDown(lastKnown: previous.status, since: since), [])
    }
}

public struct BlockNotice: Equatable, Sendable {
    public let target: String
    public let at: Date

    public init(target: String, at: Date) {
        self.target = target
        self.at = at
    }
}

public enum OverlayState: Equatable, Sendable {
    case warning(target: String, secondsLeft: UInt64)
    case blockCard(target: String, reason: String)
    case hidden
}

public func deriveOverlay(status: Status?, block: BlockNotice?, at now: Date) -> OverlayState {
    if let block, now >= block.at, now.timeIntervalSince(block.at) < 3 {
        return .blockCard(target: block.target, reason: "daily budget exhausted")
    }
    if let rule = status?.rules.first(where: { $0.state == "warning" }) {
        return .warning(target: rule.target, secondsLeft: rule.remainingSecs)
    }
    return .hidden
}

public func formatDuration(_ seconds: UInt64) -> String {
    if seconds >= 3600 { return String(format: "%llu:%02llu", seconds / 3600, seconds % 3600 / 60) }
    return String(format: "%llu:%02llu", seconds / 60, seconds % 60)
}
