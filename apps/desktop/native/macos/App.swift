import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// The native macOS companion. The Rust engine owns all state and talks to this process over
/// its inherited stdin/stdout, one JSON object per line: `open`, `panel`, `state` (whenever the
/// engine changes), command replies, and the final warning. Windows render the latest
/// snapshot; they never poll the engine.
@MainActor
final class NativeUI: NSObject, ObservableObject, NSWindowDelegate {
    @Published var snapshot: JSON = [:]
    /// When `snapshot` arrived. Times count down from the engine's `now` on this clock.
    @Published var receivedAt = Date()
    @Published var page: Page? = .overview
    @Published var search = ""
    /// A setting search jumped to, scrolled into view on its page.
    @Published var revealed: String?
    @Published var notice = ""
    @Published var pendingChange: PendingChange?
    @Published var confirmReset = false
    @Published var showAssertions = false
    @Published var appIcon: NSImage?
    // Final warning
    @Published var warningAction = "Sleep"
    @Published var warningSource: String?
    @Published var remaining = 60
    @Published var preview = false
    @Published var warningVisible = false
    @Published var snoozeMinutes = 15
    // Custom duration and end-time form
    @Published var timerIsAwake = false
    @Published var timerUsesDate = false
    @Published var durationMinutes = 30
    @Published var targetDate = Date().addingTimeInterval(1800)
    @Published var timerAction = "sleep"

    /// The page the menu bar panel shows; nil for its main page.
    @Published var panelPage: PanelPage?

    private var settingsWindow: NSWindow?
    private var warningWindows: [NSWindow] = []
    private var timerWindow: NSWindow?
    let panel = PanelController()
    private var previewDeadline: Date?
    private var previewClock: Timer?
    let verification = CommandLine.arguments.contains("--verify-ui")

    // MARK: Reading the snapshot

    var settings: JSON { snapshot.object("settings") ?? [:] }
    var session: JSON { snapshot.object("session") ?? [:] }
    func setting(_ key: String) -> Any? { settings.value(key) }
    func bool(_ key: String) -> Bool { settings.bool(key) }
    func int(_ key: String, _ fallback: Int) -> Int { settings.int(key) ?? fallback }
    func string(_ key: String) -> String? { settings.string(key) }
    var actions: [String] { snapshot.value("actions") as? [String] ?? ["sleep"] }
    var agents: [AgentRow] { snapshot.objects("agents.sessions").map(AgentRow.init) }
    var links: [AgentLink] { snapshot.objects("agentLinks").map(AgentLink.init) }

    /// The engine's clock now: its time at the last snapshot plus what has passed since.
    func engineNow(_ date: Date = Date()) -> Int {
        (snapshot.int("now") ?? 0) + Int(date.timeIntervalSince(receivedAt))
    }
    /// Seconds left until an engine deadline.
    func left(_ part: JSON?, _ now: Date = Date()) -> Int {
        if let deadline = part?.int("deadline") { return max(0, deadline - engineNow(now)) }
        return max(0, (part?.int("remaining") ?? 0) - Int(now.timeIntervalSince(receivedAt)))
    }
    func awakeLeft(_ now: Date = Date()) -> Int? {
        guard session.int("awakeRemaining") != nil else { return nil }
        if let deadline = session.int("awakeDeadline") { return max(0, deadline - engineNow(now)) }
        return max(0, (session.int("awakeRemaining") ?? 0) - Int(now.timeIntervalSince(receivedAt)))
    }
    /// The wall-clock time of an engine time.
    func wallClock(_ engineSeconds: Int) -> Date { Date().addingTimeInterval(Double(engineSeconds - engineNow())) }
    func workingSeconds(_ row: AgentRow, _ now: Date = Date()) -> Int {
        row.workingSeconds + (row.state == "working" ? max(0, engineNow(now) - row.stateSince) : 0)
    }

    // MARK: Pipe

    func start() {
        installApplicationMenu()
        // Only inherited pipes are used. Closing the parent ends the companion.
        DispatchQueue.global(qos: .utility).async { [weak self] in
            while let line = readLine() {
                guard let data = line.data(using: .utf8),
                      let message = try? JSONSerialization.jsonObject(with: data) as? JSON
                else { continue }
                DispatchQueue.main.async { self?.receive(message) }
            }
            DispatchQueue.main.async { NSApp.terminate(nil) }
        }
    }

