import AppKit
import CoreText
import Combine
import RoguestillKit
import SwiftUI
import WidgetKit

struct Appearance {
    let normalIcon = "hourglass"
    let failureIcon = "exclamationmark.triangle"
    let shieldIcon = "shield.fill"
    let panelSize = CGSize(width: 360, height: 104)
    let screenInset: CGFloat = 20
    let spacing: CGFloat = 12
    let ground = Color.black
    let frame = Color(red: 0.36, green: 0.23, blue: 0.18)
    let text = Color(red: 0.94, green: 0.90, blue: 0.85)
    let barFill = Color(red: 1.0, green: 0.65, blue: 0.24)
    let barTrack = Color(red: 0.17, green: 0.11, blue: 0.08)
}

@MainActor
final class AppModel: ObservableObject {
    let store: StatusStore
    let appearance: Appearance
    private let panel: NSPanel
    private var subscriptions: Set<AnyCancellable> = []
    @Published var agent: AgentStatus = installDaemon()
    private var lastSnapshot: Snapshot?
    private var lastReload: Date?

    init(appearance: Appearance = Appearance()) {
        self.appearance = appearance
        registerPixelFont()
        store = StatusStore()
        panel = NSPanel(contentRect: NSRect(origin: .zero, size: appearance.panelSize),
                        styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.level = .screenSaver
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true
        panel.hidesOnDeactivate = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        store.$overlay.sink { [weak self] state in self?.showOverlay(state) }.store(in: &subscriptions)
        store.$connection.sink { [weak self] connection in self?.publish(connection) }.store(in: &subscriptions)
        store.objectWillChange.sink { [weak self] _ in self?.objectWillChange.send() }.store(in: &subscriptions)
        store.start()
    }

    var icon: String {
        if case .connected = store.connection { return appearance.normalIcon }
        return appearance.failureIcon
    }

    var rows: [RuleRow] {
        (store.connection.status?.rules ?? []).map {
            RuleRow(name: $0.name, usage: "Used \(formatDuration($0.usedSecs)), remaining \(formatDuration($0.remainingSecs))",
                    runs: runsText($0), degraded: $0.degraded)
        }
    }

    var offline: Bool {
        if case .daemonDown = store.connection { return true }
        return false
    }

    var message: String? {
        switch store.connection {
        case .connected: nil
        case .daemonDown: agentMessage(for: agent) ?? "Daemon not running"
        case .unsupportedVersion: "Update the app"
        }
    }

    var lastSeen: String? {
        guard offline, let date = store.lastSeen else { return nil }
        let formatter = DateFormatter()
        formatter.dateFormat = "HH:mm:ss"
        return "last seen \(formatter.string(from: date))"
    }

    var needsSettings: Bool { agentNeedsSettings(agent) }

    func refreshAgent() { agent = installDaemon() }

    func quit() {
        store.stop()
        NSApplication.shared.terminate(nil)
    }

    private func publish(_ connection: ConnectionState) {
        guard let url = snapshotURL() else { return }
        let snapshot = Snapshot(connection: connection)
        try? snapshot.write(to: url)
        let now = Date()
        if shouldReloadWidget(previous: lastSnapshot, next: snapshot, lastReload: lastReload, now: now) {
            WidgetCenter.shared.reloadAllTimelines()
            lastReload = now
        }
        lastSnapshot = snapshot
    }

    private func showOverlay(_ state: OverlayState) {
        guard state != .hidden, let screen = NSScreen.main else {
            panel.orderOut(nil)
            return
        }
        guard let content = overlayContent(for: state) else { return }
        panel.contentView = NSHostingView(rootView: OverlayView(content: content, icon: gameIcon(content.gameKey),
                                                                appearance: appearance))
        let frame = screen.visibleFrame
        panel.setFrameOrigin(NSPoint(x: frame.maxX - appearance.panelSize.width - appearance.screenInset,
                                     y: frame.maxY - appearance.panelSize.height - appearance.screenInset))
        panel.orderFrontRegardless()
    }
}

struct RuleRow: Identifiable {
    let name: String
    let usage: String
    let runs: String
    let degraded: Bool
    var id: String { name }
}

struct MenuContent: View {
    let rows: [RuleRow]
    let message: String?
    let lastSeen: String?
    let offline: Bool
    let needsSettings: Bool
    let openSettings: () -> Void
    let quit: () -> Void

    @Environment(\.openWindow) private var openWindow

    var body: some View {
        if let message { Text(message) }
        if let lastSeen { Text(lastSeen).foregroundStyle(.secondary) }
        ForEach(rows) { row in
            Section {
                Text(row.name)
                Text(row.usage)
                Text(row.runs)
                if row.degraded { Text("degraded") }
            }
            .foregroundStyle(offline ? .secondary : .primary)
        }
        if needsSettings { Button("Open Login Items", action: openSettings) }
        Divider()
        Button("Settings…") {
            openWindow(id: "settings")
            NSApplication.shared.activate(ignoringOtherApps: true)
        }
        .keyboardShortcut(",")
        Button("Quit", action: quit).keyboardShortcut("q")
    }
}

struct OverlayView: View {
    let content: OverlayContent
    let icon: NSImage?
    let appearance: Appearance

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: appearance.spacing) {
                gameMark
                Text(content.title)
            }
            if let detail = content.detail { Text(detail) }
            if let progress = content.progress {
                GeometryReader { box in
                    Rectangle().fill(appearance.barFill).frame(width: box.size.width * progress)
                }
                .frame(height: 12)
                .background(appearance.barTrack)
                .overlay(Rectangle().stroke(appearance.frame, lineWidth: 2))
            }
        }
        .font(.custom("Silkscreen-Regular", size: 15))
        .foregroundStyle(appearance.text)
        .padding(appearance.spacing)
        .frame(width: appearance.panelSize.width, height: appearance.panelSize.height, alignment: .leading)
        .background(appearance.ground)
        .overlay(Rectangle().stroke(appearance.frame, lineWidth: 4))
    }

    @ViewBuilder private var gameMark: some View {
        if let icon {
            Image(nsImage: icon).resizable().interpolation(.none).frame(width: 32, height: 32)
                .accessibilityHidden(true)
        } else {
            Image(systemName: appearance.shieldIcon).font(.title).foregroundStyle(.gray).accessibilityHidden(true)
        }
    }
}

#if SWIFT_PACKAGE
let appResources = Bundle.module
#else
let appResources = Bundle.main
#endif

@MainActor
func gameIcon(_ key: String) -> NSImage? {
    appResources.url(forResource: key, withExtension: "png", subdirectory: "Resources").flatMap(NSImage.init(contentsOf:))
}

func registerPixelFont() {
    guard let url = appResources.url(forResource: "Silkscreen-Regular", withExtension: "ttf", subdirectory: "Resources")
    else { return }
    CTFontManagerRegisterFontsForURL(url as CFURL, .process, nil)
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApplication.shared.setActivationPolicy(.accessory)
    }
}

@main
struct RoguestillBarApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var model = AppModel()
    @StateObject private var settings = SettingsModel()

    var body: some Scene {
        MenuBarExtra("roguestill", systemImage: model.icon) {
            MenuContent(rows: model.rows, message: model.message, lastSeen: model.lastSeen,
                        offline: model.offline, needsSettings: model.needsSettings,
                        openSettings: { openLoginItems(); model.refreshAgent() }, quit: model.quit)
        }
        Window("Roguestill Settings", id: "settings") {
            SettingsView(model: settings)
        }
        .windowResizability(.contentSize)
    }
}
