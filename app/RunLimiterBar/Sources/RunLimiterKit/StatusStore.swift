import Combine
import Foundation

@MainActor
public final class StatusStore: ObservableObject {
    @Published public private(set) var connection: ConnectionState
    @Published public private(set) var overlay: OverlayState = .hidden
    @Published public private(set) var lastSeen: Date?
    private let client: any StatusClient
    private let pollInterval: Duration
    private let now: @Sendable () -> Date
    private var polling: Task<Void, Never>?
    private var expiration: Task<Void, Never>?
    private var block: BlockNotice?
    private var fetching = false
    private var generation = 0

    public init(client: any StatusClient = SocketClient(), pollInterval: Duration = .seconds(1),
                now: @escaping @Sendable () -> Date = { Date() }) {
        self.client = client
        self.pollInterval = max(pollInterval, .milliseconds(1))
        self.now = now
        connection = .daemonDown(lastKnown: nil, since: now())
    }

    deinit {
        polling?.cancel()
        expiration?.cancel()
    }

    public func start() {
        guard polling == nil else { return }
        let interval = pollInterval
        polling = Task { [weak self] in
            while !Task.isCancelled {
                await self?.poll()
                do { try await Task.sleep(for: interval) } catch { return }
            }
        }
    }

    public func stop() {
        generation += 1
        polling?.cancel()
        polling = nil
        expiration?.cancel()
        expiration = nil
        block = nil
        overlay = .hidden
    }

    public func poll() async {
        guard !fetching else { return }
        fetching = true
        defer { fetching = false }
        let currentGeneration = generation
        let result: Result<Data, any Error>
        do { result = .success(try await client.fetch()) } catch { result = .failure(error) }
        guard !Task.isCancelled, generation == currentGeneration else { return }
        let date = now()
        let reduction = reduce(previous: connection, result: result, at: date)
        connection = reduction.state
        if case .connected = connection { lastSeen = date }
        if case .unsupportedVersion = connection {
            block = nil
            expiration?.cancel()
        }
        if let event = reduction.events.last, case .blocked(_, let target) = event {
            block = BlockNotice(target: target, at: date)
            expiration?.cancel()
            expiration = Task { [weak self] in
                do { try await Task.sleep(for: .seconds(3)) } catch { return }
                self?.block = nil
                self?.refreshOverlay()
            }
        }
        refreshOverlay()
    }

    private func refreshOverlay() {
        let status: Status?
        if case .connected(let value, _) = connection { status = value } else { status = nil }
        overlay = deriveOverlay(status: status, block: block, at: now())
    }
}
