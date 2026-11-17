import Darwin
import Foundation
import Testing
@testable import RoguestillKit

private let date = Date(timeIntervalSince1970: 1_000)

private func rule(_ sequence: UInt64 = 0, name: String = "game", state: String = "ok") -> RuleStatus {
    RuleStatus(name: name, target: "game.exe", usedSecs: 123, remainingSecs: 3250,
               runsToday: 2, degraded: true, state: state, blockedSeq: sequence,
               killInSecs: state == "warning" ? 9 : nil)
}

private func status(_ sequence: UInt64 = 0, state: String = "ok") -> Status {
    Status(v: 1, rules: [rule(sequence, state: state)])
}

private func success(_ value: Status) throws -> Result<Data, any Error> {
    .success(try JSONEncoder().encode(value))
}

@Test func decodingAndEncoding() throws {
    let json = #"{"v":1,"rules":[{"name":"game","target":"game.exe","used_secs":123,"remaining_secs":3250,"runs_today":2,"degraded":true,"state":"ok"}]}"#
    let decoded = try JSONDecoder().decode(Status.self, from: Data(json.utf8))
    #expect(decoded == status())
    let encoded = try JSONEncoder().encode(status(42))
    #expect(try JSONDecoder().decode(Status.self, from: encoded) == status(42))
    let object = try #require(JSONSerialization.jsonObject(with: encoded) as? [String: Any])
    let rules = try #require(object["rules"] as? [[String: Any]])
    #expect(rules[0]["blocked_seq"] as? Int == 42)
    #expect(rules[0]["used_secs"] as? Int == 123)
    #expect(rules[0]["remaining_secs"] as? Int == 3250)
    #expect(rules[0]["runs_today"] as? Int == 2)
    let wide = RuleStatus(name: "wide", target: "wide", usedSecs: .max, remainingSecs: .max,
                          runsToday: .max, degraded: false, state: "ok", blockedSeq: .max)
    #expect(try JSONDecoder().decode(RuleStatus.self, from: JSONEncoder().encode(wide)) == wide)
}

@Test func firstPollAndConnected() throws {
    let result = reduce(previous: .daemonDown(lastKnown: nil, since: date),
                        result: try success(status(9)), at: date)
    #expect(result.state == .connected(status(9), at: date))
    #expect(result.events.isEmpty)
}

@Test func failuresPreserveStatusAndOutageStart() throws {
    let later = date.addingTimeInterval(1)
    let first = reduce(previous: .connected(status(), at: date),
                       result: .failure(SocketError.timedOut), at: later)
    #expect(first.state == .daemonDown(lastKnown: status(), since: later))
    #expect(first.events.isEmpty)
    let second = reduce(previous: first.state, result: .success(Data("bad json".utf8)),
                        at: later.addingTimeInterval(1))
    #expect(second.state == first.state)
    #expect(second.events.isEmpty)
    let recovery = reduce(previous: second.state, result: try success(status(1)), at: later)
    #expect(recovery.events == [.blocked(rule: "game", target: "game.exe")])
}

@Test func unsupportedHasNoNumbers() {
    for version in [0, 2, 300] {
        let result = reduce(previous: .connected(status(), at: date),
                            result: .success(Data("{\"v\":\(version),\"rules\":\"future schema\"}".utf8)), at: date)
        #expect(result.state == .unsupportedVersion(version))
        #expect(result.state.status == nil)
        #expect(result.events.isEmpty)
    }
    let result = reduce(previous: .unsupportedVersion(2), result: .failure(SocketError.timedOut), at: date)
    #expect(result.state == .daemonDown(lastKnown: nil, since: date))
}

