import AppKit
import SwiftUI

/// A Settings row, declared once and used both to draw the page and to search it.
struct RowSpec: Identifiable {
    enum Control {
        case toggle(Binding<Bool>, enabled: Bool)
        case choice([(tag: String, label: String)], Binding<String>, enabled: Bool)
        case button(String, prominent: Bool, destructive: Bool, action: () -> Void)
        case value(String)
        case view(AnyView)
        case none
    }
    let title: String
    let detail: String?
    var mono = false
    let control: Control
    var id: String { title }
}

struct SectionSpec: Identifiable {
    let title: String?
    var footer: String? = nil
    let rows: [RowSpec]
    var id: String { (title ?? "") + rows.map(\.title).joined() }
}

/// The nine pages of SettingsMac.dc.html. Every control reads and writes one key of the
/// engine's settings store; changes made in the panel, the menu or the CLI arrive live.
@MainActor
enum SettingsCatalog {
    static func rows(for page: Page, model: NativeUI) -> [RowSpec] {
        sections(for: page, model: model).flatMap(\.rows)
    }

    // MARK: Row builders

    static func toggle(_ model: NativeUI, _ title: String, _ detail: String? = nil, key: String, inverted: Bool = false, enabled: Bool = true) -> RowSpec {
        RowSpec(title: title, detail: detail, control: .toggle(Binding(
            get: { model.bool(key) != inverted },
            set: { model.set(key, $0 != inverted) }), enabled: enabled))
    }

    static func command(_ model: NativeUI, _ title: String, _ detail: String? = nil, on: Bool, command: String, enabled: Bool = true) -> RowSpec {
        RowSpec(title: title, detail: detail, control: .toggle(Binding(get: { on }, set: { _ in model.send(command) }), enabled: enabled))
    }

    static func minutes(_ model: NativeUI, _ title: String, _ detail: String? = nil, key: String, choices: [Int], fallback: Int) -> RowSpec {
        let current = model.int(key, fallback)
        let values: [Int] = Array(Set(choices + [current])).sorted()
        let options: [(tag: String, label: String)] = values.map { (tag: String($0), label: durationLabel(minutes: $0)) }
        return RowSpec(title: title, detail: detail, control: .choice(options, Binding(
            get: { String(model.int(key, fallback)) },
            set: { model.set(key, Int($0) ?? fallback) }), enabled: true))
    }

    static func seconds(_ model: NativeUI, _ title: String, _ detail: String? = nil, key: String, choices: [Int], fallback: Int, enabled: Bool = true) -> RowSpec {
        let current = model.int(key, fallback)
        let values: [Int] = Array(Set(choices + [current])).sorted()
        let options: [(tag: String, label: String)] = values.map { (tag: String($0), label: durationLabel(seconds: $0)) }
        return RowSpec(title: title, detail: detail, control: .choice(options, Binding(
            get: { String(model.int(key, fallback)) },
            set: { model.set(key, Int($0) ?? fallback) }), enabled: enabled))
    }

    static func action(_ model: NativeUI, _ title: String, _ detail: String? = nil, key: String, allowNothing: Bool = false, enabled: Bool = true) -> RowSpec {
        var options: [(tag: String, label: String)] = model.actions.map { (tag: $0, label: actionLabel($0)) }
        if allowNothing { options.insert((tag: "nothing", label: "Nothing"), at: 0) }
        return RowSpec(title: title, detail: detail, control: .choice(options, Binding(
            get: { model.string(key) ?? (allowNothing ? "nothing" : "sleep") },
            set: { model.set(key, $0 == "nothing" ? nil : $0) }), enabled: enabled))
    }

    static func button(_ title: String, _ detail: String? = nil, mono: Bool = false, label: String, prominent: Bool = false, destructive: Bool = false, action: @escaping () -> Void) -> RowSpec {
        RowSpec(title: title, detail: detail, mono: mono, control: .button(label, prominent: prominent, destructive: destructive, action: action))
    }

    // MARK: Pages