    func send(_ command: String, _ extra: [String: Any] = [:]) {
        guard !verification else { return }
        var message = extra
        message["command"] = command
        guard let data = try? JSONSerialization.data(withJSONObject: message),
              var line = String(data: data, encoding: .utf8) else { return }
        line += "\n"
        FileHandle.standardOutput.write(Data(line.utf8))
    }

    /// Changes one setting in the engine's store. `nil` clears an optional setting.
    func set(_ key: String, _ value: Any?) {
        send("set", ["key": key, "value": value ?? NSNull()])
    }

    func receive(_ message: JSON) {
        let type = message["type"] as? String ?? ""
        let command = message["command"] as? String
        if var next = message["snapshot"] as? JSON {
            // Streamed updates leave out parts that read files; keep the last ones.
            for key in ["agentLinks", "agentSkills", "agentConnections"] where next[key] == nil {
                next[key] = snapshot[key]
            }
            let theme = next.string("settings.theme")
            if theme != settings.string("theme") { applyAppearance(theme ?? "system") }
            snapshot = next
            receivedAt = Date()
            if appIcon == nil, let path = next.string("iconPath") { appIcon = NSImage(contentsOfFile: path) }
            panel.refit()
        }
        if let theme = message["theme"] as? String { applyAppearance(theme) }
        if let error = message["error"] as? String {
            notice = error
            if command == "connect-preview" { pendingChange = nil }
        } else if command != nil {
            notice = ""
        }
        if let result = message["result"] as? JSON { handle(result, for: command) }
        switch type {
        case "open":
            if let id = message["page"] as? String, let named = Page(engineId: id) { page = named }
            switch message["view"] as? String {
            case "about": page = .about; showSettings()
            case "help": page = .guide; showSettings()
            case "agents": page = .agents; showSettings()
            case "awakeDuration": showTimer(awake: true, date: false)
            case "awakeTime": showTimer(awake: true, date: true)
            case "timerDuration": showTimer(awake: false, date: false)
            case "timerTime": showTimer(awake: false, date: true)
            default:
                if message["page"] == nil || message["page"] is NSNull { page = page ?? .overview }
                showSettings()
            }
        case "panel":
            panel.toggle(model: self, anchor: message["anchor"] as? JSON)
        case "countdown":
            readWarningPreferences(message)
            if let countdown = message["countdown"] as? JSON {
                let newlyShown = !warningVisible || preview
                preview = false
                previewDeadline = nil
                warningAction = countdown["action"] as? String ?? "Sleep"
                warningSource = countdown["source"] as? String
                remaining = countdown["remaining"] as? Int ?? 0
                showWarning(allDisplays: message["allDisplays"] as? Bool ?? false)
                if newlyShown, message["sound"] as? Bool ?? true, !verification {
                    NSSound(named: NSSound.Name("Glass"))?.play()
                }
            } else if !preview { dismissWarning() }
        case "preview":
            readWarningPreferences(message)
            // An actual countdown always takes priority over a preview.
            if !warningVisible || preview {
                preview = true
                warningAction = message["action"] as? String ?? "Sleep"
                warningSource = nil
                remaining = 60
                previewDeadline = Date().addingTimeInterval(60)
                startPreviewClock()
                showWarning(allDisplays: message["allDisplays"] as? Bool ?? false)
            }
        default: break
        }
    }

    private func readWarningPreferences(_ message: JSON) {
        if let minutes = message["snoozeMinutes"] as? Int { snoozeMinutes = minutes }
    }

    /// Data a command returned: a config change to confirm, a config to copy, or a message.
    private func handle(_ result: JSON, for command: String?) {
        switch command {
        case "connect-preview":
            pendingChange = PendingChange(result)
        case "connect-apply":
            notice = result.string("message") ?? ""
        case "copy-config":
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(result.string("config") ?? "", forType: .string)
            notice = "MCP config copied. Paste it into your MCP client's settings."
        default: break
        }
    }

    // MARK: Search

    /// Every setting's title and description, so search jumps to the setting itself.
    var searchIndex: [SearchEntry] {
        Page.allCases.flatMap { page in
            [SearchEntry(page: page, title: page.rawValue, detail: page.keywords)]
                + SettingsCatalog.rows(for: page, model: self).map { SearchEntry(page: page, title: $0.title, detail: $0.detail ?? "") }
        }
    }

    func searchResults(_ query: String) -> [SearchEntry] {
        let words = query.split(separator: " ").map(String.init)
        guard !words.isEmpty else { return [] }
        return searchIndex.filter { entry in
            words.allSatisfy { word in
                "\(entry.title) \(entry.detail) \(entry.page.rawValue)".localizedCaseInsensitiveContains(word)
            }
        }
        .sorted { lhs, rhs in
            let l = words.allSatisfy { lhs.title.localizedCaseInsensitiveContains($0) }
            let r = words.allSatisfy { rhs.title.localizedCaseInsensitiveContains($0) }
            return l && !r
        }
    }

