import AppKit
import SwiftUI

private extension View {
    @ViewBuilder func navigationGlass() -> some View {
        if #available(macOS 26.0, *) {
            self.glassEffect(.regular, in: .rect(cornerRadius: 16))
        } else {
            self.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
        }
    }
}

/// The floating final warning, unchanged in behaviour: Cancel, Snooze and Stay Awake.
struct WarningView: View {
    @ObservedObject var model: NativeUI
    var body: some View {
        VStack(spacing: 12) {
            DozeGlyph(state: "countdown", size: 24).foregroundStyle(Color.dozeAccent)
            if let source = model.warningSource {
                Text(countdownSourceLabel(source)).foregroundStyle(.secondary)
            }
            Text("\(model.warningAction) in").font(.title2.weight(.semibold))
            Text(clockText(model.remaining))
                .font(.system(size: 64, weight: .semibold, design: .rounded))
                .monospacedDigit().accessibilityAddTraits(.updatesFrequently)
            Text(model.preview ? "Preview only — no power action is scheduled." : "Cancel the action or snooze for \(model.snoozeMinutes) minutes.")
                .foregroundStyle(.secondary)
            HStack {
                Button("Snooze \(model.snoozeMinutes) minutes", action: model.snoozeWarning)
                Button("Cancel", action: model.cancelWarning).keyboardShortcut(.cancelAction)
                if !model.preview { Button("Stay Awake", action: model.stayAwakeFromWarning) }
            }.padding(12).navigationGlass()
        }
        .padding(24).frame(maxWidth: .infinity, maxHeight: .infinity)
        .tint(.dozeAccent)
    }
}

/// Custom Keep Awake or Power Timer: a duration or an end time.
struct TimerView: View {
    @ObservedObject var model: NativeUI
    private let presets = [15, 30, 60, 120, 240, 480]
    private var startTitle: String { model.timerIsAwake ? "Keep Awake" : "Start Timer" }
    private func presetTitle(_ minutes: Int) -> String { minutes < 60 ? "\(minutes)m" : "\(minutes / 60)h" }

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
                    ForEach(model.actions, id: \.self) { Text(actionLabel($0)).tag($0) }
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
                        Button(presetTitle(minutes)) { model.durationMinutes = minutes }.controlSize(.small)
                            .accessibilityLabel("Set duration to " + durationLabel(minutes: minutes))
                    }
                }
            }
            Text(endText).font(.callout).foregroundStyle(.secondary)
            if !model.notice.isEmpty { Text(model.notice).foregroundStyle(.red) }
            HStack {
                Button("Cancel", action: model.closeTimer).keyboardShortcut(.cancelAction)
                Spacer()
                Button(startTitle, action: model.startTimer).keyboardShortcut(.defaultAction)
            }
        }
        .padding(24)
        .frame(width: 448)
        .navigationGlass()
        .padding(16)
        .tint(.dozeAccent)
    }
}