    static func sections(for page: Page, model: NativeUI) -> [SectionSpec] {
        let session = model.session
        let audio = model.snapshot.bool("audioSupported")
        switch page {
        case .overview:
            return OverviewPage.sections(model)

        case .general:
            var awake = [
                toggle(model, "Keep the display on too", "When off, the screen can sleep while your Mac stays awake.", key: "allowDisplaySleep", inverted: true),
            ]
            // Only MacBooks have a lid to keep open.
            if model.snapshot.bool("lidClosedSupported") {
                awake.append(toggle(model, "Stay awake with the lid closed", "Only while Doze is keeping your Mac awake.", key: "lidClosedKeepAwake"))
            }
            let floors: [Int] = Array(Set([0, 10, 15, 20, 25, 30, 50, model.int("batteryFloorPercent", 15)])).sorted()
            let floorOptions: [(tag: String, label: String)] = floors.map { percent in
                (tag: String(percent), label: percent == 0 ? "Never" : "\(percent)% battery")
            }
            let floorBinding = Binding<String>(get: { String(model.int("batteryFloorPercent", 15)) },
                                               set: { model.set("batteryFloorPercent", Int($0) ?? 15) })
            awake.append(RowSpec(title: "Stop keeping awake below", detail: "On battery only.",
                                 control: .choice(floorOptions, floorBinding, enabled: true)))
            return [
                SectionSpec(title: "Startup", rows: [
                    toggle(model, "Open Doze at login", key: "launchAtStartup", enabled: model.snapshot.bool("startupSupported")),
                ]),
                SectionSpec(title: "Menu bar", rows: [
                    RowSpec(title: "Clicking the icon opens", detail: "Right-click or ⌥-click always opens the other one.", control: .choice(
                        [(tag: "panel", label: "Panel"), (tag: "menu", label: "Menu")],
                        Binding(get: { model.string("iconClickOpens") ?? "panel" }, set: { model.set("iconClickOpens", $0) }), enabled: true)),
                    toggle(model, "Show time left next to the icon", key: "menuBarTime"),
                    toggle(model, "Show agent count next to the icon", key: "menuBarAgentCount"),
                ]),
                SectionSpec(title: "While keeping awake",
                            footer: model.snapshot.bool("lidClosedSupported")
                                ? "With the lid closed, keep your Mac on power and somewhere it can stay cool." : nil,
                            rows: awake),
            ]

        case .session:
            let stays: [Int] = Array(Set([0, 30, 60, 120, model.settings.int("stayAwakeMinutes") ?? 0])).sorted()
            let stayOptions: [(tag: String, label: String)] = stays.map { minutes in
                (tag: String(minutes), label: minutes == 0 ? "Until I stop it" : durationLabel(minutes: minutes))
            }
            let stayBinding = Binding<String>(get: { String(model.settings.int("stayAwakeMinutes") ?? 0) },
                                              set: { let m = Int($0) ?? 0; model.set("stayAwakeMinutes", m == 0 ? nil : m) })
            return [
                SectionSpec(title: "Keep Awake", rows: [
                    minutes(model, "Default duration", "Used by “Default” in the menu and the panel.", key: "defaultAwakeMinutes", choices: [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 480], fallback: 30),
                    toggle(model, "Remember my last custom duration", key: "rememberLastCustomDuration"),
                ]),
                SectionSpec(title: "Power Timer", rows: [
                    action(model, "Default action", key: "defaultAction"),
                    minutes(model, "Default timer", key: "defaultTimerMinutes", choices: [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 480], fallback: 30),
                ]),
                SectionSpec(title: "Countdown", rows: [
                    minutes(model, "Snooze length", key: "snoozeMinutes", choices: [5, 10, 15, 20, 30, 45, 60], fallback: 15),
                    RowSpec(title: "Stay Awake keeps going for", detail: "What the Stay Awake button in the countdown does.",
                            control: .choice(stayOptions, stayBinding, enabled: true)),
                ]),
            ]

        case .playback:
            let monitoring = session.bool("whileAudio") || session.bool("playbackEnabled")
            return [
                SectionSpec(title: "Sleep after playback", footer: audio ? nil : "Audio monitoring is unavailable in this build.", rows: [
                    command(model, "Sleep after playback stops", "Turn on before you start watching; it turns off after it runs.",
                            on: session.bool("playbackEnabled"), command: "playback-toggle", enabled: audio),
                    seconds(model, "Wait for inactivity", "No keyboard or mouse input for this long.", key: "idleSeconds",
                            choices: [60, 120, 300, 600, 900, 1200, 1800, 3600], fallback: 600, enabled: audio),
                    action(model, "Then", key: "playbackAction", enabled: audio),
                ]),
                SectionSpec(title: "Keep awake while audio plays", rows: [
                    command(model, "Keep awake while audio plays", "Holds your Mac awake while sound is playing.",
                            on: session.bool("whileAudio"), command: "audio-toggle", enabled: audio),
                ]),
                SectionSpec(title: "Output level", rows: [
                    RowSpec(title: "Live output level",
                            detail: "Read from the system output. Doze never records audio." + (monitoring ? "" : " Shown while one of the switches above is on."),
                            control: .view(AnyView(OutputLevel(monitoring: monitoring, playing: session.bool("audioActive"))))),
                ]),
            ]

        case .notifications:
            return [
                SectionSpec(title: "Final warning", rows: [
                    seconds(model, "Warning length", "How long the countdown runs before any power action.", key: "countdownSeconds",
                            choices: [60, 120, 180, 300, 600, 900], fallback: 300),
                    toggle(model, "Play a sound when it appears", key: "warningSound"),
                    toggle(model, "Show on every display", key: "warningAllDisplays"),
                    button("Preview the warning", "Shows the warning without scheduling anything.", label: "Preview") { model.send("preview") },
                ]),
                SectionSpec(title: "Agents", rows: [
                    toggle(model, "When an agent asks to keep your Mac awake", key: "notifyAgentApproval"),
                    toggle(model, "When all agents have finished", key: "notifyAgentsFinished"),
                    toggle(model, "When an agent stops checking in", key: "notifyAgentStalled"),
                ]),
                SectionSpec(title: "Keep Awake", rows: [
                    toggle(model, "When a Keep Awake session ends", key: "notifyKeepAwakeEnded"),
                ]),
            ]

        case .agents:
            return AgentsPage.sections(model)

        case .advanced:
            let address = model.snapshot.string("mcpAddress") ?? "Unavailable"
            let installed = model.cliInstalled
            return [
                SectionSpec(title: "Command line", footer: installed ? nil : "Or run in Terminal: " + model.cliInstallCommand, rows: [
                    installed
                        ? RowSpec(title: "doze command-line tool", detail: "doze run --then sleep -- <command>", mono: true, control: .value("Installed"))
                        : button("doze command-line tool", "doze run --then sleep -- <command>", mono: true, label: "Install…") { model.installCommandLineTool() },
                ]),
                SectionSpec(title: "Local server", rows: [
                    RowSpec(title: "MCP server for agents", detail: address + " · loopback only", mono: true, control: .toggle(Binding(
                        get: { model.bool("mcpServerEnabled") }, set: { model.set("mcpServerEnabled", $0) }), enabled: true)),
                ]),
                SectionSpec(title: "Power", rows: [
                    button("How Doze keeps your Mac awake", "System power assertions, released when you quit. No simulated input.",
                           label: "Show active assertions") { model.showAssertions = true },
                ]),
                SectionSpec(title: "Diagnostics", rows: [
                    button("Local data", "Preferences and diagnostics never leave this Mac.", label: "Show local data", action: model.openData),
                    toggle(model, "Write diagnostic logs", "Errors are logged on this Mac, up to about 256 KB.", key: "logging"),
                    button("Export a diagnostics report", label: "Export…", action: model.exportDiagnostics),
                    button("Reset all settings", label: "Reset…", destructive: true) { model.confirmReset = true },
                ]),
            ]

        case .guide:
            let panel = model.string("iconClickOpens") != "menu"
            func glyph(_ state: String, _ title: String, _ detail: String) -> RowSpec {
                RowSpec(title: title, detail: detail, control: .view(AnyView(GlyphRow(state: state, title: title, detail: detail))))
            }
            return [
                SectionSpec(title: "The menu bar icon", rows: [
                    glyph("normal", "Hollow sun", "Normal sleep allowed. Doze isn't holding anything."),
                    glyph("awake", "Whole sun, with time or a number", "Keeping awake. The time left, or how many agents are working."),
                    glyph("attention", "Sun with a dot", "An agent is waiting for your approval."),
                    glyph("countdown", "Banded sun", "The final warning is counting down."),
                ]),
                SectionSpec(title: "Opening Doze", rows: [
                    RowSpec(title: "Click the icon", detail: nil, control: .value(panel ? "Panel" : "Menu")),
                    RowSpec(title: "Right-click or ⌥-click", detail: nil, control: .value(panel ? "Menu" : "Panel")),
                    RowSpec(title: "Settings", detail: nil, control: .value("⌘,")),
                ]),
            ]

        case .about:
            return [SectionSpec(title: nil, rows: [
                RowSpec(title: "Doze", detail: "Version \(model.snapshot.string("version") ?? "") · Your computer knows when it's bedtime.",
                        control: .view(AnyView(AboutView(model: model)))),
            ])]
        }
    }
}

