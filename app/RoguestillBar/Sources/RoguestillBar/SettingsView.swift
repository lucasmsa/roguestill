import AppKit
import RoguestillKit
import SwiftUI

struct SettingsView: View {
    @ObservedObject var model: SettingsModel
    @State private var newName = ""
    @State private var newProcess = ""

    var body: some View {
        Form {
            if let error = model.error { Text(error).foregroundStyle(.red) }
            if model.config != nil {
                ForEach(model.config?.rule.indices ?? 0..<0, id: \.self) { index in
                    RuleSection(rule: ruleBinding(index), canRemove: (model.config?.rule.count ?? 0) > 1,
                                remove: { model.remove(index) })
                }
                Section("Add a game or group") {
                    TextField("Group or game name (for example Social media)", text: $newName)
                    TextField("First game or app (for example Reddit)", text: $newProcess)
                    Button("Add") {
                        model.addGame(name: newName, process: newProcess)
                        newName = ""
                        newProcess = ""
                    }
                    .disabled(newName.trimmingCharacters(in: .whitespaces).isEmpty
                              || newProcess.trimmingCharacters(in: .whitespaces).isEmpty)
                }
            }
            Section("App") {
                Toggle("Open Roguestill at login", isOn: Binding(get: { model.launchAtLogin },
                                                                   set: { model.setLaunchAtLogin($0) }))
            }
        }
        .formStyle(.grouped)
        .frame(width: 460, height: 560)
        .task { await model.load() }
        .onChange(of: model.config) { _, _ in model.changed() }
    }

    private func ruleBinding(_ index: Int) -> Binding<GameRule> {
        Binding(get: { model.config?.rule[index] ?? .new(name: "", process: "") },
                set: { model.config?.rule[index] = $0 })
    }
}

struct MembersEditor: View {
    @Binding var rule: GameRule
    @State private var draft = ""

    var body: some View {
        ForEach(rule.target.processes, id: \.self) { member in
            LabeledContent(rule.target.processes.count > 1 ? "Member" : "Game") {
                HStack {
                    Text(member).foregroundStyle(.secondary)
                    if rule.target.processes.count > 1 {
                        Button("Remove", systemImage: "minus.circle") { rule.removeMember(member) }
                            .labelStyle(.iconOnly).buttonStyle(.borderless)
                    }
                }
            }
        }
        HStack {
            TextField("Add to this group (for example Hades)", text: $draft)
                .onSubmit(add)
            Menu("Running apps") {
                ForEach(runningAppNames(), id: \.self) { name in Button(name) { rule.addMember(name) } }
            }
            .fixedSize()
            Button("Add", action: add).disabled(draft.trimmingCharacters(in: .whitespaces).isEmpty)
        }
    }

    private func add() {
        rule.addMember(draft)
        draft = ""
    }
}

func runningAppNames() -> [String] {
    Set(NSWorkspace.shared.runningApplications.filter { $0.activationPolicy == .regular }.compactMap(\.localizedName))
        .sorted { $0.localizedCaseInsensitiveCompare($1) == .orderedAscending }
}

struct RuleSection: View {
    @Binding var rule: GameRule
    let canRemove: Bool
    let remove: () -> Void

    var body: some View {
        Section(rule.name) {
            MembersEditor(rule: $rule)
            Stepper("Daily limit: \(formatDuration(UInt64(rule.budget.limitMinutes) * 60))", value: $rule.budget.limitMinutes, in: 1...1440, step: 5)
            Toggle("Limit runs per day", isOn: Binding(get: { rule.budget.limitRuns != nil },
                                                        set: { rule.budget.limitRuns = $0 ? 3 : nil }))
            if let runs = rule.budget.limitRuns {
                Stepper("Runs per day: \(runs)", value: Binding(get: { runs }, set: { rule.budget.limitRuns = $0 }), in: 1...50)
            }
            Stepper("Warning before closing: \(rule.action.warnSeconds)s", value: $rule.action.warnSeconds, in: 0...120, step: 5)
            Stepper("Pause when idle after: \(formatDuration(UInt64(rule.budget.idlePauseMinutes) * 60))", value: $rule.budget.idlePauseMinutes, in: 0...120)
            Picker("Day resets at", selection: $rule.budget.resetHour) {
                ForEach(0..<24, id: \.self) { Text(String(format: "%02d:00", $0)).tag($0) }
            }
            Toggle("Close the game when time is up", isOn: $rule.action.kill)
            Toggle("Lock limits for the day", isOn: $rule.lockChanges)
            if rule.lockChanges {
                Text("Edits to this game, including turning the lock off, take effect at the next day reset.")
                    .font(.caption).foregroundStyle(.secondary)
            }
            if canRemove { Button("Remove \(rule.name)", role: .destructive, action: remove) }
        }
    }
}
