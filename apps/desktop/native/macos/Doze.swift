import AppKit
import SwiftUI

struct Preferences: Codable, Equatable {
    var theme = "system"
    var launchAtStartup = false
    var startMinimized = true
    var notifications = true
    var defaultAction = "sleep"
    var silenceSeconds = 60
    var idleSeconds = 300
    var countdownSeconds = 300
    var playbackAction = "sleep"
    var logging = false
    var allowDisplaySleep = false
    var defaultAwakeMinutes = 30
    var defaultTimerMinutes = 30
}

struct AgentClient: Codable, Identifiable {
    var id: String; var name: String; var keepAwake: Bool; var actions: [String]
}
struct AgentSettings: Codable {
    var enabled: Bool; var leaseSeconds: Int; var defaultCompletion: String?; var clients: [AgentClient]
}
struct AgentSession: Codable, Identifiable {
    var session_id: String; var client_name: String; var reason: String; var status: String
    var completion_action: String?; var last_heartbeat: Int
    var id: String { session_id }
}
struct AgentConnection: Codable, Identifiable {
    var clientId: String; var name: String; var generic: String; var codex: String; var claude: String
    var id: String { clientId }
}
struct AgentSkill: Codable, Identifiable {
    var name: String; var status: String; var path: String?; var error: String?
    var id: String { name }
}
struct EngineSnapshot: Codable {
    var settings: Preferences
    var settingsPath: String
    var actions: [String]
    var audioSupported: Bool
    var startupSupported: Bool
    var status: String
    var timerStatus: String
    var version: String
    var agentSessions: [AgentSession]?
    var agentConnections: [AgentConnection]?
    var agentNow: Int?
    var agentSkills: [AgentSkill]?
    var agentSkillMessage: String?
}

enum Page: String, CaseIterable, Identifiable {
    case overview = "Overview"
    case general = "General"
    case session = "Session defaults"
    case playback = "After playback"
    case notifications = "Notifications"
    case agents = "Agents"
    case advanced = "Advanced"
    case help = "Menu guide"
    case about = "About Doze"

    var id: String { rawValue }
    var symbol: String {
        switch self {
        case .overview: return "house"
        case .general: return "gearshape"
        case .session: return "moon"
        case .playback: return "speaker.wave.2"
        case .notifications: return "bell"
        case .agents: return "person.2"
        case .advanced: return "slider.horizontal.3"
        case .help: return "book"
        case .about: return "info.circle"
        }
    }
}

@MainActor
final class NativeUI: NSObject, ObservableObject, NSWindowDelegate {
    @Published var agentSettings: AgentSettings?
    @Published var snapshot: EngineSnapshot?
    @Published var draft = Preferences() {
        didSet { applyAppearance(draft.theme); applyChanges() }
    }
    @Published var page: Page? = .overview
    @Published var search = ""
    @Published var notice = ""
    @Published var saving = false
    @Published var warningAction = "Sleep"
    @Published var remaining = 60
    @Published var preview = false
    @Published var warningVisible = false
    @Published var timerIsAwake = false
    @Published var timerUsesDate = false
    @Published var durationMinutes = 30
    @Published var targetDate = Date().addingTimeInterval(1800)

    private var settingsWindow: NSWindow?
    private var warningWindow: NSWindow?
    private var timerWindow: NSWindow?
    private var previewDeadline: Date?
    private var clock: Timer?
    private var ticks = 0
    private var pendingSettings: Preferences?
    private let verification = CommandLine.arguments.contains("--verify-ui")

    var dirty: Bool { snapshot.map { draft != $0.settings } ?? false }

    private func applyAppearance(_ theme: String) {
        switch theme {
        case "light": NSApp.appearance = NSAppearance(named: .aqua)
        case "dark": NSApp.appearance = NSAppearance(named: .darkAqua)
        default: NSApp.appearance = nil
        }
    }