/// `--verify-ui` builds every window offscreen with sample data; `--render-ui` also writes
/// PNGs for visual review. Neither contacts the engine or saves settings.
@MainActor
enum Verification {
    static func sample(_ model: NativeUI, countdown: Bool = false) {
        var session: JSON = [
            "awake": true, "awakeRemaining": 2520, "awakeDeadline": 3120, "whileAudio": false, "audioActive": false,
            "holdingAwake": true, "playbackEnabled": true, "playbackPhase": "playing", "selectedAction": "sleep",
            "timer": ["action": "sleep", "remaining": 3900, "deadline": 4500] as JSON,
        ]
        if countdown {
            session["countdown"] = ["action": "sleep", "remaining": 287, "deadline": 887, "source": "agents", "length": 300] as JSON
        }
        let agents: JSON = [
            "enabled": true, "askBeforeNew": true, "defaultCompletion": "sleep", "trusted": ["claude-code"],
            "processTools": [["id": "cursor", "detect": true, "keepAwake": false] as JSON], "clients": [JSON](),
        ]
        let settings: JSON = [
            "agents": agents, "theme": "system", "launchAtStartup": true, "defaultAction": "sleep", "playbackAction": "sleep",
            "idleSeconds": 600, "countdownSeconds": 300, "defaultAwakeMinutes": 30, "defaultTimerMinutes": 30,
            "menuBarTime": true, "menuBarAgentCount": true, "iconClickOpens": "panel", "batteryFloorPercent": 15,
            "rememberLastCustomDuration": true, "snoozeMinutes": 15, "warningSound": true, "warningAllDisplays": false,
            "notifyAgentApproval": true, "notifyAgentsFinished": true, "notifyAgentStalled": true,
            "notifyKeepAwakeEnded": false, "mcpServerEnabled": true, "allowDisplaySleep": false, "logging": false,
        ]
        let sessions: [JSON] = [
            ["id": "s1", "name": "Claude Code", "monogram": "CC", "project": "doze-app", "task": "Running the test suite",
             "state": "working", "source": "hooks", "startedAt": 0, "stateSince": 0, "workingSeconds": 120, "holdsAssertion": true],
            ["id": "s2", "name": "OpenCode", "monogram": "OC", "project": "api", "task": "Refactor",
             "state": "idle", "source": "plugin", "startedAt": 0, "stateSince": 420, "workingSeconds": 300, "holdsAssertion": false],
            ["id": "s4", "name": "Cursor", "monogram": "Cu", "state": "idle", "source": "process", "startedAt": 0, "stateSince": 0,
             "workingSeconds": 0, "holdsAssertion": false],
            ["id": "s3", "name": "Codex", "monogram": "Cx", "project": "website", "task": "Migrating the database",
             "state": "needsApproval", "source": "hooks", "startedAt": 590, "stateSince": 590, "workingSeconds": 0, "holdsAssertion": false],
        ]
        let links: [JSON] = [
            ["id": "claude-code", "name": "Claude Code", "monogram": "CC", "method": "Hooks", "installed": true, "connected": true],
            ["id": "codex", "name": "Codex", "monogram": "Cx", "method": "Hooks", "installed": true, "connected": true],
            ["id": "opencode", "name": "OpenCode", "monogram": "OC", "method": "Plugin", "installed": true, "connected": true],
            ["id": "gemini-cli", "name": "Gemini CLI", "monogram": "Ge", "method": "Hooks", "installed": false, "connected": false],
        ]
        var snapshot: JSON = [
            "now": 600, "settings": settings, "session": session,
            "agents": ["working": 1, "idle": 1, "pending": 1, "sessions": sessions] as JSON,
            "agentLinks": links,
            "actions": ["sleep", "shutdown", "lock", "displayOff"],
            "audioSupported": true, "startupSupported": true, "lidClosedSupported": false,
            "status": "Keeping awake · agents working",
            "statusDetail": "For Claude Code",
            "version": "0.2.0", "settingsPath": "/tmp/doze/settings.json", "mcpAddress": "127.0.0.1:53817",
            "cliPath": "/Applications/Doze.app/Contents/MacOS/doze",
            "battery": ["percent": 82, "onBattery": true] as JSON,
            "assertions": ["PreventUserIdleDisplaySleep · “Doze keep awake”"],
            "links": [["title": "getdoze.app", "url": "https://getdoze.app"], ["title": "MCP guide", "url": "https://getdoze.app/mcp"]],
        ]
        snapshot["statusShort"] = countdown ? "All agents finished" : "Keeping awake"
        snapshot["iconState"] = countdown ? "countdown" : "attention"
        model.snapshot = snapshot
        model.receivedAt = Date()
    }