@Test func sequenceChanges() throws {
    let previous = ConnectionState.connected(status(4), at: date)
    for sequence: UInt64 in [0, 3, 4] {
        #expect(reduce(previous: previous, result: try success(status(sequence)), at: date).events.isEmpty)
    }
    #expect(reduce(previous: previous, result: try success(status(10)), at: date).events == [
        .blocked(rule: "game", target: "game.exe")
    ])
    let changedRules = Status(v: 1, rules: [rule(100, name: "new"), rule(5)])
    #expect(reduce(previous: previous, result: try success(changedRules), at: date).events == [
        .blocked(rule: "game", target: "game.exe")
    ])
    #expect(reduce(previous: .connected(Status(v: 1, rules: []), at: date),
                   result: try success(status(100)), at: date).events.isEmpty)
}

@Test func overlays() {
    let warning = status(state: "warning")
    let notice = BlockNotice(target: "blocked.exe", at: date)
    #expect(deriveOverlay(status: warning, block: nil, at: date) == .warning(target: "game.exe", secondsLeft: 9))
    for elapsed in [0.0, 2.999] {
        #expect(deriveOverlay(status: warning, block: notice, at: date.addingTimeInterval(elapsed)) ==
                .blockCard(target: "blocked.exe", reason: "daily budget exhausted"))
    }
    #expect(deriveOverlay(status: warning, block: notice, at: date.addingTimeInterval(3)) ==
            .warning(target: "game.exe", secondsLeft: 9))
    #expect(deriveOverlay(status: status(state: "exceeded"), block: notice,
                          at: date.addingTimeInterval(3)) == .hidden)
    #expect(deriveOverlay(status: nil, block: nil, at: date) == .hidden)
    #expect(deriveOverlay(status: status(), block: nil, at: date) == .hidden)
}

@Test func durationsAndPaths() {
    for (seconds, expected): (UInt64, String) in [(0, "0s"), (9, "9s"), (60, "1m 00s"), (123, "2m 03s"),
                                                (1251, "20m 51s"), (3599, "59m 59s"), (3600, "1h 00m"),
                                                (5940, "1h 39m"), (7260, "2h 01m")] {
        #expect(formatDuration(seconds) == expected)
    }
    #expect(SocketClient.defaultPath(environment: [:], home: URL(fileURLWithPath: "/Users/test")) ==
            "/Users/test/Library/Application Support/roguestill/roguestilld.sock")
    #expect(SocketClient.defaultPath(environment: ["ROGUESTILL_SOCKET": "/tmp/override"]) == "/tmp/override")
    #expect(SocketClient().timeout == 3)
}

private actor ScriptedClient: StatusClient {
    var results: [Result<Data, SocketError>]
    var calls = 0

    init(_ results: [Result<Data, SocketError>]) { self.results = results }

    func fetch() async throws -> Data {
        calls += 1
        return try results.removeFirst().get()
    }
}

