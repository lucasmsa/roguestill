import RoguestillKit
import SwiftUI
import WidgetKit

struct Entry: TimelineEntry {
    let date: Date
    let snapshot: Snapshot?
}

struct Provider: TimelineProvider {
    func placeholder(in context: Context) -> Entry { Entry(date: Date(), snapshot: nil) }

    func getSnapshot(in context: Context, completion: @escaping (Entry) -> Void) {
        completion(current())
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<Entry>) -> Void) {
        completion(Timeline(entries: [current()], policy: .after(Date().addingTimeInterval(300))))
    }

    private func current() -> Entry {
        Entry(date: Date(), snapshot: snapshotURL().flatMap { Snapshot.read(from: $0) })
    }
}

struct WidgetStyle {
    let ground = Color.black
    let frame = Color(red: 0.36, green: 0.23, blue: 0.18)
    let text = Color(red: 0.94, green: 0.90, blue: 0.85)
    let dim = Color(red: 0.94, green: 0.90, blue: 0.85).opacity(0.6)
    let barFill = Color(red: 1.0, green: 0.65, blue: 0.24)
    let barTrack = Color(red: 0.17, green: 0.11, blue: 0.08)
}

struct RoguestillWidgetView: View {
    let entry: Entry
    private let style = WidgetStyle()

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let message { Text(message).font(.caption.bold()) }
            ForEach(entry.snapshot?.status?.rules ?? [], id: \.name) { rule in
                VStack(alignment: .leading, spacing: 3) {
                    Text(rule.name.uppercased()).font(.caption.bold())
                    Text("LEFT \(formatDuration(rule.remainingSecs))").font(.title3.monospacedDigit().bold())
                    bar(rule)
                    Text("USED \(formatDuration(rule.usedSecs)) · RUNS \(rule.runsToday)").font(.caption2)
                }
            }
            if entry.snapshot == nil { Text("OPEN THE APP").font(.caption2) }
            if let stamp { Text(stamp).font(.caption2) }
        }
        .foregroundStyle(offline ? style.dim : style.text)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .containerBackground(style.ground, for: .widget)
    }

    private var offline: Bool { entry.snapshot?.kind != .connected }

    private var message: String? {
        switch entry.snapshot?.kind {
        case .daemonDown: "DAEMON NOT RUNNING"
        case .unsupportedVersion: "UPDATE THE APP"
        default: nil
        }
    }

    private var stamp: String? {
        guard offline, let snapshot = entry.snapshot, snapshot.status != nil else { return nil }
        return "LAST SEEN " + snapshot.updatedAt.formatted(date: .omitted, time: .shortened)
    }

    private func bar(_ rule: RuleStatus) -> some View {
        let total = Double(rule.usedSecs + rule.remainingSecs)
        let fraction = total > 0 ? Double(rule.remainingSecs) / total : 0
        return GeometryReader { box in
            Rectangle().fill(style.barFill).frame(width: box.size.width * fraction)
        }
        .frame(height: 8)
        .background(style.barTrack)
        .overlay(Rectangle().stroke(style.frame, lineWidth: 2))
    }
}

@main
struct RoguestillWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "RoguestillWidget", provider: Provider()) { entry in
            RoguestillWidgetView(entry: entry)
        }
        .configurationDisplayName("Roguestill")
        .description("Time left today for each limited game.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}