// MARK: - The window

struct SettingsView: View {
    @ObservedObject var model: NativeUI

    var body: some View {
        NavigationSplitView {
            List(selection: $model.page) {
                identity
                if model.search.isEmpty {
                    ForEach(Page.groups, id: \.self) { group in
                        Section {
                            ForEach(group) { page in
                                Label { Text(page.rawValue) } icon: { SettingsIcon(symbol: page.symbol, color: page.color) }
                                    .tag(page)
                            }
                        }
                    }
                } else {
                    let results = model.searchResults(model.search)
                    Section("Results") {
                        if results.isEmpty { Text("No results").foregroundStyle(.secondary) }
                        ForEach(results) { entry in
                            Button { model.reveal(entry) } label: {
                                VStack(alignment: .leading, spacing: 1) {
                                    Text(entry.title)
                                    if entry.title != entry.page.rawValue {
                                        Text(entry.page.rawValue).font(.caption).foregroundStyle(.secondary)
                                    }
                                }
                            }
                            .buttonStyle(.plain)
                            .accessibilityHint("Opens \(entry.page.rawValue)")
                        }
                    }
                }
            }
            .searchable(text: $model.search, placement: .sidebar, prompt: "Search")
            // Outermost on the column, or the split view ignores it and truncates page names.
            .navigationSplitViewColumnWidth(min: 210, ideal: 230, max: 300)
        } detail: {
            ScrollViewReader { proxy in
                VStack(spacing: 0) {
                    Form {
                        ForEach(SettingsCatalog.sections(for: model.page ?? .overview, model: model)) { section in
                            Section {
                                ForEach(section.rows) { row in
                                    RowView(row: row, highlighted: model.revealed == row.title).id(row.title)
                                }
                            } header: {
                                if let title = section.title { Text(title) }
                            } footer: {
                                if let footer = section.footer { Text(footer).foregroundStyle(.secondary).textSelection(.enabled) }
                            }
                        }
                    }
                    .formStyle(.grouped)
                    .tint(.dozeAccent)
                    .id(model.page)
                    if !model.notice.isEmpty {
                        Label(model.notice, systemImage: "info.circle")
                            .font(.callout).foregroundStyle(.secondary).textSelection(.enabled).padding(12)
                    }
                }
                .onChange(of: model.revealed) { title in
                    guard let title else { return }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                        withAnimation { proxy.scrollTo(title, anchor: .center) }
                    }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                        if model.revealed == title { model.revealed = nil }
                    }
                }
            }
            .navigationTitle(model.page?.rawValue ?? "Doze")
            .sheet(item: $model.setupPrompt) { setup in PromptSheet(model: model, setup: setup) }
        }
        .sheet(item: $model.pendingChange) { change in ConnectSheet(model: model, change: change) }
        .confirmationDialog("Reset all settings to their defaults?", isPresented: $model.confirmReset) {
            Button("Reset", role: .destructive) { model.send("reset") }
        } message: {
            Text("Every page returns to its defaults, including the agents you allowed. Connected tools keep their hooks.")
        }
        .alert("Active power assertions", isPresented: $model.showAssertions) {
            Button("OK", role: .cancel) {}
        } message: {
            let held = model.snapshot.value("assertions") as? [String] ?? []
            Text(held.isEmpty ? "Doze isn't holding any power assertions right now." : held.joined(separator: "\n"))
        }
    }

    /// Like the account row in System Settings: the app and what it is doing right now.
    private var identity: some View {
        HStack(spacing: 10) {
            AppIcon(model: model, size: 34)
            VStack(alignment: .leading, spacing: 1) {
                Text("Doze").font(.headline)
                Text(model.snapshot.string("statusShort") ?? "Normal sleep allowed")
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture { model.page = .overview; model.search = "" }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }
}

/// One row: a native control with the setting's title and explanation, as in System Settings.
struct RowView: View {
    let row: RowSpec
    var highlighted = false

    var body: some View {
        content
            .padding(.vertical, 1)
            .background(highlighted ? Color.dozeTint : Color.clear, in: RoundedRectangle(cornerRadius: 6))
    }

    @ViewBuilder private var content: some View {
        switch row.control {
        case let .toggle(binding, enabled):
            Toggle(isOn: binding) { described }.toggleStyle(.switch).disabled(!enabled)
        case let .choice(options, binding, enabled):
            Picker(selection: binding) {
                ForEach(options, id: \.tag) { Text($0.label).tag($0.tag) }
            } label: { described }
            .pickerStyle(.menu)
            .disabled(!enabled)
        case let .button(label, prominent, destructive, action):
            LabeledContent {
                if prominent {
                    Button(label, action: action).buttonStyle(.borderedProminent)
                } else {
                    Button(label, role: destructive ? .destructive : nil, action: action)
                }
            } label: { described }
        case let .value(text):
            LabeledContent { Text(text).foregroundStyle(.secondary) } label: { described }
        case let .view(view):
            view
        case .none:
            described
        }
    }

    private var described: some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(row.title)
            if let detail = row.detail {
                Text(detail)
                    .font(row.mono ? .system(.subheadline, design: .monospaced) : .subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
        }
    }
}

// MARK: - Overview

@MainActor
enum OverviewPage {
    static let presets: [(Int, String)] = [(900, "15m"), (1800, "30m"), (3600, "1h"), (7200, "2h")]

    static func sections(_ model: NativeUI) -> [SectionSpec] {
        let session = model.session
        let audio = model.snapshot.bool("audioSupported")
        var result = [SectionSpec(title: nil, rows: [
            RowSpec(title: "Status", detail: model.snapshot.string("statusDetail"), control: .view(AnyView(StatusHeader(model: model)))),
        ])]
        if let countdown = session.object("countdown") {
            result.append(SectionSpec(title: "Final warning", rows: [
                RowSpec(title: "Countdown", detail: nil, control: .view(AnyView(CountdownControls(model: model, countdown: countdown)))),
            ]))
        }
        var keep: [RowSpec] = []
        if session.bool("awake") {
            keep.append(RowSpec(title: "Keeping awake", detail: nil, control: .view(AnyView(KeepingAwakeRow(model: model)))))
        } else {
            keep.append(RowSpec(title: "Keep awake for", detail: nil, control: .view(AnyView(
                PresetRow(label: "Start", spoken: "Keep awake for", presets: presets, start: { model.send("awake", ["seconds": $0]) }) {
                    ChipButton("Indefinitely") { model.send("awake-forever") }
                    MoreMenu(items: [("Custom duration…", { model.showTimer(awake: true, date: false) }),
                                     ("Until a specific time…", { model.showTimer(awake: true, date: true) })])
                }))))
        }
        keep.append(SettingsCatalog.command(model, "Keep awake while audio plays", "Holds your Mac awake while sound is playing.",
                                            on: session.bool("whileAudio"), command: "audio-toggle", enabled: audio))
        result.append(SectionSpec(title: "Keep Awake", rows: keep))

        let selected = session.string("selectedAction") ?? model.string("defaultAction") ?? "sleep"
        let actionOptions: [(tag: String, label: String)] = model.actions.map { (tag: $0, label: actionLabel($0)) }
        let actionBinding = Binding<String>(get: { selected }, set: { model.send("select-action", ["action": $0]) })
        var timer: [RowSpec] = [
            RowSpec(title: "Action", detail: nil, control: .choice(actionOptions, actionBinding, enabled: true)),
        ]
        if let running = session.object("timer") {
            timer.append(RowSpec(title: actionLabel(running.string("action")) + " scheduled", detail: nil, control: .view(AnyView(
                TimelineView(.periodic(from: .now, by: 1)) { context in
                    LabeledContent {
                        HStack {
                            Text(remainingText(model.left(running, context.date))).monospacedDigit().foregroundStyle(.secondary)
                            Button("Stop timer") { model.send("stop-timer") }
                        }
                    } label: { Text(actionLabel(running.string("action")) + " scheduled") }
                }))))
        } else {
            timer.append(RowSpec(title: "Start a timer", detail: nil, control: .view(AnyView(
                PresetRow(label: "Start", spoken: actionLabel(selected) + " in", presets: presets,
                          start: { model.send("timer", ["seconds": $0, "action": selected]) }) {
                    MoreMenu(items: [("Custom duration…", { model.showTimer(awake: false, date: false) }),
                                     ("At a specific time…", { model.showTimer(awake: false, date: true) })])
                }))))
        }
        timer.append(SettingsCatalog.command(model, "Sleep after playback stops",
                                             session.bool("playbackEnabled") ? playbackPhaseLabel(session.string("playbackPhase"))
                                                 : "Turn on before you start watching; it turns off after it runs.",
                                             on: session.bool("playbackEnabled"), command: "playback-toggle", enabled: audio))
        result.append(SectionSpec(title: "Power Timer", rows: timer))

        result.append(SectionSpec(title: "Agents", rows: [
            RowSpec(title: "Agents", detail: nil, control: .view(AnyView(AgentsSummary(model: model)))),
        ]))
        result.append(SectionSpec(title: "Quick access", rows: [
            RowSpec(title: "Quick access", detail: nil, control: .view(AnyView(HStack {
                ChipButton("Preview the countdown") { model.send("preview") }
                ChipButton("Session defaults") { model.page = .session }
                Spacer()
                ChipButton("Show local data", action: model.openData)
            }))),
        ]))
        return result
    }
}

struct StatusHeader: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            HStack(spacing: 14) {
                StatusTile(state: model.snapshot.string("iconState") ?? "normal", size: 44)
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.snapshot.string("statusShort") ?? "Normal sleep allowed").font(.system(size: 15, weight: .semibold))
                    Text(PanelText.detail(model, context.date)).foregroundStyle(.secondary)
                }
            }
            .padding(.vertical, 4)
            .accessibilityElement(children: .combine)
        }
    }
}

