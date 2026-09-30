import AppKit
import SwiftUI

struct Preferences: Codable, Equatable {
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

struct EngineSnapshot: Decodable {
    var settings: Preferences
    var settingsPath: String
    var actions: [String]
    var audioSupported: Bool
    var startupSupported: Bool
    var status: String
    var timerStatus: String
    var version: String
}

enum Page: String, CaseIterable, Identifiable {
    case overview = "Overview"
    case general = "General"
    case session = "Session defaults"
    case playback = "After playback"
    case notifications = "Notifications"
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
        case .advanced: return "slider.horizontal.3"
        case .help: return "questionmark.circle"
        case .about: return "info.circle"
        }
    }
}

@MainActor
final class NativeUI: NSObject, ObservableObject, NSWindowDelegate {
    @Published var snapshot: EngineSnapshot?
    @Published var draft = Preferences()
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

    var dirty: Bool { snapshot.map { draft != $0.settings } ?? false }

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
        reset()
        guard draft.defaultAwakeMinutes == 30 else { throw verificationError("Reset did not restore defaults.") }
        // Preview buttons must never submit engine commands.
        preview = true
        warningVisible = true
        snoozeWarning()
        guard !warningVisible else { throw verificationError("Preview did not dismiss.") }
    }

    private func verificationError(_ message: String) -> NSError {
        NSError(domain: "Doze.NativeUI", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private func tick() {
        if let deadline = previewDeadline, preview {
            remaining = max(0, Int(ceil(deadline.timeIntervalSinceNow)))
            if remaining == 0 { dismissWarning() }
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
        let type = message["type"] as? String ?? ""
        if let raw = message["snapshot"],
           let data = try? JSONSerialization.data(withJSONObject: raw),
           let incoming = try? JSONDecoder().decode(EngineSnapshot.self, from: data) {
            let replaceDraft = snapshot == nil || !dirty || type == "saved"
            snapshot = incoming
            if replaceDraft { draft = incoming.settings }
        }
        switch type {
        case "open":
            switch message["view"] as? String {
            case "about": page = .about; showSettings()
            case "help": page = .help; showSettings()
            case "awakeDuration": showTimer(awake: true, date: false)
            case "awakeTime": showTimer(awake: true, date: true)
            case "timerDuration": showTimer(awake: false, date: false)
            case "timerTime": showTimer(awake: false, date: true)
            default: showSettings()
            }
        case "saved": saving = false; notice = "Changes saved."
        case "error": saving = false; notice = message["error"] as? String ?? "Unable to complete the request."
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
            warningWindow = window("Doze · Power countdown", size: NSSize(width: 460, height: 260), content: WarningView(model: self))
            warningWindow?.styleMask.remove([.resizable, .miniaturizable])
            warningWindow?.level = .floating
            warningWindow?.delegate = self
        }
        if !warningVisible, let window = warningWindow { activate(window) }
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
        }
        if let window = timerWindow { activate(window) }
    }

    func startTimer() {
        let seconds = timerUsesDate ? Int(ceil(targetDate.timeIntervalSinceNow)) : durationMinutes * 60
        guard (60...604800).contains(seconds) else { notice = "Choose a duration between 1 minute and 7 days."; return }
        send(timerIsAwake ? "awake" : "timer", extra: ["seconds": seconds])
        timerWindow?.orderOut(nil)
    }

    func closeTimer() { timerWindow?.orderOut(nil) }

    func save() {
        guard let data = try? JSONEncoder().encode(draft),
              let settings = try? JSONSerialization.jsonObject(with: data) else { return }
        saving = true
        notice = ""
        send("save", extra: ["settings": settings])
    }

    func discard() { if let saved = snapshot?.settings { draft = saved }; notice = "" }
    func reset() { draft = Preferences(); notice = "Defaults restored. Save to apply." }

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

    var body: some View {
        NavigationSplitView {
            List(selection: $model.page) {
                ForEach(Page.allCases.filter { model.search.isEmpty || $0.rawValue.localizedCaseInsensitiveContains(model.search) }) { page in
                    Label(page.rawValue, systemImage: page.symbol).tag(page)
                }
            }
            .navigationSplitViewColumnWidth(min: 190, ideal: 220)
            .searchable(text: $model.search, placement: .sidebar, prompt: "Find a setting")
            .disabled(model.saving)
        } detail: {
            VStack(spacing: 0) {
                Form {
                    pageContent
                }
                .formStyle(.grouped)
                // New native form identity also resets scroll when changing pages.
                .id(model.page)
                if let page = model.page, page != .about && page != .help {
                    HStack {
                        Text(model.dirty ? "You have unsaved changes." : "Changes apply when you save.")
                            .font(.callout).foregroundStyle(.secondary)
                        Spacer()
                        Button("Reset defaults", action: model.reset)
                        Button("Discard", action: model.discard).disabled(!model.dirty)
                        Button("Save changes", action: model.save).keyboardShortcut("s", modifiers: .command)
                            .disabled(!model.dirty)
                    }
                    .padding(16).navigationGlass().padding(12)
                    .disabled(model.saving)
                }
                if !model.notice.isEmpty {
                    Text(model.notice).font(.callout).textSelection(.enabled).padding(12)
                }
            }
            .navigationTitle(model.page?.rawValue ?? "Doze")
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
        case .advanced:
            Section("Local data") {
                Toggle("Enable diagnostic logging", isOn: $model.draft.logging)
                Text(model.snapshot?.settingsPath ?? "Loading…").font(.callout).textSelection(.enabled)
                Button("Show preferences in Finder", action: model.openData)
            }
            Section("Session safety") { Text("Rust validates settings and owns all power actions. Closing Settings keeps Doze running. Transient sessions are never restored from disk.") }
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
        LabeledContent(title) {
            TextField(title, value: binding, format: .number).labelsHidden().frame(width: 90)
                .accessibilityLabel(title)
            Stepper(title, value: binding, in: range).labelsHidden().accessibilityLabel(title)
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
        VStack(spacing: 18) {
            Image(systemName: "moon.zzz").font(.system(size: 32)).foregroundStyle(.tint)
            Text("\(model.warningAction) in \(model.remaining / 60):\(String(format: "%02d", model.remaining % 60))")
                .font(.title2.bold()).monospacedDigit().accessibilityAddTraits(.updatesFrequently)
            Text(model.preview ? "Preview only — no power action is scheduled." : "Cancel the action or snooze for 15 minutes.")
                .foregroundStyle(.secondary)
            HStack {
                Button("Snooze 15 minutes", action: model.snoozeWarning)
                Button("Cancel", action: model.cancelWarning).keyboardShortcut(.cancelAction)
            }.padding(12).navigationGlass()
        }.padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

struct TimerView: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        Form {
            Text(model.timerIsAwake ? "Keep Awake" : "Power Timer").font(.title2.bold())
            if model.timerUsesDate {
                DatePicker("End time", selection: $model.targetDate, in: Date()...Date().addingTimeInterval(604800))
            } else {
                TextField("Duration (minutes)", value: $model.durationMinutes, format: .number)
                Text("From 1 minute to 7 days.").foregroundStyle(.secondary)
            }
            if !model.notice.isEmpty { Text(model.notice).foregroundStyle(.red) }
            HStack {
                Button("Cancel", action: model.closeTimer).keyboardShortcut(.cancelAction)
                Spacer()
                Button("Start", action: model.startTimer).keyboardShortcut(.defaultAction)
            }
        }.formStyle(.grouped).padding(12)
    }
}

@main
enum DozeNativeUI {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let ui = NativeUI()
        if CommandLine.arguments.contains("--verify-ui") {
            do {
                try ui.verify()
                print("Verified: native macOS pages in light/dark, draft preservation, reset, and preview snooze.")
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