    func reveal(_ entry: SearchEntry) {
        page = entry.page
        revealed = entry.title == entry.page.rawValue ? nil : entry.title
        search = ""
    }

    // MARK: Windows

    private func applyAppearance(_ theme: String) {
        switch theme {
        case "light": NSApp.appearance = NSAppearance(named: .aqua)
        case "dark": NSApp.appearance = NSAppearance(named: .darkAqua)
        default: NSApp.appearance = nil
        }
    }

    private func installApplicationMenu() {
        let menu = NSMenu()
        let application = NSMenu()
        let appItem = NSMenuItem()
        appItem.submenu = application
        menu.addItem(appItem)
        let settings = NSMenuItem(title: "Settings…", action: #selector(openSettingsFromMenu), keyEquivalent: ",")
        settings.target = self
        application.addItem(settings)
        application.addItem(.separator())
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

    /// Shows a page of the menu bar panel in place, or its main page for nil, keeping the
    /// panel's top edge under the icon.
    func showPanelPage(_ page: PanelPage?) {
        panelPage = page
        panel.refit()
    }

    @objc private func quitDoze() { send("quit") }
    @objc private func openSettingsFromMenu() { showSettings() }
    func openSettings(_ page: Page) {
        self.page = page
        showSettings()
    }

    private func makeWindow<Content: View>(_ title: String, size: NSSize, content: Content) -> NSWindow {
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

    func showSettings() {
        panel.close()
        if settingsWindow == nil {
            let window = makeWindow("Doze Settings", size: NSSize(width: 860, height: 640), content: SettingsView(model: self))
            window.minSize = NSSize(width: 740, height: 500)
            window.delegate = self
            settingsWindow = window
        }
        if let window = settingsWindow { activate(window) }
    }

    /// One warning window on the main display, plus one per other display when Show on every
    /// display is on. Every copy offers the same buttons.
    private func showWarning(allDisplays: Bool) {
        let screens = allDisplays ? NSScreen.screens : [NSScreen.main ?? NSScreen.screens.first].compactMap { $0 }
        while warningWindows.count < max(1, screens.count) {
            let window = makeWindow("Doze · Power countdown", size: NSSize(width: 520, height: 440), content: WarningView(model: self))
            window.styleMask.remove([.resizable, .miniaturizable])
            window.level = .floating
            window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
            window.delegate = self
            warningWindows.append(window)
        }
        while warningWindows.count > max(1, screens.count) { warningWindows.removeLast().orderOut(nil) }
        for (window, screen) in zip(warningWindows, screens) {
            let frame = screen.visibleFrame
            window.setFrameOrigin(NSPoint(x: frame.midX - window.frame.width / 2, y: frame.midY - window.frame.height / 2))
            if !warningVisible, !verification { window.orderFrontRegardless() }
        }
        if !warningVisible, !verification, let first = warningWindows.first {
            NSApp.activate(ignoringOtherApps: true)
            first.makeKey()
        }
        warningVisible = true
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        if warningWindows.contains(sender) { cancelWarning(); return false }
        return true
    }

    func windowWillClose(_ notification: Notification) {
        // A closed Settings window keeps no views alive.
        if (notification.object as? NSWindow) === settingsWindow {
            settingsWindow?.contentView = nil
            settingsWindow = nil
        }
    }

    private func dismissWarning() {
        for window in warningWindows { window.orderOut(nil) }
        warningVisible = false
        previewDeadline = nil
        previewClock?.invalidate()
        previewClock = nil
        preview = false
    }

    private func startPreviewClock() {
        previewClock?.invalidate()
        previewClock = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
            DispatchQueue.main.async {
                guard let self, let deadline = self.previewDeadline else { return }
                self.remaining = max(0, Int(ceil(deadline.timeIntervalSinceNow)))
                if self.remaining == 0 { self.previewClock?.invalidate(); self.previewClock = nil }
            }
        }
    }

    func cancelWarning() {
        if !preview { send("cancel") }
        dismissWarning()
    }

    func snoozeWarning() {
        if !preview { send("snooze") }
        dismissWarning()
    }

    func stayAwakeFromWarning() {
        if !preview { send("stay-awake") }
        dismissWarning()
    }

    func showTimer(awake: Bool, date: Bool) {
        panel.close()
        timerIsAwake = awake
        timerUsesDate = date
        let remembered = awake && bool("rememberLastCustomDuration") ? settings.int("lastCustomAwakeMinutes") : nil
        durationMinutes = remembered ?? int(awake ? "defaultAwakeMinutes" : "defaultTimerMinutes", 30)
        targetDate = Date().addingTimeInterval(Double(durationMinutes * 60))
        timerAction = session.string("selectedAction") ?? string("defaultAction") ?? "sleep"
        notice = ""
        if timerWindow == nil {
            timerWindow = makeWindow("Doze · Custom session", size: NSSize(width: 480, height: 360), content: TimerView(model: self))
            timerWindow?.styleMask.remove([.resizable, .miniaturizable])
        }
        if let window = timerWindow {
            if let content = window.contentView { window.setContentSize(content.fittingSize) }
            activate(window)
        }
    }

    func startTimer() {
        guard timerUsesDate || (1...10080).contains(durationMinutes) else {
            notice = "Choose a duration between 1 minute and 7 days."; return
        }
        let seconds = timerUsesDate ? Int(ceil(targetDate.timeIntervalSinceNow)) : durationMinutes * 60
        guard (60...604800).contains(seconds) else { notice = "Choose a duration between 1 minute and 7 days."; return }
        send(timerIsAwake ? "awake" : "timer",
             timerIsAwake ? ["seconds": seconds] : ["seconds": seconds, "action": timerAction])
        if timerIsAwake, !timerUsesDate, bool("rememberLastCustomDuration") {
            set("lastCustomAwakeMinutes", durationMinutes)
        }
        timerWindow?.orderOut(nil)
    }

    func closeTimer() { timerWindow?.orderOut(nil) }

    func openData() {
        guard let path = snapshot.string("settingsPath") else { return }
        NSWorkspace.shared.activateFileViewerSelecting([URL(fileURLWithPath: path)])
    }

    func openLink(_ url: String) {
        if let url = URL(string: url) { NSWorkspace.shared.open(url) }
    }

    /// Settings › Advanced › Export: the engine writes the report to the chosen file.
    func exportDiagnostics() {
        let save = NSSavePanel()
        save.nameFieldStringValue = "Doze diagnostics \(Date().formatted(.iso8601.year().month().day())).json"
        save.allowedContentTypes = [.json]
        if save.runModal() == .OK, let url = save.url { send("export-diagnostics", ["path": url.path]) }
    }

    // MARK: Command-line tool

    var cliPath: String { snapshot.string("cliPath") ?? "/Applications/Doze.app/Contents/MacOS/doze" }

    var cliInstalled: Bool {
        (try? FileManager.default.destinationOfSymbolicLink(atPath: "/usr/local/bin/doze")) == cliPath
    }

    var cliInstallCommand: String {
        "sudo mkdir -p /usr/local/bin && sudo ln -sf " + shellQuoted(cliPath) + " /usr/local/bin/doze"
    }

    /// Links /usr/local/bin/doze to the app, after the standard administrator prompt.
    func installCommandLineTool() {
        let script = "do shell script \"mkdir -p /usr/local/bin && ln -sf \" & quoted form of \""
            + cliPath.replacingOccurrences(of: "\"", with: "\\\"")
            + "\" & \" /usr/local/bin/doze\" with administrator privileges"
        var error: NSDictionary?
        NSAppleScript(source: script)?.executeAndReturnError(&error)
        if let error, (error[NSAppleScript.errorNumber] as? Int) != -128 {
            notice = "Couldn't install the command-line tool. Run this in Terminal instead: " + cliInstallCommand
        } else if error == nil {
            notice = "Installed. Open a new Terminal window and run doze help."
        }
        objectWillChange.send()
    }
}

func shellQuoted(_ path: String) -> String { "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'" }

@main
enum DozeNativeUI {
    @MainActor static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let ui = NativeUI()
        if let index = CommandLine.arguments.firstIndex(of: "--render-ui"), index + 1 < CommandLine.arguments.count {
            do {
                try Verification.render(ui, to: URL(fileURLWithPath: CommandLine.arguments[index + 1]))
            } catch {
                FileHandle.standardError.write(Data("\(error)\n".utf8))
                exit(1)
            }
            return
        }
        if CommandLine.arguments.contains("--verify-ui") {
            do {
                try Verification.verify(ui)
                print("Verified: the menu bar panel in its idle, working and countdown states and its Countdown, Quick Settings and Help & About pages, nine Settings pages in light and dark, search by setting, and preview snooze.")
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
