import Foundation

public struct Status: Codable, Equatable, Sendable {
    public let v: Int
    public let rules: [RuleStatus]

    public init(v: Int, rules: [RuleStatus]) {
        self.v = v
        self.rules = rules
    }
}

public struct RuleStatus: Codable, Equatable, Sendable {
    public let name: String
    public let target: String
    public let usedSecs: UInt64
    public let remainingSecs: UInt64
    public let runsToday: UInt64
    public let degraded: Bool
    public let state: String
    public let blockedSeq: UInt64
    public let limitRuns: UInt64?

    enum CodingKeys: String, CodingKey {
        case name, target, degraded, state
        case usedSecs = "used_secs"
        case remainingSecs = "remaining_secs"
        case runsToday = "runs_today"
        case blockedSeq = "blocked_seq"
        case limitRuns = "limit_runs"
    }

    public init(name: String, target: String, usedSecs: UInt64, remainingSecs: UInt64,
                runsToday: UInt64, degraded: Bool, state: String, blockedSeq: UInt64 = 0, limitRuns: UInt64? = nil) {
        self.name = name
        self.target = target
        self.usedSecs = usedSecs
        self.remainingSecs = remainingSecs
        self.runsToday = runsToday
        self.degraded = degraded
        self.state = state
        self.blockedSeq = blockedSeq
        self.limitRuns = limitRuns
    }

    public init(from decoder: any Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        name = try c.decode(String.self, forKey: .name)
        target = try c.decode(String.self, forKey: .target)
        usedSecs = try c.decode(UInt64.self, forKey: .usedSecs)
        remainingSecs = try c.decode(UInt64.self, forKey: .remainingSecs)
        runsToday = try c.decode(UInt64.self, forKey: .runsToday)
        degraded = try c.decode(Bool.self, forKey: .degraded)
        state = try c.decode(String.self, forKey: .state)
        blockedSeq = try c.decodeIfPresent(UInt64.self, forKey: .blockedSeq) ?? 0
        limitRuns = try c.decodeIfPresent(UInt64.self, forKey: .limitRuns)
    }
}

public func runsText(_ rule: RuleStatus) -> String {
    guard let limit = rule.limitRuns else { return "Runs today: \(rule.runsToday)" }
    return "Runs today: \(rule.runsToday) of \(limit)"
}
