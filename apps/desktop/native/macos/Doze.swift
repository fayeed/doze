import AppKit
import SwiftUI

extension Color {
    /// Brand accent: the coral end of the setting-sun gradient, darker in light appearances and
    /// lighter in dark ones so tinted controls and text stay readable.
    static let dozeAccent = Color(nsColor: NSColor(name: nil) { appearance in
        appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
            ? NSColor(srgbRed: 0.969, green: 0.639, blue: 0.478, alpha: 1)
            : NSColor(srgbRed: 0.761, green: 0.271, blue: 0.247, alpha: 1)
    })
}

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
    var menuBarTime = true
}

struct AgentClient: Codable, Identifiable {
    var id: String; var name: String; var keepAwake: Bool; var actions: [String]
}
struct AgentSettings: Codable {
    var enabled: Bool; var leaseSeconds: Int; var defaultCompletion: String?; var clients: [AgentClient]
    var keepAliveWhileConnected: Bool?
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
struct SessionTimer: Codable { var action: String; var remaining: Int }
struct SessionCountdown: Codable { var action: String; var remaining: Int; var source: String }
struct SessionState: Codable {
    var awake = false
    var awakeRemaining: Int?
    var whileAudio = false
    var holdingAwake = false
    var playbackEnabled = false
    var playbackPhase = "waiting"
    var selectedAction = "sleep"
    var timer: SessionTimer?
    var countdown: SessionCountdown?
    var error: String?
    var message: String?
}

/// Labels match the Rust engine and the menu bar.
func actionLabel(_ action: String) -> String {
    ["sleep": "Sleep", "hibernate": "Hibernate", "shutdown": "Shut down", "lock": "Lock",
     "displayOff": "Turn display off"][action] ?? action
}

func agentStatusLabel(_ status: String) -> String {
    switch status {
    case "active": return "Working"
    case "connection_lost": return "Connection lost · keeping awake"
    case "awaiting_authorization": return "Waiting for your approval"
    default: return status
    }
}

func playbackPhaseLabel(_ phase: String) -> String {
    switch phase {
    case "playing": return "Playing · waiting for it to stop"
    case "grace": return "Waiting for silence and inactivity"
    case "countdown": return "Final warning shown"
    default: return "Waiting for playback to start"
    }
}

func remainingText(_ seconds: Int) -> String {
    if seconds >= 3600 { return "\(seconds / 3600)h \(seconds / 60 % 60)m" }
    if seconds >= 60 { return "\((seconds + 59) / 60)m" }
    return "\(seconds)s"
}

struct EngineSnapshot: Codable {
    var settings: Preferences
    var session: SessionState?
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
    var executable: String?
    var iconPath: String?
    var clypyIconPath: String?
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
    /// Settings that live on each page, so search finds them as well as page titles.
    var keywords: String {
        switch self {
        case .overview: return "status session control keep awake stop extend timer countdown audio playback start"
        case .general: return "launch login sign in startup menu bar theme appearance dark light time remaining"
        case .session: return "keep awake display sleep screen default duration minutes power timer action"
        case .playback: return "audio music video silence inactivity idle after playback"
        case .notifications: return "final warning countdown duration seconds notification preview snooze"
        case .agents: return "mcp codex claude code agent lease heartbeat permissions skill connection keep alive connected"
        case .advanced: return "logging diagnostics log reset defaults data finder preferences command line terminal cli run watch job"
        case .help: return "menu guide help explain"
        case .about: return "version privacy about acknowledgements"
        }
    }
    func matches(_ query: String) -> Bool {
        let words = query.split(separator: " ")
        return words.allSatisfy { rawValue.localizedCaseInsensitiveContains($0) || keywords.localizedCaseInsensitiveContains($0) }
    }
    /// Sidebar groups, separated like System Settings.
    static let groups: [[Page]] = [[.overview], [.general, .session, .playback, .notifications], [.agents, .advanced], [.help, .about]]
    var symbol: String {
        switch self {
        case .overview: return "moon.zzz.fill"
        case .general: return "gearshape.fill"
        case .session: return "sun.max.fill"
        case .playback: return "speaker.wave.2.fill"
        case .notifications: return "bell.badge.fill"
        case .agents: return "person.2.fill"
        case .advanced: return "slider.horizontal.3"
        case .help: return "book.fill"
        case .about: return "info"
        }
    }
    var color: Color {
        switch self {
        case .overview: return .indigo
        case .general: return .gray
        case .session: return .orange
        case .playback: return .pink
        case .notifications: return .red
        case .agents: return .teal
        case .advanced: return .gray
        case .help: return .brown
        case .about: return .blue
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
    @Published var timerAction = "sleep"
    @Published var appIcon: NSImage?

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
                let host = NSHostingView(rootView: SettingsView(model: self).tint(.dozeAccent))
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
        guard Page.session.matches("display sleep"), Page.general.matches("Login"), !Page.about.matches("lease") else {
            throw verificationError("Search does not find settings by keyword.")
        }
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
        snapshot?.session = SessionState(awake: true, awakeRemaining: 2520, holdingAwake: true, playbackEnabled: true,
                                         timer: SessionTimer(action: "sleep", remaining: 3900))
        draft = snapshot!.settings
        func capture(_ view: some View, size: NSSize, name: String, appearance: NSAppearance.Name) throws {
            let window = NSWindow(contentRect: NSRect(origin: NSPoint(x: -20000, y: -20000), size: size),
                                  styleMask: [.titled, .fullSizeContentView], backing: .buffered, defer: false)
            window.appearance = NSAppearance(named: appearance)
            window.titlebarAppearsTransparent = true
            let host = NSHostingView(rootView: view.tint(.dozeAccent))
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
        let session = snapshot?.session
        let live = page == .overview && (session?.awakeRemaining != nil || session?.timer != nil || session?.countdown != nil)
        if ticks % (live ? 1 : 5) == 0, settingsWindow?.isVisible == true, !saving { send("refresh") }
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
            if appIcon == nil, let path = incoming.iconPath { appIcon = NSImage(contentsOfFile: path) }
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
        window.contentView = NSHostingView(rootView: content.tint(.dozeAccent))
        window.center()
        return window
    }

    private func activate(_ window: NSWindow) {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    private func showSettings() {
        if settingsWindow == nil {
            settingsWindow = window("Doze Settings", size: NSSize(width: 860, height: 640), content: SettingsView(model: self))
            settingsWindow?.minSize = NSSize(width: 740, height: 500)
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
        timerAction = snapshot?.session?.selectedAction ?? draft.defaultAction
        notice = ""
        if timerWindow == nil {
            timerWindow = window("Doze · Custom session", size: NSSize(width: 480, height: 360), content: TimerView(model: self))
            timerWindow?.styleMask.remove([.resizable, .miniaturizable])
        }
        if let window = timerWindow {
            // Fit the form: duration and end-time variants differ in height.
            if let content = window.contentView { window.setContentSize(content.fittingSize) }
            activate(window)
        }
    }

    /// Opens the custom duration or end-time window from the control center.
    func openTimer(awake: Bool, date: Bool) { showTimer(awake: awake, date: date) }

    func startTimer() {
        guard timerUsesDate || (1...10080).contains(durationMinutes) else {
            notice = "Choose a duration between 1 minute and 7 days."; return
        }
        let seconds = timerUsesDate ? Int(ceil(targetDate.timeIntervalSinceNow)) : durationMinutes * 60
        guard (60...604800).contains(seconds) else { notice = "Choose a duration between 1 minute and 7 days."; return }
        send(timerIsAwake ? "awake" : "timer",
             extra: timerIsAwake ? ["seconds": seconds] : ["seconds": seconds, "action": timerAction])
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
    @State private var confirmReset = false

    var body: some View {
        NavigationSplitView {
            List(selection: $model.page) {
                sidebarHeader
                ForEach(Page.groups, id: \.self) { group in
                    let pages = group.filter { model.search.isEmpty || $0.matches(model.search) }
                    if !pages.isEmpty {
                        Section {
                            ForEach(pages) { page in
                                Label {
                                    Text(page.rawValue)
                                } icon: {
                                    SettingsIcon(symbol: page.symbol, color: page.color)
                                }
                                .tag(page)
                            }
                        }
                    }
                }
            }
            .searchable(text: $model.search, placement: .sidebar, prompt: "Search")
            // Outermost on the column, or the split view ignores it and truncates page names.
            .navigationSplitViewColumnWidth(min: 210, ideal: 225, max: 300)
        } detail: {
            VStack(spacing: 0) {
                Form {
                    pageContent
                }
                .formStyle(.grouped)
                // New native form identity also resets scroll when changing pages.
                .id(model.page)
                if !model.notice.isEmpty {
                    Label(model.notice, systemImage: "exclamationmark.triangle.fill")
                        .font(.callout).foregroundStyle(.secondary).textSelection(.enabled).padding(12)
                }
            }
            .navigationTitle(model.page?.rawValue ?? "Doze")
        }
        .sheet(isPresented: Binding(get: { selectedAgent != nil }, set: { if !$0 { selectedAgent = nil } })) {
            agentSheet
        }
        .confirmationDialog("Reset all preferences to their defaults?", isPresented: $confirmReset) {
            Button("Reset Preferences", role: .destructive, action: model.reset)
        } message: {
            Text("Agent connections, permissions and running sessions are kept.")
        }
    }

    /// Like the account row in System Settings: the app and what it is doing right now.
    private var sidebarHeader: some View {
        HStack(spacing: 10) {
            AppIcon(model: model, size: 34)
            VStack(alignment: .leading, spacing: 1) {
                Text("Doze").font(.headline)
                Text(model.snapshot?.status ?? "Normal sleep allowed")
                    .font(.caption).foregroundStyle(.secondary).lineLimit(1)
            }
        }
        .padding(.vertical, 4)
        .contentShape(Rectangle())
        .onTapGesture { model.page = .overview }
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
    }

    @ViewBuilder private var pageContent: some View {
        switch model.page ?? .overview {
        case .overview:
            controlCenter
        case .general:
            Section("Startup") {
                toggle("Launch at login", "Doze starts quietly in the menu bar when you log in.", $model.draft.launchAtStartup)
                    .disabled(model.snapshot?.startupSupported != true)
                toggle("Start in the menu bar", "Opening Doze doesn’t show this window.", $model.draft.startMinimized)
            }
            Section("Display") {
                Picker(selection: $model.draft.theme) {
                    Text("Automatic").tag("system")
                    Text("Light").tag("light")
                    Text("Dark").tag("dark")
                } label: {
                    described("Appearance", "Automatic follows your Mac.")
                }
                toggle("Show time remaining in the menu bar", "The final warning, power timer or timed Keep Awake appears beside Doze’s icon.", $model.draft.menuBarTime)
            }
        case .session:
            Section {
                minutesPicker("Default duration", "Used by Keep Awake › Default in the menu bar.", $model.draft.defaultAwakeMinutes)
                toggle("Allow the display to sleep", "Your Mac stays awake, but the screen can turn off.", $model.draft.allowDisplaySleep)
            } header: {
                Text("Keep Awake")
            }
            Section {
                minutesPicker("Default duration", "Used by Power Timer › Default in the menu bar.", $model.draft.defaultTimerMinutes)
                actionPicker("Default action", "Applies to new timers.", $model.draft.defaultAction)
            } header: {
                Text("Power Timer")
            } footer: {
                Text("A timer keeps your Mac awake until its final warning.").foregroundStyle(.secondary)
            }
        case .playback:
            if model.snapshot?.audioSupported != true {
                Section { explanation("Audio monitoring unavailable", "After Playback and keeping awake while audio plays are unavailable in this build.", "speaker.slash.fill", .gray) }
            }
            Section {
                actionPicker("When playback ends", "What happens once audio has stopped and you’re away.", $model.draft.playbackAction)
                secondsPicker("After silence of", "Pauses, buffering and quiet scenes shorter than this are ignored.", $model.draft.silenceSeconds, [10, 20, 30, 60, 120, 300, 600, 900, 1800])
                secondsPicker("And no activity for", "Using the keyboard or mouse restarts this wait.", $model.draft.idleSeconds, [30, 60, 120, 300, 600, 900, 1800, 3600])
            } footer: {
                Text("Turn it on from the menu bar or Overview before you start watching. It is one-shot: after its action runs, or when you choose Cancel or Stay Awake on its warning, it turns itself off.")
                    .foregroundStyle(.secondary)
            }
            .disabled(model.snapshot?.audioSupported != true)
        case .notifications:
            Section {
                secondsPicker("Warning length", "How long you have to cancel before the action runs.", $model.draft.countdownSeconds, [15, 30, 60, 120, 180, 300, 600, 900, 1800])
                toggle("Show a notification", "Also announces the final warning in Notification Center.", $model.draft.notifications)
                setting("Preview", "Shows the warning without scheduling anything.") {
                    Button("Preview Warning") { model.send("preview") }
                }
            } header: {
                Text("Final warning")
            } footer: {
                Text("The floating warning window always offers Cancel, Snooze 15 minutes and Stay Awake.").foregroundStyle(.secondary)
            }
        case .agents:
            agentsView
        case .advanced:
            Section("Diagnostics") {
                toggle("Write diagnostic logs", "Errors are logged on this Mac, up to about 256 KB.", $model.draft.logging)
                setting("Preferences file", model.snapshot?.settingsPath ?? "Loading…") {
                    Button("Show in Finder", action: model.openData)
                }
            }
            Section {
                let alias = "alias doze=" + shellQuoted(model.snapshot?.executable ?? "/Applications/Doze.app/Contents/MacOS/doze")
                setting("Shell alias", "Add it to ~/.zshrc so you can type doze in Terminal.") {
                    Button("Copy Alias") {
                        NSPasteboard.general.clearContents()
                        NSPasteboard.general.setString(alias, forType: .string)
                    }
                }
                Text("doze run --then sleep -- ffmpeg -i in.mov out.mp4\ndoze watch --pid 1234 --then sleep")
                    .font(.system(.callout, design: .monospaced)).textSelection(.enabled)
            } header: {
                Text("Command line")
            } footer: {
                Text("Keeps your Mac awake while a job runs. --then accepts nothing, sleep, display-off, lock or shutdown, and runs only after the job succeeds and the final warning ends. A failed job or Ctrl-C releases without any action.")
                    .foregroundStyle(.secondary)
            }
            Section {
                setting("Reset preferences", "Restores the defaults on every page.") {
                    Button("Reset…") { confirmReset = true }
                }
            } footer: {
                Text("Doze validates every change and owns all power actions. Closing this window keeps Doze running; sessions are never restored after a restart.")
                    .foregroundStyle(.secondary)
            }
        case .help:
            Section {
                explanation("Normal sleep allowed", "No Doze session is holding your Mac awake. Your macOS sleep settings apply normally.", "moon.fill", .indigo)
                explanation("Keep Awake", "Choose a duration, an end time, or indefinitely. Stop releases Doze’s power assertion; Extend adds 15 minutes to a timed session.", "sun.max.fill", .orange)
                explanation("Power Timer", "Choose an action and duration. A final warning lets you cancel or snooze before the action runs.", "timer", .blue)
                explanation("Keep awake while audio plays", "Holds your Mac awake while sound is playing, through the silence grace period. Muted or zero-volume output counts as silence.", "speaker.wave.2.fill", .pink)
                explanation("After Playback", "Once playback has been seen, Doze waits for silence and inactivity, then shows the final warning. It turns itself off after it runs or when you cancel it.", "play.slash.fill", .purple)
                explanation("Final warning", "Every power action shows a floating warning first. Cancel removes the action, Snooze waits 15 more minutes, and Stay Awake keeps your Mac awake instead.", "hourglass", .red)
            } header: {
                Text("In the menu bar")
            }
            Section {
                explanation("Command line", "doze run --then sleep -- your-command stays awake until a job finishes; doze watch --pid follows one already running. See Advanced.", "terminal.fill", .gray)
                explanation("Agents", "Coding agents connected through MCP keep your Mac awake while they work. New requests need your approval in Agents unless you granted them there.", "person.2.fill", .teal)
                explanation("Quick Settings", "Checkmarks are saved defaults. Duration defaults apply to new sessions; other changes, such as display sleep, apply immediately.", "slider.horizontal.3", .gray)
                explanation("Unavailable commands", "Commands such as Stop or Cancel appear dimmed when there is nothing to stop. Actions this Mac does not support stay dimmed.", "info", .gray)
            } header: {
                Text("More ways to use Doze")
            }
        case .about:
            Section {
                VStack(spacing: 6) {
                    AppIcon(model: model, size: 96)
                    Text("Doze").font(.title.bold())
                    Text("Version \(model.snapshot?.version ?? "0.1.0")").foregroundStyle(.secondary)
                    Text("Your computer knows when it’s bedtime.").padding(.top, 2)
                    Text("Made by Fayeed Pawaskar").font(.callout).foregroundStyle(.secondary)
                    Button("Show Local Data", action: model.openData).buttonStyle(.link)
                        .font(.callout).padding(.top, 6)
                }
                .frame(maxWidth: .infinity).padding(.vertical, 14)
            }
            Section {
                HStack(alignment: .top, spacing: 12) {
                    if let path = model.snapshot?.clypyIconPath, let icon = NSImage(contentsOfFile: path) {
                        Image(nsImage: icon).resizable().interpolation(.high).frame(width: 26, height: 26)
                            .accessibilityHidden(true)
                    } else {
                        SettingsIcon(symbol: "paperclip", color: .blue, size: 26)
                    }
                    described("Clypy", "A clipboard manager for Mac, Windows, Linux and phones, also by Fayeed Pawaskar.")
                    Spacer()
                    Link("Visit", destination: URL(string: "https://clypy.app")!)
                }
                .padding(.vertical, 3)
            } header: {
                Text("Also from the developer")
            }
            Section {
                explanation("No account. No cloud. No ads.", "Doze does not record audio, simulate input or send telemetry. Preferences and optional diagnostics stay on this Mac.", "lock.fill", .green)
            } header: {
                Text("Privacy")
            }
            Section {
                explanation("Rust and Tauri", "The engine owns sessions, power assertions, validation and countdown safety.", "gearshape.2.fill", .orange)
                explanation("SwiftUI and AppKit", "Settings, timers and the final warning use Apple’s native controls and materials.", "swift", .orange)
            } header: {
                Text("Built with")
            } footer: {
                Text("Open-source components retain their respective licenses.").foregroundStyle(.secondary)
            }
        }
    }

    private var session: SessionState { model.snapshot?.session ?? SessionState() }
    private var actions: [String] { model.snapshot?.actions ?? ["sleep"] }
    private let presets: [(Int, String)] = [(900, "15m"), (1800, "30m"), (3600, "1h"), (7200, "2h")]
    private var statusSymbol: String {
        if session.countdown != nil { return "timer" }
        return session.holdingAwake ? "sun.max.fill" : "moon.zzz.fill"
    }
    private var awakeText: String {
        guard let remaining = session.awakeRemaining else { return "Indefinitely" }
        return remainingText(remaining) + " left"
    }
    private var statusTint: Color {
        session.holdingAwake || session.countdown != nil ? .orange : .indigo
    }

    /// Overview doubles as a control center: everything in the menu bar, with live times.
    @ViewBuilder private var controlCenter: some View {
        Section {
            HStack(spacing: 14) {
                SettingsIcon(symbol: statusSymbol, color: statusTint, size: 40)
                VStack(alignment: .leading, spacing: 3) {
                    Text(model.snapshot?.status ?? "Normal sleep allowed").font(.title3.weight(.semibold))
                        .textSelection(.enabled)
                    Text(model.snapshot?.timerStatus ?? "No power action scheduled").foregroundStyle(.secondary)
                    if let message = session.message, !message.hasPrefix("Doze skill") {
                        Label(message, systemImage: "clock.arrow.circlepath")
                            .font(.callout).foregroundStyle(.secondary).textSelection(.enabled)
                            .accessibilityLabel("Last event: " + message)
                    }
                }
            }.padding(.vertical, 6)
        }
        if let countdown = session.countdown {
            Section("Final warning") {
                LabeledContent(actionLabel(countdown.action) + " in") {
                    Text(String(format: "%d:%02d", countdown.remaining / 60, countdown.remaining % 60))
                        .font(.title2.monospacedDigit().weight(.semibold))
                }
                HStack {
                    Button("Snooze 15 minutes") { model.send("snooze") }
                    Button("Stay Awake") { model.send("stay-awake") }
                    Spacer()
                    Button("Cancel", role: .destructive) { model.send("cancel") }
                }
            }
        }
        Section("Keep Awake") {
            if session.awake {
                LabeledContent("Active") {
                    Text(awakeText).monospacedDigit()
                }
                HStack {
                    if session.awakeRemaining != nil {
                        Button("Extend 15 minutes") { model.send("extend") }
                    }
                    Spacer()
                    Button("Stop keeping awake") { model.send("stop-awake") }
                }
            } else {
                presetRow(spoken: "Keep awake for", start: { model.send("awake", extra: ["seconds": $0]) }) {
                    Button("Indefinitely") { model.send("awake-forever") }
                    Menu("More") {
                        Button("Custom duration…") { model.openTimer(awake: true, date: false) }
                        Button("Until a specific time…") { model.openTimer(awake: true, date: true) }
                    }.fixedSize()
                }
            }
            toggle("Keep awake while audio plays", "Holds your Mac awake while sound is playing.", Binding(get: { session.whileAudio }, set: { _ in model.send("audio-toggle") }))
                .disabled(model.snapshot?.audioSupported != true)
        }
        Section("Power Timer") {
            Picker("Action", selection: Binding(get: { session.selectedAction }, set: { model.send("select-action", extra: ["action": $0]) })) {
                ForEach(actions, id: \.self) { Text(actionLabel($0)).tag($0) }
            }
            if let timer = session.timer {
                LabeledContent(actionLabel(timer.action) + " in") { Text(remainingText(timer.remaining)).monospacedDigit() }
                HStack { Spacer(); Button("Stop timer") { model.send("stop-timer") } }
            } else {
                presetRow(spoken: actionLabel(session.selectedAction) + " in", start: { model.send("timer", extra: ["seconds": $0, "action": session.selectedAction]) }) {
                    Menu("More") {
                        Button("Custom duration…") { model.openTimer(awake: false, date: false) }
                        Button("At a specific time…") { model.openTimer(awake: false, date: true) }
                    }.fixedSize()
                }
            }
            toggle(actionLabel(model.snapshot?.settings.playbackAction ?? "sleep") + " after playback stops",
                   session.playbackEnabled ? playbackPhaseLabel(session.playbackPhase) : "Turn on before you start watching; it turns off after it runs.",
                   Binding(get: { session.playbackEnabled }, set: { _ in model.send("playback-toggle") }))
                .disabled(model.snapshot?.audioSupported != true)
        }
        Section("Quick access") {
            HStack {
                Button("Preview the countdown") { model.send("preview") }
                Button("Session defaults") { model.page = .session }
                Spacer()
                Button("Show local data", action: model.openData)
            }
        }
    }

    private func presetRow(spoken: String, start: @escaping (Int) -> Void, @ViewBuilder trailing: () -> some View) -> some View {
        HStack {
            Text("Start").foregroundStyle(.secondary)
            ForEach(presets, id: \.0) { preset in
                Button(preset.1) { start(preset.0) }.accessibilityLabel(spoken + " " + preset.1)
            }
            Spacer()
            trailing()
        }
    }

    @ViewBuilder private var agentsView: some View {
        Section {
            toggle("Allow agents to connect", "Coding agents such as Codex and Claude Code connect through MCP on this Mac.", Binding(get: { model.agentSettings?.enabled ?? false }, set: { _ in model.send("agent-enable") }))
            Picker(selection: Binding(get: { model.agentSettings?.leaseSeconds ?? 300 }, set: { model.send("agent-lease", extra: ["agent_seconds": $0]) })) {
                ForEach([60, 300, 900, 1800, 3600], id: \.self) { seconds in
                    Text(leaseLabel(seconds)).tag(seconds)
                }
            } label: {
                described("Heartbeat lease", "How long a session stays awake between check-ins.")
            }
            Picker(selection: Binding(get: { model.agentSettings?.defaultCompletion ?? "normal" }, set: { model.send("agent-default", extra: ["action": $0 == "normal" ? NSNull() : $0 as Any]) })) {
                Text("Return to normal").tag("normal")
                ForEach(model.snapshot?.actions ?? [], id: \.self) { Text(actionLabel($0)).tag($0) }
            } label: {
                described("When an agent finishes", "Used when the agent doesn’t ask for an action.")
            }
            toggle("Keep sessions alive while connected", "Renews a session through long steps without check-ins, for up to 24 hours. It never finishes a session.", Binding(get: { model.agentSettings?.keepAliveWhileConnected ?? true }, set: { _ in model.send("agent-keepalive") }))
        } footer: {
            Text("A lost connection keeps your Mac awake for up to 30 minutes, then releases without any action. An automatic action needs every overlapping agent to finish with the same approved action, followed by a final warning of at least five minutes.")
                .foregroundStyle(.secondary)
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
                    let finish: String = session.completion_action.map(actionLabel) ?? "Return to normal"
                    Text(agentStatusLabel(session.status) + " · When finished: " + finish).foregroundStyle(.secondary)
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

    private func agentActionName(_ action: String) -> String { actionLabel(action) }

    /// Doze's binary also handles `doze run` and `doze watch`; single quotes keep paths with
    /// spaces intact in the shell alias.
    private func shellQuoted(_ path: String) -> String {
        "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }

    private func leaseLabel(_ seconds: Int) -> String {
        if seconds == 60 { return "1 minute" }
        if seconds == 3600 { return "1 hour" }
        return "\(seconds / 60) minutes"
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

    /// A setting title with its explanation underneath, as in System Settings.
    private func described(_ title: String, _ detail: String?) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            Text(title)
            if let detail {
                Text(detail).font(.subheadline).foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true).textSelection(.enabled)
            }
        }
    }

    private func setting<Control: View>(_ title: String, _ detail: String? = nil, @ViewBuilder control: () -> Control) -> some View {
        LabeledContent { control() } label: { described(title, detail) }
    }

    private func toggle(_ title: String, _ detail: String? = nil, _ binding: Binding<Bool>) -> some View {
        Toggle(isOn: binding) { described(title, detail) }
    }

    private func explanation(_ title: String, _ detail: String, _ symbol: String, _ color: Color = .dozeAccent) -> some View {
        HStack(alignment: .top, spacing: 12) {
            SettingsIcon(symbol: symbol, color: color, size: 26)
            described(title, detail)
        }
        .padding(.vertical, 3)
        .accessibilityElement(children: .combine)
    }

    /// Durations are chosen from menus, like the System Settings energy options. A value
    /// saved outside the list stays available.
    private func minutesPicker(_ title: String, _ detail: String, _ binding: Binding<Int>) -> some View {
        let options = Set([5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1440, binding.wrappedValue]).sorted()
        return Picker(selection: binding) {
            ForEach(options, id: \.self) { Text(durationLabel(minutes: $0)).tag($0) }
        } label: {
            described(title, detail)
        }
    }

    private func secondsPicker(_ title: String, _ detail: String, _ binding: Binding<Int>, _ choices: [Int]) -> some View {
        let options = Set(choices + [binding.wrappedValue]).sorted()
        return Picker(selection: binding) {
            ForEach(options, id: \.self) { Text(durationLabel(seconds: $0)).tag($0) }
        } label: {
            described(title, detail)
        }
    }

    private func actionPicker(_ title: String, _ detail: String? = nil, _ binding: Binding<String>) -> some View {
        Picker(selection: binding) {
            ForEach(model.snapshot?.actions ?? ["sleep"], id: \.self) { action in
                Text(actionLabel(action)).tag(action)
            }
        } label: {
            described(title, detail)
        }
    }
}

func durationLabel(minutes: Int) -> String {
    func unit(_ value: Int, _ name: String) -> String { "\(value) \(name)\(value == 1 ? "" : "s")" }
    if minutes < 60 { return unit(minutes, "minute") }
    if minutes % 60 == 0 { return minutes % 1440 == 0 ? unit(minutes / 1440, "day") : unit(minutes / 60, "hour") }
    return unit(minutes / 60, "hour") + " " + unit(minutes % 60, "minute")
}

func durationLabel(seconds: Int) -> String {
    func unit(_ value: Int, _ name: String) -> String { "\(value) \(name)\(value == 1 ? "" : "s")" }
    if seconds < 60 { return unit(seconds, "second") }
    if seconds % 60 == 0 { return durationLabel(minutes: seconds / 60) }
    return unit(seconds / 60, "minute") + " " + unit(seconds % 60, "second")
}

/// The rounded-square icons System Settings uses for its sidebar and rows.
struct SettingsIcon: View {
    let symbol: String
    let color: Color
    var size: CGFloat = 22
    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: size * 0.55, weight: .semibold))
            .foregroundStyle(.white)
            .frame(width: size, height: size)
            .background(
                LinearGradient(colors: [color.opacity(0.8), color], startPoint: .top, endPoint: .bottom),
                in: RoundedRectangle(cornerRadius: size * 0.26, style: .continuous)
            )
            .accessibilityHidden(true)
    }
}

/// Doze's own app icon, with the moon symbol as a fallback before the engine reports it.
struct AppIcon: View {
    @ObservedObject var model: NativeUI
    let size: CGFloat
    var body: some View {
        Group {
            if let image = model.appIcon {
                Image(nsImage: image).resizable().interpolation(.high)
            } else {
                SettingsIcon(symbol: "moon.zzz.fill", color: .indigo, size: size)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

struct WarningView: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "moon.zzz").font(.system(size: 24)).foregroundStyle(.tint).accessibilityHidden(true)
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
    private let presets = [15, 30, 60, 120, 240, 480]
    private var startTitle: String {
        model.timerIsAwake ? "Keep Awake" : "Start Timer"
    }
    private func presetTitle(_ minutes: Int) -> String {
        minutes < 60 ? "\(minutes)m" : "\(minutes / 60)h"
    }

    private var endText: String {
        if model.timerUsesDate {
            let seconds = max(0, Int(model.targetDate.timeIntervalSinceNow.rounded()))
            return seconds < 60 ? "Choose a time at least a minute from now." : "In \(remainingText(seconds))"
        }
        guard (1...10080).contains(model.durationMinutes) else { return "From 1 minute to 7 days." }
        let end = Date().addingTimeInterval(Double(model.durationMinutes * 60))
        return "Ends \(end.formatted(date: Calendar.current.isDateInToday(end) ? .omitted : .abbreviated, time: .shortened))"
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            VStack(alignment: .leading, spacing: 4) {
                Text(model.timerIsAwake ? "Keep Awake" : "Power Timer").font(.title2.bold())
                Text(model.timerIsAwake ? "Your Mac stays awake, then normal sleep settings apply again."
                     : "A final warning lets you cancel or snooze before the action runs.")
                    .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
            }
            if !model.timerIsAwake {
                Picker("Action", selection: $model.timerAction) {
                    ForEach(model.snapshot?.actions ?? ["sleep"], id: \.self) { Text(actionLabel($0)).tag($0) }
                }
            }
            if model.timerUsesDate {
                DatePicker(model.timerIsAwake ? "Until" : "At", selection: $model.targetDate,
                           in: Date()...Date().addingTimeInterval(604800))
            } else {
                HStack {
                    Text("Duration")
                    Spacer()
                    TextField("Minutes", value: $model.durationMinutes, format: .number)
                        .textFieldStyle(.roundedBorder).frame(width: 80).multilineTextAlignment(.trailing)
                        .accessibilityLabel("Duration in minutes")
                    Stepper("Duration in minutes", value: $model.durationMinutes, in: 1...10080, step: 5).labelsHidden()
                    Text("minutes").foregroundStyle(.secondary)
                }
                HStack(spacing: 6) {
                    ForEach(presets, id: \.self) { minutes in
                        Button(presetTitle(minutes)) { model.durationMinutes = minutes }
                            .controlSize(.small)
                    }
                }
            }
            Text(endText).font(.callout).foregroundStyle(.secondary)
            if !model.notice.isEmpty { Text(model.notice).foregroundStyle(.red) }
            HStack {
                Button("Cancel", action: model.closeTimer).keyboardShortcut(.cancelAction)
                Spacer()
                Button(startTitle, action: model.startTimer)
                    .keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 448)
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