    func start() {
        installApplicationMenu()
        // Only inherited pipes are used. Closing the parent ends the companion.
        DispatchQueue.global(qos: .utility).async { [weak self] in
            while let line = readLine() {
                guard let data = line.data(using: .utf8),
                      let message = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
                else { continue }
                DispatchQueue.main.async { self?.receive(message) }
            }
            DispatchQueue.main.async { NSApp.terminate(nil) }
        }
        clock = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.tick() }
        }
    }

    private func installApplicationMenu() {
        let menu = NSMenu()
        let application = NSMenu()
        let appItem = NSMenuItem()
        appItem.submenu = application
        menu.addItem(appItem)
        let quit = NSMenuItem(title: "Quit Doze", action: #selector(quitDoze), keyEquivalent: "q")
        quit.target = self
        application.addItem(quit)
        let edit = NSMenu(title: "Edit")
        let editItem = NSMenuItem(title: "Edit", action: nil, keyEquivalent: "")
        editItem.submenu = edit
        menu.addItem(editItem)
        for (title, selector, key) in [("Cut", "cut:", "x"), ("Copy", "copy:", "c"),
                                       ("Paste", "paste:", "v"), ("Select All", "selectAll:", "a")] {
            edit.addItem(NSMenuItem(title: title, action: NSSelectorFromString(selector), keyEquivalent: key))
        }
        let window = NSMenu(title: "Window")
        let windowItem = NSMenuItem(title: "Window", action: nil, keyEquivalent: "")
        windowItem.submenu = window
        menu.addItem(windowItem)
        window.addItem(NSMenuItem(title: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w"))
        NSApp.mainMenu = menu
        NSApp.windowsMenu = window
    }

    @objc private func quitDoze() { send("quit") }

    func verify() throws {
        snapshot = EngineSnapshot(settings: Preferences(), settingsPath: "/tmp/doze/settings.json",
                                  actions: ["sleep"], audioSupported: false, startupSupported: false,
                                  status: "Normal sleep allowed", timerStatus: "No power action scheduled", version: "0.1.0")
        draft = snapshot!.settings
        for theme in ["light", "dark", "system"] {
            applyAppearance(theme)
            let expected: NSAppearance.Name? = theme == "light" ? .aqua : theme == "dark" ? .darkAqua : nil
            guard NSApp.appearance?.name == expected else { throw verificationError("Appearance override was not applied.") }
        }
        for appearance in [NSAppearance.Name.aqua, .darkAqua] {
            for selected in Page.allCases {
                page = selected
                let host = NSHostingView(rootView: SettingsView(model: self))
                host.appearance = NSAppearance(named: appearance)
                host.frame = NSRect(x: 0, y: 0, width: 940, height: 720)
                host.layoutSubtreeIfNeeded()
                guard host.fittingSize.width > 0 else {
                    throw NSError(domain: "Doze.NativeUI", code: 1,
                                  userInfo: [NSLocalizedDescriptionKey: "Unable to lay out \(selected.rawValue)."])
                }
            }
        }
        draft.defaultAwakeMinutes = 42
        page = .about
        page = .session
        guard draft.defaultAwakeMinutes == 42 else { throw verificationError("Navigation lost edits.") }
        var first = Preferences()
        first.notifications = false
        draft = first
        pendingSettings = first
        saving = true
        draft.defaultAwakeMinutes = 42
        var acknowledgement = snapshot!
        acknowledgement.settings = first
        let payload = try JSONSerialization.jsonObject(with: JSONEncoder().encode(acknowledgement))
        receive(["type": "saved", "snapshot": payload])
        guard draft.defaultAwakeMinutes == 42, !draft.notifications, !saving else {
            throw verificationError("Acknowledgement lost newer edits.")
        }
        pendingSettings = draft
        saving = true
        receive(["type": "error", "command": "save", "error": "Rejected test change."])
        guard draft == snapshot?.settings, !saving else {
            throw verificationError("Rejected changes were not restored.")
        }
        reset()
        guard draft.defaultAwakeMinutes == 30 else { throw verificationError("Reset did not restore defaults.") }
        // Preview buttons must never submit engine commands.
        receive(["type": "preview", "action": "Sleep"])
        guard warningWindow?.level == .floating else {
            throw verificationError("Preview is not always on top.")
        }
        snoozeWarning()
        guard !warningVisible else { throw verificationError("Preview did not dismiss.") }
    }

    /// Draws every page and window offscreen to PNG files for visual review. Like --verify-ui,
    /// it uses sample data and never contacts the engine or saves preferences.
    func render(to folder: URL) throws {
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        snapshot = EngineSnapshot(settings: Preferences(), settingsPath: "~/Library/Application Support/app.getdoze.desktop/settings.json",
                                  actions: ["sleep", "shutdown", "lock", "displayOff"], audioSupported: true, startupSupported: true,
                                  status: "Keeping awake · 42m left", timerStatus: "Sleep in 1h 5m", version: "0.1.0")
        draft = snapshot!.settings
        func capture(_ view: some View, size: NSSize, name: String, appearance: NSAppearance.Name) throws {
            let window = NSWindow(contentRect: NSRect(origin: NSPoint(x: -20000, y: -20000), size: size),
                                  styleMask: [.titled, .fullSizeContentView], backing: .buffered, defer: false)
            window.appearance = NSAppearance(named: appearance)
            window.titlebarAppearsTransparent = true
            let host = NSHostingView(rootView: view)
            window.contentView = host
            window.orderFrontRegardless()
            for _ in 0..<3 { RunLoop.main.run(until: Date().addingTimeInterval(0.15)) }
            host.layoutSubtreeIfNeeded()
            // Render the whole frame view's layer tree; cacheDisplay omits layer-hosted SwiftUI text.
            guard let root = window.contentView?.superview, let layer = root.layer else {
                throw verificationError("Could not render \(name).")
            }
            let scale = window.backingScaleFactor
            guard let context = CGContext(data: nil, width: Int(root.bounds.width * scale), height: Int(root.bounds.height * scale),
                                          bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
                throw verificationError("Could not render \(name).")
            }
            context.scaleBy(x: scale, y: scale)
            NSAppearance(named: appearance)?.performAsCurrentDrawingAppearance {
                context.setFillColor(NSColor.windowBackgroundColor.cgColor)
            }
            context.fill(root.bounds)
            layer.render(in: context)
            window.orderOut(nil)
            guard let rendered = context.makeImage(),
                  let png = NSBitmapImageRep(cgImage: rendered).representation(using: .png, properties: [:]) else {
                throw verificationError("Could not encode \(name).")
            }
            let theme = appearance == .aqua ? "light" : "dark"
            try png.write(to: folder.appendingPathComponent("\(name)-\(theme).png"))
        }
        for appearance in [NSAppearance.Name.aqua, .darkAqua] {
            for selected in Page.allCases {
                page = selected
                try capture(SettingsView(model: self), size: NSSize(width: 940, height: 720),
                            name: "settings-\(selected.rawValue.lowercased().replacingOccurrences(of: " ", with: "-"))", appearance: appearance)
            }
            for (isPreview, name) in [(true, "countdown-preview"), (false, "countdown")] {
                preview = isPreview
                warningAction = "Sleep"
                remaining = 287
                try capture(WarningView(model: self), size: NSSize(width: 520, height: 440), name: name, appearance: appearance)
            }
            for (awake, date) in [(true, false), (false, true)] {
                timerIsAwake = awake
                timerUsesDate = date
                try capture(TimerView(model: self), size: NSSize(width: 480, height: 280),
                            name: "timer-\(awake ? "awake" : "power")-\(date ? "time" : "duration")", appearance: appearance)
            }
        }
    }

    private func verificationError(_ message: String) -> NSError {
        NSError(domain: "Doze.NativeUI", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private func tick() {
        if let deadline = previewDeadline, preview {
            remaining = max(0, Int(ceil(deadline.timeIntervalSinceNow)))
            if remaining == 0 { previewDeadline = nil }
        }
        ticks += 1
        if ticks % 5 == 0, settingsWindow?.isVisible == true, !saving { send("refresh") }
    }

    func send(_ command: String, extra: [String: Any] = [:]) {
        var message = extra
        message["command"] = command
        guard let data = try? JSONSerialization.data(withJSONObject: message),
              var line = String(data: data, encoding: .utf8) else { return }
        line += "\n"
        FileHandle.standardOutput.write(Data(line.utf8))
    }

    private func receive(_ message: [String: Any]) {
        if let rawSnapshot = message["snapshot"] as? [String: Any],
           let settings = rawSnapshot["settings"] as? [String: Any], let agents = settings["agents"],
           let data = try? JSONSerialization.data(withJSONObject: agents) {
            agentSettings = try? JSONDecoder().decode(AgentSettings.self, from: data)
        }
        if let theme = message["theme"] as? String { applyAppearance(theme) }
        let type = message["type"] as? String ?? ""
        if let raw = message["snapshot"],
           let data = try? JSONSerialization.data(withJSONObject: raw),
           let incoming = try? JSONDecoder().decode(EngineSnapshot.self, from: data) {
            let replaceDraft = snapshot == nil || (!dirty && !saving)
                || (type == "saved" && draft == pendingSettings)
            snapshot = incoming
            if replaceDraft { draft = incoming.settings }
        }
        switch type {
        case "open":
            switch message["view"] as? String {
            case "about": page = .about; showSettings()
            case "help": page = .help; showSettings()
            case "agents": page = .agents; showSettings()
            case "awakeDuration": showTimer(awake: true, date: false)
            case "awakeTime": showTimer(awake: true, date: true)
            case "timerDuration": showTimer(awake: false, date: false)
            case "timerTime": showTimer(awake: false, date: true)
            default: showSettings()
            }
        case "saved":
            pendingSettings = nil
            saving = false
            applyChanges()
        case "error":
            if message["command"] as? String == "save" {
                if draft == pendingSettings, let saved = snapshot?.settings { draft = saved }
                pendingSettings = nil
                saving = false
                applyChanges()
            }
            notice = message["error"] as? String ?? "Unable to complete the request."
        case "countdown":
            if let countdown = message["countdown"] as? [String: Any] {
                preview = false
                previewDeadline = nil
                warningAction = countdown["action"] as? String ?? "Sleep"
                remaining = countdown["remaining"] as? Int ?? 0
                showWarning()
            } else if !preview { dismissWarning() }
        case "preview":
            // An actual countdown always takes priority over a preview.
            if !warningVisible || preview {
                preview = true
                warningAction = message["action"] as? String ?? "Sleep"
                remaining = 60
                previewDeadline = Date().addingTimeInterval(60)
                showWarning()
            }
        default: break
        }
    }

    private func window<Content: View>(_ title: String, size: NSSize, content: Content) -> NSWindow {
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size),
                              styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                              backing: .buffered, defer: false)
        window.title = title
        window.titlebarAppearsTransparent = true
        window.toolbarStyle = .unified
        window.isReleasedWhenClosed = false
        window.contentView = NSHostingView(rootView: content)
        window.center()
        return window
    }

    private func activate(_ window: NSWindow) {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    private func showSettings() {
        if settingsWindow == nil {
            settingsWindow = window("Doze Settings", size: NSSize(width: 940, height: 720), content: SettingsView(model: self))
            settingsWindow?.minSize = NSSize(width: 760, height: 540)
        }
        if let window = settingsWindow { activate(window) }
    }

    private func showWarning() {
        if warningWindow == nil {
            warningWindow = window("Doze · Power countdown", size: NSSize(width: 520, height: 440), content: WarningView(model: self))
            warningWindow?.styleMask.remove([.resizable, .miniaturizable])
            warningWindow?.level = .floating
            warningWindow?.delegate = self
        }
        if !warningVisible, !verification, let window = warningWindow { activate(window) }
        warningVisible = true
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if sender === warningWindow { cancelWarning(); return false }
        return true
    }

    private func dismissWarning() {
        warningWindow?.orderOut(nil)
        warningVisible = false
        previewDeadline = nil
        preview = false
    }

    func cancelWarning() {
        if !preview { send("cancel") }
        dismissWarning()
    }

    func snoozeWarning() {
        if !preview { send("snooze") }
        dismissWarning()
    }

    private func showTimer(awake: Bool, date: Bool) {
        timerIsAwake = awake
        timerUsesDate = date
        durationMinutes = awake ? draft.defaultAwakeMinutes : draft.defaultTimerMinutes
        targetDate = Date().addingTimeInterval(Double(durationMinutes * 60))
        notice = ""
        if timerWindow == nil {
            timerWindow = window("Doze · Custom session", size: NSSize(width: 480, height: 280), content: TimerView(model: self))
            timerWindow?.styleMask.remove([.resizable, .miniaturizable])
            timerWindow?.isOpaque = false
            timerWindow?.backgroundColor = .clear
        }
        if let window = timerWindow { activate(window) }
    }

    func startTimer() {
        guard timerUsesDate || (1...10080).contains(durationMinutes) else {
            notice = "Choose a duration between 1 minute and 7 days."; return
        }
        let seconds = timerUsesDate ? Int(ceil(targetDate.timeIntervalSinceNow)) : durationMinutes * 60
        guard (60...604800).contains(seconds) else { notice = "Choose a duration between 1 minute and 7 days."; return }
        send(timerIsAwake ? "awake" : "timer", extra: ["seconds": seconds])
        timerWindow?.orderOut(nil)
    }

    func closeTimer() { timerWindow?.orderOut(nil) }

    func applyChanges() {
        guard !verification, !saving, dirty else { return }
        guard let data = try? JSONEncoder().encode(draft),
              let settings = try? JSONSerialization.jsonObject(with: data) else { return }
        pendingSettings = draft
        saving = true
        notice = ""
        send("save", extra: ["settings": settings])
    }

    func reset() { notice = ""; draft = Preferences() }

    func openData() {
        guard let path = snapshot?.settingsPath else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }
}

private extension View {
    @ViewBuilder func navigationGlass() -> some View {
        if #available(macOS 26.0, *) {
            self.glassEffect(.regular, in: .rect(cornerRadius: 16))
        } else {
            self.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
        }
    }
}

struct SettingsView: View {
    @ObservedObject var model: NativeUI
    @State private var selectedAgent: String?
    @State private var showAgentPermissions = false
    @State private var copiedAgentConfig = false
    @State private var confirmAgentRevocation = false
    @State private var confirmSkillUpdate = false

    var body: some View {
        NavigationSplitView {
            List(selection: $model.page) {
                ForEach(Page.allCases.filter { model.search.isEmpty || $0.rawValue.localizedCaseInsensitiveContains(model.search) }) { page in
                    Label(page.rawValue, systemImage: page.symbol).tag(page)
                }
            }
            .navigationSplitViewColumnWidth(min: 190, ideal: 220)
            .searchable(text: $model.search, placement: .sidebar, prompt: "Find a setting")
        } detail: {
            VStack(spacing: 0) {
                Form {
                    pageContent
                }
                .formStyle(.grouped)
                // New native form identity also resets scroll when changing pages.
                .id(model.page)
                if !model.notice.isEmpty {
                    Text(model.notice).font(.callout).textSelection(.enabled).padding(12)
                }
            }
            .navigationTitle(model.page?.rawValue ?? "Doze")
        }
        .sheet(isPresented: Binding(get: { selectedAgent != nil }, set: { if !$0 { selectedAgent = nil } })) {
            agentSheet
        }
    }

    @ViewBuilder private var pageContent: some View {
        switch model.page ?? .overview {
        case .overview:
            Section("Current session") {
                explanation("Keep Awake", model.snapshot?.status ?? "Normal sleep allowed", "moon")
                explanation("Power action", model.snapshot?.timerStatus ?? "No power action scheduled", "timer")
            }
            Section("Quick access") {
                Button("Preview the countdown") { model.send("preview") }
                Button("Session defaults") { model.page = .session }
                Button("Show local data", action: model.openData)
            }
            Section("How Doze works") {
                explanation("Everything starts in the menu bar", "Keep Awake, Power Timer, After Playback and Quick Settings live in Doze's moon menu. Disabled commands explain why they aren't available.", "menubar.rectangle")
            }
        case .general:
            Section("Launch behavior") {
                Toggle("Launch at sign-in", isOn: $model.draft.launchAtStartup).disabled(model.snapshot?.startupSupported != true)
                if model.snapshot?.startupSupported != true { Text("Launch-at-login integration is not available in this build.").foregroundStyle(.secondary) }
                Toggle("Start in the menu bar", isOn: $model.draft.startMinimized)
            }
            Section("Appearance") {
                Picker("Theme", selection: $model.draft.theme) {
                    Text("System").tag("system")
                    Text("Light").tag("light")
                    Text("Dark").tag("dark")
                }
                Text("System follows your Mac’s appearance automatically. Changes apply immediately to Doze windows.").foregroundStyle(.secondary)
                explanation("Made for macOS", "Native controls, sidebar and Liquid Glass follow the system appearance on recent macOS releases. Older systems use native vibrancy. macOS controls contrast and reduced transparency.", "macwindow")
            }
        case .session:
            Section("Keep Awake") {
                Toggle("Allow the display to sleep", isOn: $model.draft.allowDisplaySleep)
                Text("The system stays awake; macOS can turn off the display.").foregroundStyle(.secondary)
                number("Default awake duration (minutes)", $model.draft.defaultAwakeMinutes, 1...10080)
            }
            Section("Power Timer") {
                number("Default timer duration (minutes)", $model.draft.defaultTimerMinutes, 1...10080)
                actionPicker("Default action", $model.draft.defaultAction)
                Text("These defaults apply to new sessions. A timer holds the computer awake until its final warning.").foregroundStyle(.secondary)
            }
        case .playback:
            Section("After Playback") {
                if model.snapshot?.audioSupported != true {
                    explanation("Audio monitoring unavailable", "After Playback and keeping awake while audio plays are unavailable on this platform build. Timed and indefinite sessions remain available.", "speaker.slash")
                }
                actionPicker("When playback ends", $model.draft.playbackAction)
                number("Continuous silence (seconds)", $model.draft.silenceSeconds, 10...3600)
                number("User inactivity (seconds)", $model.draft.idleSeconds, 30...7200)
            }.disabled(model.snapshot?.audioSupported != true)
            Section("Safety") { Text("Playback must first be observed. Both silence and user inactivity must continue before the final countdown. Resumed playback or input cancels that countdown.") }
        case .notifications:
            Section("Final warning") {
                Toggle("Show system countdown notifications", isOn: $model.draft.notifications)
                number("Countdown duration (seconds)", $model.draft.countdownSeconds, 15...1800)
                Text("The native countdown window always provides Cancel and Snooze, even when notifications are disabled.").foregroundStyle(.secondary)
                Button("Preview countdown") { model.send("preview") }
            }
        case .agents:
            agentsView
        case .advanced:
            Section("Local data") {
                Toggle("Enable diagnostic logging", isOn: $model.draft.logging)
                Text(model.snapshot?.settingsPath ?? "Loading…").font(.callout).textSelection(.enabled)
                Button("Show preferences in Finder", action: model.openData)
            }
            Section("Session safety") { Text("Rust validates settings and owns all power actions. Closing Settings keeps Doze running. Transient sessions are never restored from disk.") }
            Section("Reset preferences") {
                Text("Restore default preferences immediately. Existing timers keep their deadlines.").foregroundStyle(.secondary)
                Button("Reset defaults", action: model.reset)
            }
        case .help:
            Section("Menu guide") {
                explanation("Normal sleep allowed", "No Doze session is holding your Mac awake. Your macOS sleep settings apply normally. Select Keep Awake to start a session.", "moon")
                explanation("Keep Awake", "Choose a duration, an end time, or indefinitely. Stop releases Doze's power assertion; Extend adds 15 minutes to a timed session.", "sun.max")
                explanation("Power Timer", "Choose an action and duration. A native final warning lets you cancel or snooze before Rust performs the action.", "timer")
                explanation("Quick Settings", "These checkmarks represent saved defaults. Duration defaults apply to new sessions. Other changes, such as display sleep, take effect immediately.", "slider.horizontal.3")
                explanation("Disabled commands", "No session to stop or extend, no timer to stop, and no countdown to cancel or snooze are informational states. Unavailable platform actions remain disabled.", "info.circle")
            }
        case .about:
            Section {
                VStack(alignment: .leading, spacing: 8) {
                    Image(systemName: "moon.zzz.fill").font(.system(size: 40)).foregroundStyle(.tint)
                    Text("Doze").font(.largeTitle.bold())
                    Text("Version \(model.snapshot?.version ?? "0.1.0")").foregroundStyle(.secondary)
                    Text("Your computer knows when it's bedtime.")
                }.padding(.vertical, 12)
            }
            Section("Local and private") { explanation("No account. No cloud. No ads.", "Doze does not record audio, simulate input or send telemetry. Preferences and optional diagnostics stay on this computer.", "lock") }
            Section("Built with") {
                explanation("Rust and Tauri", "The Rust engine owns sessions, power assertions, validation and countdown safety. The menu bar uses native macOS menus.", "terminal")
                explanation("SwiftUI and AppKit", "Settings, About, custom timers and countdown windows use Apple's native controls and system materials. Open-source components retain their respective licenses.", "swift")
                Button("Show local preferences", action: model.openData)
            }
        }
    }

    @ViewBuilder private var agentsView: some View {
        Section("Agents") {
            Toggle("Enable MCP", isOn: Binding(get: { model.agentSettings?.enabled ?? false }, set: { _ in model.send("agent-enable") }))
            Picker("Default lease (seconds)", selection: Binding(get: { model.agentSettings?.leaseSeconds ?? 300 }, set: { model.send("agent-lease", extra: ["agent_seconds": $0]) })) {
                ForEach([60, 300, 900, 1800, 3600], id: \.self) { Text("\($0)").tag($0) }
            }
            Picker("Default completion", selection: Binding(get: { model.agentSettings?.defaultCompletion ?? "normal" }, set: { model.send("agent-default", extra: ["action": $0 == "normal" ? NSNull() : $0 as Any]) })) {
                Text("Return to normal").tag("normal")
                ForEach(model.snapshot?.actions ?? [], id: \.self) { Text($0.capitalized).tag($0) }
            }
            Text("A lost connection keeps the computer awake for up to 30 minutes, then releases without a completion action. Automatic completion requires all overlapping agents to finish with the same authorized action, followed by at least five minutes to cancel.").foregroundStyle(.secondary)
        }
        Section("Agent connections") {
            ForEach(["Codex", "Claude Code", "Generic MCP client"], id: \.self) { name in
                let client = model.agentSettings?.clients.first { $0.name == name }
                HStack {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(name).bold()
                        Text(client == nil ? "Set up a local connection to Doze." : "Profile created · \(client!.keepAwake ? "Keep awake allowed" : "Ask before keeping awake")")
                            .font(.callout).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button(client == nil ? "Set up" : "Configure") {
                        showAgentPermissions = false; copiedAgentConfig = false; selectedAgent = name
                        if client == nil { model.send("agent-connect", extra: ["name": name]) }
                    }
                    if client != nil {
                        Button("Permissions") { showAgentPermissions = true; selectedAgent = name }
                    }
                }
            }
        }
        Section("Sessions") {
            if !(model.snapshot?.agentSessions ?? []).contains(where: { ["active", "connection_lost", "awaiting_authorization"].contains($0.status) }) {
                Text("No active agent sessions").foregroundStyle(.secondary)
            }
            ForEach((model.snapshot?.agentSessions ?? []).filter { ["active", "connection_lost", "awaiting_authorization"].contains($0.status) }) { session in
                VStack(alignment: .leading, spacing: 6) {
                    Text(session.client_name).bold()
                    Text(session.reason)
                    Text("\(session.status.replacingOccurrences(of: "_", with: " ")) · When finished: \(session.completion_action ?? "Return to normal")").foregroundStyle(.secondary)
                    if session.status == "awaiting_authorization" {
                        HStack {
                            Button("Allow Once") { model.send("agent-authorize", extra: ["id": session.id, "decision": "once"]) }
                            Button("Deny") { model.send("agent-authorize", extra: ["id": session.id, "decision": "deny"]) }
                        }
                    } else {
                        if session.status == "connection_lost" {
                            Text("Last activity \(max(0, (model.snapshot?.agentNow ?? 0) - session.last_heartbeat) / 60)m ago")
                        }
                        HStack {
                            Button("Cancel session") { model.send("agent-cancel", extra: ["id": session.id]) }
                            if session.status == "connection_lost" {
                                Menu("Resolve") {
                                    Button("Wait 30 minutes") { model.send("agent-wait", extra: ["id": session.id]) }
                                    Button("End and apply completion action") { model.send("agent-finish", extra: ["id": session.id]) }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    private func agentActionName(_ action: String) -> String {
        ["sleep": "Sleep", "hibernate": "Hibernate", "lock": "Lock", "displayOff": "Turn display off", "shutdown": "Shut down"][action] ?? action
    }

    @ViewBuilder private var agentSheet: some View {
        let name = selectedAgent ?? "Agent"
        VStack(alignment: .leading, spacing: 16) {
            Text(showAgentPermissions ? "\(name) permissions" : "Set up \(name)").font(.title2).bold()
            if showAgentPermissions, let client = model.agentSettings?.clients.first(where: { $0.name == name }) {
                Text("Allow automatically when a switch is on. Otherwise, Doze asks for each session. Changes apply immediately and cancel this agent’s active sessions.").foregroundStyle(.secondary)
                LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], alignment: .leading, spacing: 20) {
                    Toggle("Keep awake", isOn: Binding(get: { client.keepAwake }, set: { _ in model.send("agent-permission", extra: ["id": client.id]) }))
                    ForEach(model.snapshot?.actions ?? [], id: \.self) { action in
                        Toggle(agentActionName(action), isOn: Binding(get: { client.actions.contains(action) }, set: { _ in model.send("agent-permission", extra: ["id": client.id, "action": action]) }))
                    }
                }.toggleStyle(.switch)
                Button("Revoke connection", role: .destructive) { confirmAgentRevocation = true }
                    .confirmationDialog("Revoke \(name)? This cancels its sessions and invalidates its credential.", isPresented: $confirmAgentRevocation) {
                        Button("Revoke", role: .destructive) { model.send("agent-revoke", extra: ["id": client.id]); selectedAgent = nil }
                    }
            } else if let connection = model.snapshot?.agentConnections?.first(where: { $0.name == name }) {
                Text("Add Doze to your client, then reload it. Creating a profile here does not install or verify the client connection.").foregroundStyle(.secondary)
                Text(name == "Codex" ? "1. Add this to ~/.codex/config.toml." : name == "Claude Code" ? "1. Add this JSON with claude mcp add-json --scope user doze '<JSON>'." : "1. Add this to your client’s mcpServers configuration.")
                let config = name == "Codex" ? connection.codex : name == "Claude Code" ? connection.claude : connection.generic
                ScrollView([.horizontal, .vertical]) {
                    Text(config).font(.system(.callout, design: .monospaced)).textSelection(.enabled).padding(12).frame(maxWidth: .infinity, alignment: .leading)
                }.frame(height: 180).background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
                Button(copiedAgentConfig ? "Copied" : "Copy configuration") {
                    NSPasteboard.general.clearContents()
                    copiedAgentConfig = NSPasteboard.general.setString(config, forType: .string)
                }
                agentSkillControls(name)
                Text("2. Reload your client and ask it to use Doze.\n3. Approve its first request in Doze, or choose persistent permissions in Settings.")
                Text("This configuration contains a private credential. Keep it out of shared files and source control.").font(.caption).foregroundStyle(.secondary)
            } else {
                ProgressView("Creating your local profile…")
            }
            if !model.notice.isEmpty { Text(model.notice).foregroundStyle(.secondary) }
            HStack { Spacer(); Button("Done") { selectedAgent = nil }.keyboardShortcut(.defaultAction) }
        }.padding(24).frame(width: 520)
    }

    @ViewBuilder private func agentSkillControls(_ name: String) -> some View {
        let skill = model.snapshot?.agentSkills?.first { $0.name == name }
        Text("Companion skill").fontWeight(.semibold)
        if let skill = skill {
            Text(skill.status == "installed" ? "Installed · \(skill.path ?? "")" : skill.status == "not_installed" ? "Install for this client at \(skill.path ?? "")" : skill.status == "update_available" ? "An existing copy differs from this release. Review local edits before updating." : "Automatic installation is unavailable. Copy the skill manually.")
                .font(.caption).foregroundStyle(.secondary)
        } else {
            Text("Copy the bundled doze folder into your client’s skill directory.").font(.caption).foregroundStyle(.secondary)
        }
        HStack {
            if skill?.status == "not_installed" {
                Button("Install Doze skill") { model.send("agent-skill-install", extra: ["name": name]) }
            } else if skill?.status == "update_available" {
                Button("Review update") { confirmSkillUpdate = true }
                    .confirmationDialog("Update Doze skill? Your current copy will be saved in a separate backup folder. Extra files are retained; local instruction edits are not merged.", isPresented: $confirmSkillUpdate) {
                        Button("Update and keep backup") { model.send("agent-skill-update", extra: ["name": name]) }
                    }
            }
            Button("Open skill folder") { model.send("agent-skill-folder") }
        }
        if let message = model.snapshot?.agentSkillMessage, message.hasPrefix("Doze skill") {
            Text(message).font(.caption).textSelection(.enabled)
        }
    }

    private func explanation(_ title: String, _ detail: String, _ symbol: String) -> some View {
        Label {
            VStack(alignment: .leading, spacing: 5) {
                Text(title).fontWeight(.semibold)
                Text(detail).font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
            }
        } icon: { Image(systemName: symbol).frame(width: 24) }
        .padding(.vertical, 5)
    }

    private func number(_ title: String, _ binding: Binding<Int>, _ range: ClosedRange<Int>) -> some View {
        let validated = Binding<Int>(get: { binding.wrappedValue }, set: { value in
            guard range.contains(value) else {
                model.notice = "Choose a value between \(range.lowerBound) and \(range.upperBound)."
                return
            }
            binding.wrappedValue = value
        })
        return LabeledContent(title) {
            TextField(title, value: validated, format: .number).labelsHidden().frame(width: 90)
                .accessibilityLabel(title)
            Stepper(title, value: validated, in: range).labelsHidden().accessibilityLabel(title)
        }
    }

    private func actionPicker(_ title: String, _ binding: Binding<String>) -> some View {
        Picker(title, selection: binding) {
            ForEach(model.snapshot?.actions ?? ["sleep"], id: \.self) { action in
                Text(action == "displayOff" ? "Turn display off" : action.capitalized).tag(action)
            }
        }
    }
}

struct WarningView: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "moon.zzz").font(.system(size: 24)).foregroundStyle(.tint)
            Text("\(model.warningAction) in").font(.title2.weight(.semibold))
            Text("\(model.remaining / 60):\(String(format: "%02d", model.remaining % 60))")
                .font(.system(size: 64, weight: .semibold, design: .rounded))
                .monospacedDigit().accessibilityAddTraits(.updatesFrequently)
            Text(model.preview ? "Preview only — no power action is scheduled." : "Cancel the action or snooze for 15 minutes.")
                .foregroundStyle(.secondary)
            HStack {
                Button("Snooze 15 minutes", action: model.snoozeWarning)
                Button("Cancel", action: model.cancelWarning).keyboardShortcut(.cancelAction)
                if !model.preview { Button("Stay Awake") { model.send("stay-awake") } }
            }.padding(12).navigationGlass()
        }.padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct TimerView: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            Text(model.timerIsAwake ? "Keep Awake" : "Power Timer").font(.title2.bold())
            if model.timerUsesDate {
                DatePicker("End time", selection: $model.targetDate, in: Date()...Date().addingTimeInterval(604800))
            } else {
                TextField("Duration (minutes)", value: $model.durationMinutes, format: .number)
                    .textFieldStyle(.roundedBorder)
                Text("From 1 minute to 7 days.").foregroundStyle(.secondary)
            }
            if !model.notice.isEmpty { Text(model.notice).foregroundStyle(.red) }
            HStack {
                Button("Cancel", action: model.closeTimer).keyboardShortcut(.cancelAction)
                Spacer()
                Button("Start", action: model.startTimer).keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(maxWidth: .infinity)
        .navigationGlass()
        .padding(16)
    }
}

@main
enum DozeNativeUI {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let ui = NativeUI()
        if let index = CommandLine.arguments.firstIndex(of: "--render-ui"), index + 1 < CommandLine.arguments.count {
            do {
                try ui.render(to: URL(fileURLWithPath: CommandLine.arguments[index + 1]))
            } catch {
                FileHandle.standardError.write(Data("\(error)\n".utf8))
                exit(1)
            }
            return
        }
        if CommandLine.arguments.contains("--verify-ui") {
            do {
                try ui.verify()
                print("Verified: native macOS pages in light/dark, queued immediate changes, failure rollback, reset, and preview snooze.")
            } catch {
                FileHandle.standardError.write(Data("\(error)\n".utf8))
                exit(1)
            }
            return
        }
        ui.start()
        withExtendedLifetime(ui) { app.run() }
    }
}