struct KeepingAwakeRow: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            LabeledContent {
                HStack {
                    if model.awakeLeft(context.date) != nil {
                        Button("Extend 15 minutes") { model.send("extend") }
                    }
                    Button("Stop") { model.send("stop-awake") }.accessibilityLabel("Stop keeping awake")
                }
            } label: {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Keeping awake")
                    Text(model.awakeLeft(context.date).map { remainingText($0) + " left" } ?? "Until you stop it")
                        .font(.subheadline).foregroundStyle(.secondary).monospacedDigit()
                }
            }
        }
    }
}

struct CountdownControls: View {
    @ObservedObject var model: NativeUI
    let countdown: JSON
    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            VStack(alignment: .leading, spacing: 10) {
                LabeledContent(actionLabel(countdown.string("action")) + " in") {
                    Text(clockText(model.left(countdown, context.date))).font(.title2.monospacedDigit().weight(.semibold))
                }
                HStack {
                    Button("Snooze \(model.int("snoozeMinutes", 15)) min") { model.send("snooze") }
                    Button("Stay Awake") { model.send("stay-awake") }
                    Spacer()
                    Button("Cancel", role: .destructive) { model.send("cancel") }.accessibilityLabel("Cancel the action")
                }
            }
        }
    }
}

struct AgentsSummary: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        let working = model.snapshot.int("agents.working") ?? 0
        let pending = model.snapshot.int("agents.pending") ?? 0
        let connected = model.links.filter(\.connected).map(\.name)
        HStack(spacing: 10) {
            SettingsIcon(symbol: "person.2.fill", color: Page.agents.color)
            VStack(alignment: .leading, spacing: 2) {
                Text(pending > 0 ? "\(pending) waiting for your approval" : working > 0 ? "\(working) working" : "No agents running")
                Text(connected.isEmpty ? "Connect Claude Code, Codex, OpenCode or Gemini CLI." : "\(listText(connected)) \(connected.count == 1 ? "is" : "are") connected.")
                    .font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
            Button("Agent settings") { model.page = .agents }
        }
    }
}

