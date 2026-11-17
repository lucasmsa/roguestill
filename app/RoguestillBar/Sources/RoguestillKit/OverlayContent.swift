import Foundation

public struct OverlayContent: Equatable, Sendable {
    public let gameKey: String
    public let title: String
    public let detail: String?
    public let progress: Double?
}

private let warningSeconds = 10.0

private func gameKey(_ target: String) -> String {
    let lower = target.lowercased()
    return lower.hasSuffix(".exe") ? String(lower.dropLast(4)) : lower
}

public func overlayContent(for state: OverlayState) -> OverlayContent? {
    switch state {
    case .warning(let target, let seconds):
        let key = gameKey(target)
        return OverlayContent(gameKey: key, title: "\(key.uppercased()) CLOSES IN \(seconds)", detail: nil,
                              progress: min(1, Double(seconds) / warningSeconds))
    case .blockCard(let target, let reason):
        let key = gameKey(target)
        return OverlayContent(gameKey: key, title: "\(key.uppercased()) BLOCKED", detail: reason.uppercased(), progress: nil)
    case .hidden:
        return nil
    }
}
