import Darwin
import Foundation

public protocol StatusClient: Sendable {
    func fetch() async throws -> Data
}

public enum SocketError: Error, Equatable {
    case invalidPath
    case system(Int32)
    case timedOut
}

public struct SocketClient: StatusClient {
    public let path: String
    public let timeout: TimeInterval

    public static func defaultPath(environment: [String: String] = ProcessInfo.processInfo.environment,
                                   home: URL = FileManager.default.homeDirectoryForCurrentUser) -> String {
        environment["RUN_LIMITER_SOCKET"] ?? home.appendingPathComponent("Library/Application Support/run-limiter/limiterd.sock").path
    }

    public init(path: String = SocketClient.defaultPath(), timeout: TimeInterval = 3) {
        self.path = path
        self.timeout = timeout
    }

    public func fetch() async throws -> Data {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .utility).async {
                continuation.resume(with: Result { try readSocket() })
            }
        }
    }

    private func readSocket() throws -> Data {
        var address = sockaddr_un()
        let bytes = Array(path.utf8)
        guard !bytes.contains(0), bytes.count < MemoryLayout.size(ofValue: address.sun_path) else {
            throw SocketError.invalidPath
        }
        address.sun_family = sa_family_t(AF_UNIX)
        address.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutableBytes(of: &address.sun_path) { destination in
            destination.copyBytes(from: bytes + [0])
        }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw SocketError.system(errno) }
        defer { close(fd) }
        guard fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else { throw SocketError.system(errno) }
        let deadline = ProcessInfo.processInfo.systemUptime + timeout
        let connected = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }
        if connected != 0 {
            guard errno == EINPROGRESS else { throw SocketError.system(errno) }
            try wait(fd, events: Int16(POLLOUT), deadline: deadline)
            var error: Int32 = 0
            var length = socklen_t(MemoryLayout<Int32>.size)
            guard getsockopt(fd, SOL_SOCKET, SO_ERROR, &error, &length) == 0 else {
                throw SocketError.system(errno)
            }
            guard error == 0 else { throw SocketError.system(error) }
        }
        var data = Data()
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            try wait(fd, events: Int16(POLLIN), deadline: deadline)
            let count = read(fd, &buffer, buffer.count)
            if count == 0 { return data }
            if count > 0 { data.append(contentsOf: buffer.prefix(count)) }
            else if errno != EINTR && errno != EAGAIN { throw SocketError.system(errno) }
        }
    }

    private func wait(_ fd: Int32, events: Int16, deadline: TimeInterval) throws {
        while true {
            let remaining = deadline - ProcessInfo.processInfo.systemUptime
            guard remaining > 0 else { throw SocketError.timedOut }
            var descriptor = pollfd(fd: fd, events: events, revents: 0)
            let result = poll(&descriptor, 1, Int32(min(remaining * 1000 + 1, Double(Int32.max))))
            if result > 0 { return }
            if result == 0 { throw SocketError.timedOut }
            if errno != EINTR { throw SocketError.system(errno) }
        }
    }
}