@Test @MainActor func storeTransitions() async throws {
    let client = ScriptedClient([
        .success(try JSONEncoder().encode(status(state: "warning"))),
        .failure(.timedOut),
        .success(try JSONEncoder().encode(status(1, state: "exceeded"))),
        .success(Data(#"{"v":2}"#.utf8))
    ])
    let store = StatusStore(client: client, now: { date })
    #expect(store.overlay == .hidden)
    await store.poll()
    #expect(store.lastSeen == date)
    #expect(store.overlay == .warning(target: "game.exe", secondsLeft: 9))
    await store.poll()
    #expect(store.connection == .daemonDown(lastKnown: status(state: "warning"), since: date))
    #expect(store.lastSeen == date)
    #expect(store.overlay == .hidden)
    await store.poll()
    #expect(store.overlay == .blockCard(target: "game.exe", reason: "daily budget exhausted"))
    await store.poll()
    #expect(store.connection == .unsupportedVersion(2))
    #expect(store.overlay == .hidden)
    store.stop()
}

@Test @MainActor func blockExpiresWithoutAnotherPoll() async throws {
    let client = ScriptedClient([
        .success(try JSONEncoder().encode(status())),
        .success(try JSONEncoder().encode(status(1, state: "exceeded")))
    ])
    let store = StatusStore(client: client)
    await store.poll()
    await store.poll()
    #expect(store.overlay == .blockCard(target: "game.exe", reason: "daily budget exhausted"))
    try await Task.sleep(for: .milliseconds(3200))
    #expect(store.overlay == .hidden)
    #expect(await client.calls == 2)
    store.stop()
}

private actor CountingClient: StatusClient {
    var calls = 0
    func fetch() async throws -> Data {
        calls += 1
        return Data(#"{"v":1,"rules":[]}"#.utf8)
    }
}

@Test @MainActor func pollingStartsOnceAndStops() async throws {
    let client = CountingClient()
    let store = StatusStore(client: client, pollInterval: .milliseconds(10))
    store.start()
    store.start()
    try await Task.sleep(for: .milliseconds(100))
    store.stop()
    let calls = await client.calls
    #expect(calls >= 2)
    try await Task.sleep(for: .milliseconds(50))
    #expect(await client.calls == calls)
}

private func withServer(
    serve: @escaping @Sendable (Int32) throws -> Void,
    check: (String) async throws -> Void
) async throws {
    let path = "/tmp/rl-\(UUID().uuidString).sock"
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    #expect(fd >= 0)
    defer { close(fd); unlink(path) }
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
    withUnsafeMutableBytes(of: &address.sun_path) { $0.copyBytes(from: Array(path.utf8) + [0]) }
    let bound = withUnsafePointer(to: &address) {
        $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
            bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
        }
    }
    let bindError = errno
    try #require(bound == 0, "Unix socket bind failed: errno \(bindError) (\(String(cString: strerror(bindError))))")
    try #require(listen(fd, 1) == 0)
    let server = Task.detached {
        var ready = pollfd(fd: fd, events: Int16(POLLIN), revents: 0)
        try #require(Darwin.poll(&ready, 1, 5000) > 0)
        let connection = accept(fd, nil, nil)
        try #require(connection >= 0)
        defer { close(connection) }
        var noSignal: Int32 = 1
        setsockopt(connection, SOL_SOCKET, SO_NOSIGPIPE, &noSignal, socklen_t(MemoryLayout<Int32>.size))
        try serve(connection)
    }
    do {
        try await check(path)
        try await server.value
    } catch {
        _ = try? await server.value
        throw error
    }
}

@Test func socketReadsFragmentsThroughEOF() async throws {
    let payload = try JSONEncoder().encode(status(7)) + Data("\n".utf8)
    try await withServer { fd in
        for byte in payload {
            var value = byte
            try #require(write(fd, &value, 1) == 1)
        }
    } check: { path in
        let data = try await SocketClient(path: path).fetch()
        #expect(data == payload)
        #expect(try JSONDecoder().decode(Status.self, from: data) == status(7))
    }
}

@Test func socketWaitsForEOFAndTimesOut() async throws {
    try await withServer { fd in
        var newline: UInt8 = 10
        try #require(write(fd, &newline, 1) == 1)
        usleep(200_000)
    } check: { path in
        await #expect(throws: SocketError.timedOut) {
            _ = try await SocketClient(path: path, timeout: 0.03).fetch()
        }
    }
}

@Test func socketErrorsAndEmptyEOF() async throws {
    await #expect(throws: SocketError.invalidPath) {
        _ = try await SocketClient(path: String(repeating: "x", count: 200)).fetch()
    }
    await #expect(throws: SocketError.invalidPath) {
        _ = try await SocketClient(path: "/tmp/a\0b").fetch()
    }
    await #expect(throws: (any Error).self) {
        _ = try await SocketClient(path: "/tmp/missing-\(UUID().uuidString)").fetch()
    }
    try await withServer { _ in } check: { path in
        let data = try await SocketClient(path: path).fetch()
        #expect(data.isEmpty)
    }
}

@Test func malformedStatusIsOffline() {
    for json in [#"{"v":1}"#, #"{"v":"1","rules":[]}"#,
                 #"{"v":1,"rules":[{"name":"game"}]}"#] {
        let result = reduce(previous: .connected(status(), at: date),
                            result: .success(Data(json.utf8)), at: date)
        #expect(result.state == .daemonDown(lastKnown: status(), since: date))
        #expect(result.events.isEmpty)
    }
}