    private static func failure(_ message: String) -> NSError {
        NSError(domain: "Doze.NativeUI", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }

    static func verify(_ model: NativeUI) throws {
        sample(model)
        for appearance in [NSAppearance.Name.aqua, .darkAqua] {
            for countdown in [false, true] {
                sample(model, countdown: countdown)
                let panel = NSHostingView(rootView: PanelView(model: model))
                panel.appearance = NSAppearance(named: appearance)
                panel.layoutSubtreeIfNeeded()
                guard panel.fittingSize.height > 300, abs(panel.fittingSize.width - PanelController.width) < 1 else {
                    throw failure("The panel did not lay out at 368 points (\(panel.fittingSize)).")
                }
            }
            sample(model)
            for page in Page.allCases {
                model.page = page
                let host = NSHostingView(rootView: SettingsView(model: model))
                host.appearance = NSAppearance(named: appearance)
                host.frame = NSRect(x: 0, y: 0, width: 940, height: 720)
                host.layoutSubtreeIfNeeded()
                guard host.fittingSize.width > 0 else { throw failure("Unable to lay out \(page.rawValue).") }
                guard !SettingsCatalog.rows(for: page, model: model).isEmpty else { throw failure("\(page.rawValue) has no settings.") }
            }
        }
        let searches: [(String, Page, String)] = [
            ("snooze", Page.session, "Snooze length"), ("battery", Page.general, "Stop keeping awake below"),
            ("checking in", Page.agents, "If an agent stops checking in"), ("sound", Page.notifications, "Play a sound when it appears"),
            ("cursor", Page.agents, "Cursor"), ("assertions", Page.advanced, "How Doze keeps your Mac awake"),
        ]
        for (query, page, title) in searches {
            guard model.searchResults(query).contains(where: { $0.page == page && $0.title == title }) else {
                throw failure("Search for \"\(query)\" did not find \(title).")
            }
        }
        let general = SettingsCatalog.rows(for: .general, model: model).map(\.title)
        guard !general.contains("Stay awake with the lid closed") else { throw failure("The lid switch is offered without support.") }
        let about = SettingsCatalog.rows(for: .about, model: model).map(\.title) + (model.snapshot.objects("links").compactMap { $0.string("title") })
        guard !about.contains(where: { $0.contains("GitHub") || $0.contains("Source") }) else { throw failure("About links to source code.") }
        // Preview buttons must never submit engine commands.
        model.receive(["type": "preview", "action": "Sleep"])
        guard model.warningVisible, model.preview else { throw failure("Preview did not show.") }
        model.snoozeWarning()
        guard !model.warningVisible else { throw failure("Preview did not dismiss.") }
    }

    static func render(_ model: NativeUI, to folder: URL) throws {
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        func capture(_ view: some View, size: NSSize, name: String, appearance: NSAppearance.Name, transparent: Bool = false) throws {
            let window = NSWindow(contentRect: NSRect(origin: NSPoint(x: -20000, y: -20000), size: size),
                                  styleMask: [.titled, .fullSizeContentView], backing: .buffered, defer: false)
            window.appearance = NSAppearance(named: appearance)
            window.titlebarAppearsTransparent = true
            let host = NSHostingView(rootView: view)
            window.contentView = host
            window.orderFrontRegardless()
            for _ in 0..<3 { RunLoop.main.run(until: Date().addingTimeInterval(0.15)) }
            host.layoutSubtreeIfNeeded()
            // Render the frame view's layer tree; cacheDisplay omits layer-hosted SwiftUI text.
            guard let root = window.contentView?.superview, let layer = root.layer else { throw failure("Could not render \(name).") }
            let scale = window.backingScaleFactor
            guard let context = CGContext(data: nil, width: Int(root.bounds.width * scale), height: Int(root.bounds.height * scale),
                                          bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { throw failure("Could not render \(name).") }
            context.scaleBy(x: scale, y: scale)
            NSAppearance(named: appearance)?.performAsCurrentDrawingAppearance {
                context.setFillColor(transparent ? NSColor.gray.cgColor : NSColor.windowBackgroundColor.cgColor)
            }
            context.fill(root.bounds)
            layer.render(in: context)
            window.orderOut(nil)
            guard let rendered = context.makeImage(),
                  let png = NSBitmapImageRep(cgImage: rendered).representation(using: .png, properties: [:]) else {
                throw failure("Could not encode \(name).")
            }
            try png.write(to: folder.appendingPathComponent("\(name)-\(appearance == .aqua ? "light" : "dark").png"))
        }
        for appearance in [NSAppearance.Name.aqua, .darkAqua] {
            for countdown in [false, true] {
                sample(model, countdown: countdown)
                let host = NSHostingView(rootView: PanelView(model: model))
                host.layoutSubtreeIfNeeded()
                try capture(PanelView(model: model).padding(20), size: NSSize(width: 408, height: host.fittingSize.height + 40),
                            name: countdown ? "panel-countdown" : "panel", appearance: appearance, transparent: true)
            }
            sample(model)
            for page in Page.allCases {
                model.page = page
                try capture(SettingsView(model: model), size: NSSize(width: 940, height: 720),
                            name: "settings-\(page.rawValue.lowercased().replacingOccurrences(of: " ", with: "-"))", appearance: appearance)
            }
            for (isPreview, name) in [(true, "countdown-preview"), (false, "countdown")] {
                model.preview = isPreview
                model.warningAction = "Sleep"
                model.remaining = 287
                try capture(WarningView(model: model), size: NSSize(width: 520, height: 440), name: name, appearance: appearance)
            }
            for (awake, date) in [(true, false), (false, true)] {
                model.timerIsAwake = awake
                model.timerUsesDate = date
                try capture(TimerView(model: model), size: NSSize(width: 480, height: 280),
                            name: "timer-\(awake ? "awake" : "power")-\(date ? "time" : "duration")", appearance: appearance)
            }
        }
    }
}
