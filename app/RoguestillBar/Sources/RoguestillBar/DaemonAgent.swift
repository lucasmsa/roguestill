import Foundation
import RoguestillKit
import ServiceManagement

let daemonPlist = "com.lucasmsa.roguestill.daemon.plist"

func daemonStatus() -> AgentStatus {
    switch SMAppService.agent(plistName: daemonPlist).status {
    case .enabled: .enabled
    case .notRegistered: .notRegistered
    case .requiresApproval: .requiresApproval
    default: .unavailable
    }
}

func installDaemon() -> AgentStatus {
    if daemonStatus() != .enabled {
        do { try SMAppService.agent(plistName: daemonPlist).register() } catch {
            FileHandle.standardError.write(Data("register: \(error)\n".utf8))
        }
    }
    return daemonStatus()
}

func openLoginItems() { SMAppService.openSystemSettingsLoginItems() }