/// Preset durations as accent-tinted buttons, as in the design's chips.
struct PresetRow<Trailing: View>: View {
    let label: String
    let spoken: String
    let presets: [(Int, String)]
    let start: (Int) -> Void
    @ViewBuilder let trailing: () -> Trailing
    var body: some View {
        HStack(spacing: 6) {
            Text(label).foregroundStyle(.secondary).padding(.trailing, 4)
            ForEach(presets, id: \.0) { preset in
                ChipButton(preset.1) { start(preset.0) }.accessibilityLabel(spoken + " " + preset.1)
            }
            Spacer(minLength: 6)
            trailing()
        }
    }
}

struct ChipButton: View {
    let title: String
    let action: () -> Void
    init(_ title: String, action: @escaping () -> Void) { self.title = title; self.action = action }
    var body: some View {
        Button(action: action) {
            Text(title).font(.system(size: 12, weight: .medium)).padding(.horizontal, 8).frame(height: 22)
                .foregroundStyle(Color.dozeAccent)
                .background(Color.dozeTint, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(.isButton)
    }
}

struct MoreMenu: View {
    let items: [(String, () -> Void)]
    var title = "More"
    var body: some View {
        Menu {
            ForEach(items.indices, id: \.self) { index in Button(items[index].0, action: items[index].1) }
        } label: {
            Text(title).font(.system(size: 12, weight: .medium)).foregroundStyle(Color.dozeAccent)
        }
        .menuStyle(.borderlessButton)
        .fixedSize()
        .accessibilityLabel("More options")
    }
}

/// Doze reads whether sound reaches the output, never the sound itself.
struct OutputLevel: View {
    let monitoring: Bool
    let playing: Bool
    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text("Live output level")
                Text("Read from the system output. Doze never records audio.").font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
            Text(!monitoring ? "Off" : playing ? "Sound playing" : "Silent").foregroundStyle(.secondary)
            HStack(alignment: .bottom, spacing: 3) {
                ForEach(Array([0.4, 0.8, 0.55, 0.95, 0.35, 0.7, 0.25, 0.5].enumerated()), id: \.offset) { item in
                    RoundedRectangle(cornerRadius: 2).fill(Color.dozeAccent)
                        .frame(width: 4, height: 22 * (playing ? item.element : 0.12))
                }
            }
            .frame(height: 22, alignment: .bottom)
            .opacity(monitoring ? 1 : 0.35)
            .accessibilityHidden(true)
        }
        .accessibilityElement(children: .combine)
    }
}