private actor SuspendedClient: StatusClient {
    private var continuation: CheckedContinuation<Data, Never>?
    var started: Bool { continuation != nil }

    func fetch() async throws -> Data {
        await withCheckedContinuation { continuation = $0 }
    }

    func finish() {
        continuation?.resume(returning: Data(#"{"v":1,"rules":[]}"#.utf8))
        continuation = nil
    }
}

@Test @MainActor func stopDiscardsInFlightResult() async {
    let client = SuspendedClient()
    let store = StatusStore(client: client, now: { date })
    let polling = Task { await store.poll() }
    while !(await client.started) { await Task.yield() }
    await store.poll()
    store.stop()
    await client.finish()
    await polling.value
    #expect(store.connection == .daemonDown(lastKnown: nil, since: date))
    #expect(store.lastSeen == nil)
    #expect(store.overlay == .hidden)
}

@Test func overlayContentForWarningAndBlock() {
    let warning = overlayContent(for: .warning(target: "noita.exe", secondsLeft: 4))
    #expect(warning?.title == "NOITA CLOSES IN 4")
    #expect(warning?.detail == nil)
    #expect(warning?.progress == 0.4)
    #expect(warning?.gameKey == "noita")
    let block = overlayContent(for: .blockCard(target: "noita.exe", reason: "daily budget exhausted"))
    #expect(block?.title == "NOITA BLOCKED")
    #expect(block?.detail == "DAILY BUDGET EXHAUSTED")
    #expect(block?.progress == nil)
    #expect(overlayContent(for: .hidden) == nil)
    #expect(overlayContent(for: .warning(target: "Game", secondsLeft: 30))?.progress == 1)
}

@Test func snapshotFromConnection() {
    let live = Snapshot(connection: .connected(status(), at: date))
    #expect(live == Snapshot(kind: .connected, status: status(), updatedAt: date))
    let down = Snapshot(connection: .daemonDown(lastKnown: status(), since: date))
    #expect(down == Snapshot(kind: .daemonDown, status: status(), updatedAt: date))
    let never = Snapshot(connection: .daemonDown(lastKnown: nil, since: date))
    #expect(never.status == nil)
    let newer = Snapshot(connection: .unsupportedVersion(2), now: date)
    #expect(newer == Snapshot(kind: .unsupportedVersion, status: nil, updatedAt: date))
}

@Test func snapshotFileRoundTrip() throws {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let file = dir.appendingPathComponent("nested/snapshot.json")
    #expect(Snapshot.read(from: file) == nil)
    let snapshot = Snapshot(kind: .connected, status: status(3), updatedAt: date)
    try snapshot.write(to: file)
    #expect(Snapshot.read(from: file) == snapshot)
    try Data("junk".utf8).write(to: file)
    #expect(Snapshot.read(from: file) == nil)
}

@Test func widgetReloadThrottle() {
    let a = Snapshot(kind: .connected, status: status(), updatedAt: date)
    let sameLater = Snapshot(kind: .connected, status: status(), updatedAt: date.addingTimeInterval(5))
    let blocked = Snapshot(kind: .connected, status: status(1, state: "exceeded"), updatedAt: date.addingTimeInterval(5))
    let down = Snapshot(kind: .daemonDown, status: status(), updatedAt: date.addingTimeInterval(5))
    #expect(shouldReloadWidget(previous: nil, next: a, lastReload: nil, now: date))
    #expect(!shouldReloadWidget(previous: a, next: sameLater, lastReload: date, now: date.addingTimeInterval(5)))
    #expect(shouldReloadWidget(previous: a, next: sameLater, lastReload: date, now: date.addingTimeInterval(60)))
    #expect(shouldReloadWidget(previous: a, next: blocked, lastReload: date, now: date.addingTimeInterval(5)))
    #expect(shouldReloadWidget(previous: a, next: down, lastReload: date, now: date.addingTimeInterval(5)))
}

@Test func agentMessages() {
    #expect(agentMessage(for: .enabled) == nil)
    #expect(agentMessage(for: .notRegistered) == "Daemon not installed")
    #expect(agentMessage(for: .requiresApproval) == "Allow Roguestill in Login Items")
    #expect(agentMessage(for: .unavailable) == "Daemon missing from the app")
    #expect(agentNeedsSettings(.requiresApproval))
    #expect(!agentNeedsSettings(.enabled))
}

private let sampleConfig = #"{"noita_save_path":"~/save00","rule":[{"name":"Noita","target":{"process":"noita.exe"},"adapter":"noita","budget":{"kind":"daily_time","limit_minutes":90,"reset_hour":0,"idle_pause_minutes":5},"action":{"warn_seconds":10,"kill":true,"relaunch":"kill_again"}}]}"#

@Test func rulesConfigRoundTrip() throws {
    var config = try JSONDecoder().decode(RulesConfig.self, from: Data(sampleConfig.utf8))
    #expect(config.rule[0].name == "Noita")
    #expect(config.rule[0].budget.limitMinutes == 90)
    #expect(config.rule[0].action.kill)
    config.rule[0].budget.limitMinutes = 120
    let object = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(config)) as? [String: Any])
    let rule = try #require((object["rule"] as? [[String: Any]])?.first)
    let budget = try #require(rule["budget"] as? [String: Any])
    #expect(budget["limit_minutes"] as? Int == 120)
    #expect(budget["kind"] as? String == "daily_time")
    #expect(object["noita_save_path"] as? String == "~/save00")
    #expect(try JSONDecoder().decode(RulesConfig.self, from: JSONEncoder().encode(config)) == config)
}

