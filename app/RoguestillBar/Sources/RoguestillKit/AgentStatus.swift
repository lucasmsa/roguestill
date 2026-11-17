public enum AgentStatus: Equatable, Sendable {
    case enabled, notRegistered, requiresApproval, unavailable
}

public func agentMessage(for status: AgentStatus) -> String? {
    switch status {
    case .enabled: nil
    case .notRegistered: "Daemon not installed"
    case .requiresApproval: "Allow Roguestill in Login Items"
    case .unavailable: "Daemon missing from the app"
    }
}

public func agentNeedsSettings(_ status: AgentStatus) -> Bool { status == .requiresApproval }