struct GlyphRow: View {
    let state: String
    let title: String
    let detail: String
    var body: some View {
        HStack(spacing: 12) {
            DozeGlyph(state: state, size: 18)
                .foregroundStyle(.primary)
                .frame(width: 26, height: 26)
                .background(.quaternary, in: RoundedRectangle(cornerRadius: 6, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                Text(detail).font(.subheadline).foregroundStyle(.secondary)
            }
        }
        .accessibilityElement(children: .combine)
    }
}

struct AboutView: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        VStack(spacing: 10) {
            AppIcon(model: model, size: 96)
            Text("Doze").font(.system(size: 24, weight: .bold))
            Text("Version \(model.snapshot.string("version") ?? "")").foregroundStyle(.secondary).textSelection(.enabled)
            Text("Your computer knows when it's bedtime.")
            HStack(spacing: 6) {
                ForEach(model.snapshot.objects("links").indices, id: \.self) { index in
                    let link = model.snapshot.objects("links")[index]
                    ChipButton(link.string("title") ?? "") { model.openLink(link.string("url") ?? "") }
                        .accessibilityHint("Opens in your browser")
                }
            }
            .padding(.top, 10)
            Text("No account · No cloud · No telemetry").font(.subheadline).foregroundStyle(.secondary).padding(.top, 14)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 24)
    }
}