@Test func newGameRuleHasSaneDefaults() {
    let rule = GameRule.new(name: "Hades", process: "Hades.exe")
    #expect(rule.budget.limitMinutes == 60 && rule.action.warnSeconds == 10 && rule.action.kill)
    #expect(rule.adapter == nil)
}

@Test func processConfigClientLoadsAndSaves() async throws {
    let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
    let out = dir.appendingPathComponent("saved.json")
    let script = dir.appendingPathComponent("cli.sh")
    try """
    #!/bin/sh
    if [ "$1" = "config" ]; then echo '\(sampleConfig)'; else cat > '\(out.path)'; fi
    """.write(to: script, atomically: true, encoding: .utf8)
    try FileManager.default.setAttributes([.posixPermissions: 0o755], ofItemAtPath: script.path)
    let client = ProcessConfigClient(executable: script)
    var config = try await client.load()
    #expect(config.rule.count == 1)
    config.rule[0].budget.limitMinutes = 45
    try await client.save(config)
    let saved = try JSONDecoder().decode(RulesConfig.self, from: Data(contentsOf: out))
    #expect(saved.rule[0].budget.limitMinutes == 45)
    let failing = ProcessConfigClient(executable: dir.appendingPathComponent("missing"))
    await #expect(throws: (any Error).self) { try await failing.load() }
}

@Test func runLimitInStatusAndConfig() throws {
    let json = #"{"v":1,"rules":[{"name":"g","target":"g.exe","used_secs":1,"remaining_secs":2,"runs_today":2,"degraded":false,"state":"ok","limit_runs":5}]}"#
    let decoded = try JSONDecoder().decode(Status.self, from: Data(json.utf8))
    #expect(decoded.rules[0].limitRuns == 5)
    #expect(runsText(decoded.rules[0]) == "Runs today: 2 of 5")
    #expect(runsText(rule()) == "Runs today: 2")
    var config = try JSONDecoder().decode(RulesConfig.self, from: Data(sampleConfig.utf8))
    #expect(config.rule[0].budget.limitRuns == nil)
    config.rule[0].budget.limitRuns = 3
    let object = try #require(JSONSerialization.jsonObject(with: JSONEncoder().encode(config)) as? [String: Any])
    let budget = try #require(((object["rule"] as? [[String: Any]])?.first)?["budget"] as? [String: Any])
    #expect(budget["limit_runs"] as? Int == 3)
}