// MARK: - Agents

@MainActor
enum AgentsPage {
    static func sections(_ model: NativeUI) -> [SectionSpec] {
        var connected: [RowSpec] = model.links.map { link in
            RowSpec(title: link.name, detail: link.method, control: .view(AnyView(AgentLinkRow(model: model, link: link))))
        }
        let tools = model.setting("agents.processTools") as? [JSON] ?? []
        let cursor = tools.first { $0.string("id") == "cursor" }
        connected.append(RowSpec(title: "Cursor", detail: "Process detection · no lifecycle events", control: .view(AnyView(
            HStack(spacing: 10) {
                Monogram("Cu", size: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Cursor")
                    Text("Process detection · no lifecycle events").font(.subheadline).foregroundStyle(.secondary)
                }
                Spacer()
                Toggle("Detect Cursor", isOn: Binding(get: { cursor?.bool("detect") ?? false },
                                                      set: { model.set("agents.processTools", processTools(tools, detect: $0)) }))
                    .toggleStyle(.switch).labelsHidden()
            }))))
        if cursor?.bool("detect") == true {
            connected.append(RowSpec(title: "Keep your Mac awake while Cursor is open", detail: "Cursor has no lifecycle events, so Doze can't tell when it's working.",
                                     control: .toggle(Binding(get: { cursor?.bool("keepAwake") ?? false },
                                                              set: { model.set("agents.processTools", processTools(tools, keepAwake: $0)) }), enabled: true)))
        }
        let address = model.snapshot.string("mcpAddress") ?? "Unavailable"
        connected.append(RowSpec(title: "Other MCP clients", detail: address + " · loopback only", control: .view(AnyView(
            HStack(spacing: 10) {
                Monogram("+", size: 28)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Other MCP clients")
                    Text(address + " · loopback only").font(.system(.subheadline, design: .monospaced)).foregroundStyle(.secondary)
                }
                Spacer()
                Button("Copy config") { model.send("copy-config") }.accessibilityLabel("Copy MCP config")
            }))))

        var result = [
            SectionSpec(title: "Agent requests", rows: [
                SettingsCatalog.toggle(model, "Let agents keep your Mac awake", "Through hooks and the local MCP server.", key: "agents.enabled"),
                SettingsCatalog.toggle(model, "Ask before a new agent holds a lease", key: "agents.askBeforeNew"),
                SettingsCatalog.action(model, "When agents finish", "After the final warning.", key: "agents.defaultCompletion", allowNothing: true),
                RowSpec(title: "If an agent stops checking in", detail: "Doze stays awake, then lets go without acting.", control: .value("Up to 30 minutes")),
            ]),
            SectionSpec(title: "Connected agents",
                        footer: "Connect adds Doze's hooks to the tool's own settings after showing you the change, keeps a timestamped backup beside the file and never touches other entries. Remove takes out exactly what Connect added. Use a Prompt… gives you text to paste into the agent instead, so it makes the same change itself.",
                        rows: connected),
        ]
        let trusted = model.setting("agents.trusted") as? [String] ?? []
        let clients = (model.setting("agents.clients") as? [JSON] ?? []).filter { $0.bool("keepAwake") }
        if !trusted.isEmpty || !clients.isEmpty {
            result.append(SectionSpec(title: "Allowed agents", rows:
                trusted.map { id in
                    SettingsCatalog.button(agentName(id), "Keeps your Mac awake without asking.", label: "Forget") {
                        model.set("agents.trusted", trusted.filter { $0 != id })
                    }
                } + clients.map { client in
                    SettingsCatalog.button(client.string("name") ?? "MCP client", "MCP client · keeps your Mac awake without asking.", label: "Forget") {
                        model.send("agent-permission", ["id": client.string("id") ?? ""])
                    }
                }))
        }
        return result
    }

    static func processTools(_ tools: [JSON], detect: Bool? = nil, keepAwake: Bool? = nil) -> [JSON] {
        var copy = tools
        if !copy.contains(where: { $0.string("id") == "cursor" }) {
            copy.append(["id": "cursor", "detect": false, "keepAwake": false])
        }
        return copy.map { tool in
            guard tool.string("id") == "cursor" else { return tool }
            var next = tool
            if let detect { next["detect"] = detect }
            if let keepAwake { next["keepAwake"] = keepAwake }
            return next
        }
    }
}

struct AgentLinkRow: View {
    @ObservedObject var model: NativeUI
    let link: AgentLink
    var body: some View {
        HStack(spacing: 10) {
            Monogram(link.monogram, size: 28)
            VStack(alignment: .leading, spacing: 2) {
                Text(link.name)
                Text(link.method).font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
            if link.connected {
                Text("Connected").foregroundStyle(.green)
                Button("Remove") { model.send("connect-preview", ["agent": link.id, "remove": true]) }
                    .accessibilityLabel("Remove \(link.name)")
            } else {
                Text(link.installed ? "Not set up" : "Not installed").foregroundStyle(.secondary)
                Button("Use a Prompt…") { model.send("connect-prompt", ["agent": link.id]) }
                    .help("Copy a prompt that has \(link.name) add Doze's hooks itself")
                    .accessibilityLabel("Connect \(link.name) with a prompt")
                Button("Connect") { model.send("connect-preview", ["agent": link.id, "remove": false]) }
                    .buttonStyle(.borderedProminent)
                    .accessibilityLabel("Connect \(link.name)")
            }
        }
        .accessibilityElement(children: .contain)
    }
}

/// Shows the exact change to a tool's config before Doze writes it.
struct ConnectSheet: View {
    @ObservedObject var model: NativeUI
    let change: PendingChange
    var body: some View {
        let agent = agentName(change.agent)
        VStack(alignment: .leading, spacing: 14) {
            Text(change.remove ? "Remove Doze from \(agent)?" : "Connect \(agent)?").font(.title2.bold())
            Text((change.remove ? "Doze will remove its hooks from \(agent)'s settings. " : "Doze will add these lines to \(agent)'s settings. ")
                 + "A timestamped backup is kept beside the file, and nothing else in it changes.")
                .fixedSize(horizontal: false, vertical: true)
            Text(change.path).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
            ScrollView([.horizontal, .vertical]) {
                Text(change.diff).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
                    .padding(12).frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(height: 220)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
            if let note = change.note { Text(note).foregroundStyle(.secondary) }
            HStack {
                Spacer()
                Button("Cancel") { model.pendingChange = nil }.keyboardShortcut(.cancelAction)
                Button(change.remove ? "Remove" : "Connect") {
                    model.send("connect-apply", ["agent": change.agent, "remove": change.remove, "token": change.token])
                    model.pendingChange = nil
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 560)
    }
}

/// The prompt that has an agent make Connect's change itself, to copy into the agent.
struct PromptSheet: View {
    @ObservedObject var model: NativeUI
    let setup: SetupPrompt
    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Connect \(setup.name) with a prompt").font(.title2.bold())
            Text("Paste this into \(setup.name). It makes the same change Connect would, backs up the file first and leaves everything else as it is. \(setup.name) shows as Connected here once it's done.")
                .fixedSize(horizontal: false, vertical: true)
            Text(setup.path).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
            ScrollView {
                Text(setup.prompt).font(.system(.callout, design: .monospaced)).textSelection(.enabled)
                    .padding(12).frame(maxWidth: .infinity, alignment: .leading)
            }
            .frame(height: 260)
            .background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
            HStack {
                Spacer()
                Button("Close") { model.setupPrompt = nil }.keyboardShortcut(.cancelAction)
                Button("Copy Prompt") {
                    model.copySetupPrompt(setup)
                    model.setupPrompt = nil
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 560)
    }
}

// MARK: - Shared pieces

/// The rounded-square icons System Settings uses for its sidebar and rows.
struct SettingsIcon: View {
    let symbol: String
    let color: Color
    var size: CGFloat = 20
    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: size * 0.55, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(
                LinearGradient(colors: [color.opacity(0.85), color], startPoint: .top, endPoint: .bottom),
                in: RoundedRectangle(cornerRadius: size * 0.26, style: .continuous)
            )
            .accessibilityHidden(true)
    }
}

/// The status square: purple while sleep is allowed, orange while keeping awake.
struct StatusTile: View {
    let state: String
    var size: CGFloat = 36
    var body: some View {
        DozeGlyph(state: state, size: size * 0.5)
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(state == "normal" ? Color.statusIdle : Color.statusAwake,
                        in: RoundedRectangle(cornerRadius: size * 0.25, style: .continuous))
            .accessibilityHidden(true)
    }
}

/// An agent's two-letter tile.
struct Monogram: View {
    let letters: String
    var size: CGFloat = 28
    init(_ letters: String, size: CGFloat = 28) { self.letters = letters; self.size = size }
    var body: some View {
        Text(letters)
            .font(.system(size: 11, weight: .bold))
            .foregroundStyle(Color.dozeTileText)
            .frame(width: size, height: size)
            .background(Color.dozeTile, in: RoundedRectangle(cornerRadius: size / 4, style: .continuous))
            .accessibilityHidden(true)
    }
}

/// Doze's own app icon, with the brand glyph as a fallback before the engine reports it.
struct AppIcon: View {
    @ObservedObject var model: NativeUI
    let size: CGFloat
    var body: some View {
        Group {
            if let image = model.appIcon {
                Image(nsImage: image).resizable().interpolation(.high)
            } else {
                StatusTile(state: "countdown", size: size)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}
