import AppKit
import SwiftUI

extension Dictionary where Key == String, Value == Any {
    func double(_ path: String) -> Double? { (value(path) as? NSNumber)?.doubleValue }
}

/// The panel opens under the menu bar icon. The status item belongs to the engine process, so
/// the engine sends the icon's rectangle (physical pixels, top-left origin) with each click.
final class PanelWindow: NSPanel {
    override var canBecomeKey: Bool { true }
    override func cancelOperation(_ sender: Any?) { (delegate as? PanelController)?.back() }
}

/// The rows at the bottom of the panel open their page in place, as Control Center's modules
/// do, rather than leaving the panel for a menu.
enum PanelPage: String, CaseIterable {
    case countdown = "Countdown"
    case quick = "Quick Settings"
    case support = "Help & About"
}

@MainActor
final class PanelController: NSObject, NSWindowDelegate {
    private var window: PanelWindow?
    private var host: NSHostingView<AnyView>?
    private weak var model: NativeUI?
    private var anchor: JSON?
    private var hiddenAt = Date.distantPast
    private var closing = false
    private var clicks: Any?
    static let width: CGFloat = 368

    var isOpen: Bool { window?.isVisible == true }

    /// Opens at the icon, or closes when already open (a second click on the icon).
    func toggle(model: NativeUI, anchor: JSON?) {
        if isOpen { close(); return }
        // The click that took focus from an open panel already closed it.
        if Date().timeIntervalSince(hiddenAt) < 0.35 { return }
        self.anchor = anchor
        self.model = model
        // Every opening starts on the main page.
        model.panelPage = nil
        let window = self.window ?? makeWindow()
        self.window = window
        let host = NSHostingView(rootView: AnyView(PanelView(model: model)))
        // The glass's backdrop reads as opaque to the window server, which would shape the
        // shadow as the whole rectangle and leave dark square corners. Clipping the content to
        // the panel's rounded shape gives the shadow that shape.
        host.wantsLayer = true
        host.layer?.cornerRadius = 18
        host.layer?.cornerCurve = .continuous
        host.layer?.masksToBounds = true
        window.contentView = host
        self.host = host
        refit()
        // Like the system's menu bar extras: in place under the icon with a short fade.
        let reduceMotion = NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        window.alphaValue = reduceMotion ? 1 : 0
        window.makeKeyAndOrderFront(nil)
        if !reduceMotion {
            NSAnimationContext.runAnimationGroup { context in
                context.duration = 0.12
                window.animator().alphaValue = 1
            }
        }
        watchClicks()
    }

    /// Fades out the way a dismissed menu does, then hides; hidden, the panel keeps no SwiftUI
    /// views alive.
    func close() {
        guard let window, window.isVisible, !closing else { return }
        closing = true
        hiddenAt = Date()
        stopWatchingClicks()
        let hide: @MainActor () -> Void = { [weak self] in
            window.orderOut(nil)
            window.alphaValue = 1
            window.contentView = nil
            self?.host = nil
            self?.closing = false
        }
        if NSWorkspace.shared.accessibilityDisplayShouldReduceMotion { hide(); return }
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = 0.15
            window.animator().alphaValue = 0
        }, completionHandler: {
            Task { @MainActor in hide() }
        })
    }

    /// Escape returns from a page to the main page, then closes the panel.
    func back() {
        if let model, model.panelPage != nil { model.showPanelPage(nil) } else { close() }
    }

    func windowDidResignKey(_ notification: Notification) { close() }

    /// Like Control Center, the panel closes on any click outside it. Resigning key covers that
    /// only while this app holds the key window, and it never activates, so while the panel is
    /// open a global monitor also watches for presses in other apps (the engine's menu bar icon
    /// included). Mouse monitoring needs no Accessibility permission.
    private func watchClicks() {
        guard clicks == nil else { return }
        clicks = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]) { [weak self] _ in
            guard let self else { return }
            Task { @MainActor in self.close() }
        }
    }

    private func stopWatchingClicks() {
        guard let clicks else { return }
        NSEvent.removeMonitor(clicks)
        self.clicks = nil
    }

    private func makeWindow() -> PanelWindow {
        let window = PanelWindow(contentRect: NSRect(x: 0, y: 0, width: Self.width, height: 400),
                                 styleMask: [.borderless, .nonactivatingPanel, .fullSizeContentView],
                                 backing: .buffered, defer: true)
        window.isFloatingPanel = true
        window.level = .statusBar
        window.backgroundColor = .clear
        window.isOpaque = false
        window.hasShadow = true
        window.hidesOnDeactivate = false
        window.isReleasedWhenClosed = false
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .transient]
        window.delegate = self
        window.setAccessibilityLabel("Doze")
        return window
    }

    /// Sizes the panel to its content and keeps its top edge under the icon.
    func refit() {
        guard let window, let host, !closing else { return }
        host.layoutSubtreeIfNeeded()
        let size = NSSize(width: Self.width, height: host.fittingSize.height)
        window.setFrame(NSRect(origin: origin(for: size), size: size), display: true)
        window.invalidateShadow()
    }

    private func origin(for size: NSSize) -> NSPoint {
        let screens = NSScreen.screens
        let primaryHeight = screens.first?.frame.height ?? 900
        guard let anchor, let x = anchor.double("x"), let y = anchor.double("y") else {
            let visible = (NSScreen.main ?? screens.first)?.visibleFrame ?? .zero
            return NSPoint(x: visible.maxX - size.width - 8, y: visible.maxY - size.height - 6)
        }
        let width = CGFloat(anchor.double("width") ?? 0)
        let height = CGFloat(anchor.double("height") ?? 0)
        let px = CGFloat(x), py = CGFloat(y)
        // Physical pixels to points: the engine's scale, or the screen the point falls on.
        let fallback = screens.first(where: { screen in
            let s = screen.backingScaleFactor
            return screen.frame.contains(NSPoint(x: px / s + 1, y: primaryHeight - py / s - 1))
        })?.backingScaleFactor ?? 2
        let scale = anchor.double("scale").map { CGFloat($0) } ?? fallback
        let iconCenterX = (px + width / 2) / scale
        let iconBottom = primaryHeight - (py + height) / scale
        let screen = screens.first { $0.frame.contains(NSPoint(x: iconCenterX, y: iconBottom + 1)) } ?? NSScreen.main ?? screens.first
        let visible = screen?.visibleFrame ?? .zero
        let left = min(max(iconCenterX - size.width / 2, visible.minX + 8), visible.maxX - size.width - 8)
        let top = min(iconBottom, visible.maxY) - 6
        return NSPoint(x: left, y: max(visible.minY + 8, top - size.height))
    }
}

/// The two status lines, shared by the panel and Overview.
@MainActor
enum PanelText {
    static func detail(_ model: NativeUI, _ now: Date) -> String {
        let session = model.session
        if let countdown = session.object("countdown") {
            return "\(actionLabel(countdown.string("action"))) in \(clockText(model.left(countdown, now)))"
        }
        let working = model.agents.filter(\.holds)
        if !working.isEmpty {
            let since = model.wallClock(working.map(\.startedAt).min() ?? model.engineNow())
            let names = Array(Set(working.map(\.name)))
            let who = names.count == 1 ? names[0] : "\(working.count) agents"
            return "For \(who) · since \(since.formatted(date: .omitted, time: .shortened))"
        }
        if session.bool("awake") {
            guard let left = model.awakeLeft(now) else { return "Until you stop it" }
            return "\(remainingText(left)) left · until \(now.addingTimeInterval(Double(left)).formatted(date: .omitted, time: .shortened))"
        }
        if let timer = session.object("timer") {
            return "\(actionLabel(timer.string("action"))) in \(remainingText(model.left(timer, now)))"
        }
        return model.snapshot.string("statusDetail") ?? "No power action scheduled"
    }
}

private extension View {
    /// Liquid Glass on macOS 26; the regular material before that.
    @ViewBuilder func panelGlass() -> some View {
        if #available(macOS 26.0, *) {
            self.glassEffect(.regular, in: .rect(cornerRadius: 18))
        } else {
            self.background(.regularMaterial, in: RoundedRectangle(cornerRadius: 18, style: .continuous))
        }
    }
}

struct PanelView: View {
    @ObservedObject var model: NativeUI
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var session: JSON { model.session }

    var body: some View {
        TimelineView(.periodic(from: .now, by: 1)) { context in
            VStack(alignment: .leading, spacing: 6) {
                switch model.panelPage {
                case .countdown?: countdownPage(context.date)
                case .quick?: quickPage
                case .support?: supportPage
                case nil:
                    status(context.date)
                    ForEach(model.agents.filter { $0.state == "needsApproval" }) { approval($0) }
                    agentsSection(context.date)
                    keepAwakeSection
                    timerSection(context.date)
                    menus
                    footer()
                }
            }
            .padding(12)
            .frame(width: PanelController.width, alignment: .topLeading)
            .tint(.dozeAccent)
            .panelGlass()
        }
    }

    // MARK: Status

    @ViewBuilder private func status(_ now: Date) -> some View {
        if let countdown = session.object("countdown") {
            let left = model.left(countdown, now)
            let length = max(1, countdown.int("length") ?? 300)
            VStack(alignment: .leading, spacing: 10) {
                Text(countdownSourceLabel(countdown.string("source"))).font(.system(size: 11, weight: .semibold)).foregroundStyle(.secondary)
                Text("\(actionLabel(countdown.string("action"))) in \(clockText(left))")
                    .font(.system(size: 30, weight: .semibold)).monospacedDigit()
                    .accessibilityAddTraits(.updatesFrequently)
                ProgressView(value: Double(left), total: Double(length)).progressViewStyle(.linear).tint(.dozeProminent)
                    .accessibilityLabel("Time until \(actionLabel(countdown.string("action")))")
                HStack(spacing: 6) {
                    pill("Snooze \(model.int("snoozeMinutes", 15)) min") { model.send("snooze") }
                    pill("Cancel") { model.send("cancel") }.accessibilityLabel("Cancel the action")
                    pill("Stay Awake", prominent: true) { model.send("stay-awake") }
                }
            }
            .padding(14)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Color.dozeSection, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        } else {
            HStack(spacing: 12) {
                StatusTile(state: model.snapshot.string("iconState") ?? "normal")
                VStack(alignment: .leading, spacing: 1) {
                    Text(model.snapshot.string("statusShort") ?? "Normal sleep allowed").font(.system(size: 13, weight: .semibold))
                    Text(PanelText.detail(model, now)).font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .accessibilityElement(children: .combine)
                Spacer(minLength: 0)
                if holding {
                    Button("Stop", action: stopEverything).controlSize(.small).accessibilityLabel("Stop keeping awake")
                }
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
            .background(Color.dozeSection, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        }
    }

    private var holding: Bool {
        session.bool("awake") || session.bool("whileAudio") || model.agents.contains(where: \.holds)
    }

    private func stopEverything() {
        if session.bool("awake") || session.bool("whileAudio") { model.send("stop-awake") }
        for agent in model.agents where agent.holds { model.send("agent-release", ["id": agent.id]) }
    }

    private func pill(_ title: String, prominent: Bool = false, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Text(title).font(.system(size: 12, weight: .medium)).frame(maxWidth: .infinity).frame(height: 28)
                .foregroundStyle(prominent ? Color.white : Color.primary)
                .background(prominent ? Color.dozeProminent : Color.primary.opacity(0.08), in: Capsule())
        }
        .buttonStyle(.plain)
    }

    // MARK: Approval

    private func approval(_ agent: AgentRow) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 10) {
                Monogram(agent.monogram)
                VStack(alignment: .leading, spacing: 1) {
                    Text("\(agent.name) wants to keep your Mac awake").font(.system(size: 13, weight: .semibold))
                    Text([agent.project, agent.task.map { "“\($0)”" }].compactMap { $0 }.joined(separator: " · "))
                        .font(.system(size: 11)).foregroundStyle(.secondary)
                    Text("When done: " + actionLabel(agent.completionAction ?? model.string("agents.defaultCompletion")))
                        .font(.system(size: 11)).foregroundStyle(.secondary)
                }
            }
            HStack(spacing: 6) {
                Spacer()
                Button("Deny") { model.send("agent-deny", ["id": agent.id]) }.controlSize(.small)
                    .accessibilityLabel("Deny \(agent.name)")
                Button("Allow") { model.send("agent-allow", ["id": agent.id]) }.controlSize(.small)
                    .buttonStyle(.borderedProminent).accessibilityLabel("Allow \(agent.name)")
            }
        }
        .padding(.horizontal, 12).padding(.vertical, 10)
        .background(Color.dozeSection, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .overlay(RoundedRectangle(cornerRadius: 12, style: .continuous).strokeBorder(Color.dozeAccent.opacity(0.4)))
        .accessibilityElement(children: .contain)
        .accessibilityLabel("\(agent.name) wants to keep your Mac awake")
    }

    // MARK: Agents

    @ViewBuilder private func agentsSection(_ now: Date) -> some View {
        let rows = model.agents.filter { $0.state != "needsApproval" }
        let working = rows.filter { $0.state == "working" }.count
        let idle = rows.filter { $0.state == "idle" }.count
        header("Agents") {
            if rows.isEmpty {
                Button("Set up…") { model.openSettings(.agents) }.buttonStyle(.plain)
                    .font(.system(size: 12, weight: .medium)).foregroundStyle(Color.dozeAccent)
            } else {
                Text([working > 0 ? "\(working) working" : nil, idle > 0 ? "\(idle) idle" : nil].compactMap { $0 }.joined(separator: " · "))
                    .font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary)
            }
        }
        if rows.isEmpty {
            group {
                row(icon: "person.2.fill", color: Page.agents.color) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("No agents running")
                        Text("Connected agents show up here while they work").font(.system(size: 11)).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Button("Connect…") { model.openSettings(.agents) }.controlSize(.small)
                }
            }
        } else {
            group {
                ForEach(rows) { agent in agentRow(agent, now) }
                row(icon: "power", color: Color(white: 0.39)) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("When agents finish")
                        Text("After the final warning").font(.system(size: 11)).foregroundStyle(.secondary)
                    }
                    Spacer()
                    Picker("When agents finish", selection: Binding(
                        get: { model.string("agents.defaultCompletion") ?? "nothing" },
                        set: { model.set("agents.defaultCompletion", $0 == "nothing" ? nil : $0) })) {
                        Text("Nothing").tag("nothing")
                        ForEach(model.actions, id: \.self) { Text(actionLabel($0)).tag($0) }
                    }
                    .pickerStyle(.menu).labelsHidden().fixedSize()
                }
            }
        }
    }

    private func agentRow(_ agent: AgentRow, _ now: Date) -> some View {
        HStack(spacing: 10) {
            Monogram(agent.monogram)
            VStack(alignment: .leading, spacing: 1) {
                Text(agent.name)
                Text(caption(agent, now)).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
            }
            Spacer(minLength: 6)
            statePill(agent, now)
        }
        .frame(minHeight: 38)
        .padding(.horizontal, 12)
        .accessibilityElement(children: .combine)
    }

    private func caption(_ agent: AgentRow, _ now: Date) -> String {
        if agent.isProcess { return "Detected by process · no hooks" }
        var parts = [agent.project].compactMap { $0 }
        if agent.state == "idle" {
            parts.append("Finished \(max(1, (model.engineNow(now) - agent.stateSince) / 60)) min ago")
        } else if agent.state == "done" {
            parts.append(agent.task ?? "Done")
        } else if let task = agent.task {
            parts.append(task)
        }
        return parts.joined(separator: " · ")
    }

    @ViewBuilder private func statePill(_ agent: AgentRow, _ now: Date) -> some View {
        switch agent.state {
        case "working":
            HStack(spacing: 5) {
                PulsingDot(animated: !reduceMotion)
                Text("Working · \(elapsedText(model.workingSeconds(agent, now)))")
            }
            .font(.system(size: 11, weight: .medium)).foregroundStyle(Color.dozeWorking)
        case "done":
            Text("Done · \(elapsedText(model.workingSeconds(agent, now)))").font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary)
        default:
            Text(agent.isProcess ? "Open" : "Idle").font(.system(size: 11, weight: .medium)).foregroundStyle(.secondary)
        }
    }

    // MARK: Keep Awake and Power Timer

    private var keepAwakeSection: some View {
        VStack(alignment: .leading, spacing: 6) {
            header("Keep Awake") { EmptyView() }
            group {
                HStack(spacing: 4) {
                    ForEach(OverviewPage.presets, id: \.0) { preset in
                        ChipButton(preset.1) { model.send("awake", ["seconds": preset.0]) }.accessibilityLabel("Keep awake for \(preset.1)")
                    }
                    ChipButton("Indefinitely") { model.send("awake-forever") }.accessibilityLabel("Keep awake indefinitely")
                    Spacer(minLength: 4)
                    MoreMenu(items: [("Custom duration…", { model.showTimer(awake: true, date: false) }),
                                     ("Until a specific time…", { model.showTimer(awake: true, date: true) })])
                }
                .frame(minHeight: 38).padding(.horizontal, 12)
                row(icon: "speaker.wave.2.fill", color: Page.playback.color) {
                    Text("Keep awake while audio plays")
                    Spacer()
                    toggle("Keep awake while audio plays", session.bool("whileAudio")) { model.send("audio-toggle") }
                        .disabled(!model.snapshot.bool("audioSupported"))
                }
            }
        }
    }

    private func timerSection(_ now: Date) -> some View {
        let selected = session.string("selectedAction") ?? model.string("defaultAction") ?? "sleep"
        return VStack(alignment: .leading, spacing: 6) {
            header("Power Timer") { EmptyView() }
            group {
                row(icon: "power", color: Color(white: 0.39)) {
                    Text("Action")
                    Spacer()
                    Picker("Power Timer action", selection: Binding(get: { selected }, set: { model.send("select-action", ["action": $0]) })) {
                        ForEach(model.actions, id: \.self) { Text(actionLabel($0)).tag($0) }
                    }
                    .pickerStyle(.menu).labelsHidden().fixedSize()
                }
                if let timer = session.object("timer") {
                    HStack {
                        Text("\(actionLabel(timer.string("action"))) in \(remainingText(model.left(timer, now)))").monospacedDigit()
                        Spacer()
                        Button("Stop timer") { model.send("stop-timer") }.controlSize(.small)
                    }
                    .frame(minHeight: 38).padding(.horizontal, 12)
                } else {
                    HStack(spacing: 4) {
                        ForEach(OverviewPage.presets, id: \.0) { preset in
                            ChipButton(preset.1) { model.send("timer", ["seconds": preset.0, "action": selected]) }
                                .accessibilityLabel("\(actionLabel(selected)) in \(preset.1)")
                        }
                        Spacer(minLength: 4)
                        MoreMenu(items: [("Custom duration…", { model.showTimer(awake: false, date: false) }),
                                         ("At a specific time…", { model.showTimer(awake: false, date: true) })])
                    }
                    .frame(minHeight: 38).padding(.horizontal, 12)
                }
                row(icon: "play.fill", color: Color(.sRGB, red: 0.37, green: 0.36, blue: 0.9)) {
                    Text("\(actionLabel(model.string("playbackAction") ?? "sleep")) after playback stops")
                    Spacer()
                    toggle("Sleep after playback stops", session.bool("playbackEnabled")) { model.send("playback-toggle") }
                        .disabled(!model.snapshot.bool("audioSupported"))
                }
            }
        }
    }

    // MARK: Menus and footer

    /// Each row opens its page in place; Escape or the back button returns.
    private var menus: some View {
        group {
            ForEach(PanelPage.allCases, id: \.self) { page in
                linkRow(page.rawValue, icon: page.symbol, color: page.color, hint: "Shows \(page.rawValue) in the panel") {
                    model.showPanelPage(page)
                }
            }
        }
        .padding(.top, 6)
    }

    /// A row that opens something: a page of the panel, Settings, or (`external`) a web page.
    private func linkRow(_ title: String, icon: String, color: Color, hint: String, external: Bool = false,
                         action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 10) {
                SettingsIcon(symbol: icon, color: color)
                Text(title)
                Spacer()
                Image(systemName: external ? "arrow.up.right" : "chevron.right")
                    .font(.system(size: 11, weight: .semibold)).foregroundStyle(.tertiary)
            }
            .frame(minHeight: 38).padding(.horizontal, 12).contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(title)
        .accessibilityHint(hint)
    }

    // MARK: Pages

    private func pageHeader(_ page: PanelPage) -> some View {
        HStack(spacing: 8) {
            Button { model.showPanelPage(nil) } label: {
                Image(systemName: "chevron.left").font(.system(size: 12, weight: .semibold))
                    .frame(width: 26, height: 26)
                    .background(Color.primary.opacity(0.08), in: Circle())
                    .contentShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel("Back")
            .help("Back")
            Text(page.rawValue).font(.system(size: 13, weight: .bold)).accessibilityAddTraits(.isHeader)
            Spacer()
        }
        .padding(.horizontal, 2).padding(.bottom, 4)
    }

    /// The final warning: the live countdown when one runs, and how the warning behaves.
    @ViewBuilder private func countdownPage(_ now: Date) -> some View {
        pageHeader(.countdown)
        if session.object("countdown") != nil {
            status(now)
        } else {
            group {
                row(icon: "stopwatch.fill", color: PanelPage.countdown.color) {
                    VStack(alignment: .leading, spacing: 1) {
                        Text("No countdown running")
                        Text("A final warning runs before every power action").font(.system(size: 11)).foregroundStyle(.secondary)
                    }
                    Spacer()
                }
            }
        }
        header("Final warning") { EmptyView() }
        group {
            plainRow {
                Text("Warning length")
                Spacer()
                choice("Warning length", key: "countdownSeconds", values: [60, 120, 180, 300, 600, 900], fallback: 300,
                       label: { durationLabel(seconds: $0) })
            }
            plainRow { Text("Play a sound"); Spacer(); settingToggle("Play a sound when the warning appears", key: "warningSound") }
            plainRow { Text("Show on every display"); Spacer(); settingToggle("Show on every display", key: "warningAllDisplays") }
            plainRow {
                Text("Snooze length")
                Spacer()
                choice("Snooze length", key: "snoozeMinutes", values: [5, 10, 15, 20, 30, 45, 60], fallback: 15,
                       label: { durationLabel(minutes: $0) })
            }
        }
        group {
            linkRow("Preview the warning", icon: "eye.fill", color: Page.notifications.color,
                    hint: "Shows the warning without scheduling anything") {
                model.panel.close()
                model.send("preview")
            }
        }
        footer("Notification Settings…", page: .notifications)
    }

    /// Saved preferences, as in the menu's Quick Settings, without leaving the panel.
    @ViewBuilder private var quickPage: some View {
        pageHeader(.quick)
        group {
            plainRow { Text("Keep the display on"); Spacer(); settingToggle("Keep the display on", key: "allowDisplaySleep", inverted: true) }
            if model.snapshot.bool("lidClosedSupported") {
                plainRow { Text("Stay awake with the lid closed"); Spacer(); settingToggle("Stay awake with the lid closed", key: "lidClosedKeepAwake") }
            }
            plainRow { Text("Show countdown notifications"); Spacer(); settingToggle("Show countdown notifications", key: "notifications") }
        }
        group {
            plainRow {
                Text("Open Doze at login")
                Spacer()
                settingToggle("Open Doze at login", key: "launchAtStartup").disabled(!model.snapshot.bool("startupSupported"))
            }
            plainRow { Text("Start in the menu bar"); Spacer(); settingToggle("Start in the menu bar", key: "startMinimized") }
            plainRow { Text("Write diagnostic logs"); Spacer(); settingToggle("Write diagnostic logs", key: "logging") }
        }
        group {
            plainRow {
                Text("Default keep awake")
                Spacer()
                choice("Default keep awake", key: "defaultAwakeMinutes", values: [15, 30, 60, 120], fallback: 30,
                       label: { durationLabel(minutes: $0) })
            }
            plainRow {
                Text("Default power timer")
                Spacer()
                choice("Default power timer", key: "defaultTimerMinutes", values: [15, 30, 60, 120], fallback: 30,
                       label: { durationLabel(minutes: $0) })
            }
        }
        footer("All Settings…", page: .general)
    }

    @ViewBuilder private var supportPage: some View {
        pageHeader(.support)
        group {
            HStack(spacing: 12) {
                AppIcon(model: model, size: 36)
                VStack(alignment: .leading, spacing: 1) {
                    Text("Doze").font(.system(size: 13, weight: .semibold))
                    Text("Version \(model.snapshot.string("version") ?? "")").font(.system(size: 11)).foregroundStyle(.secondary)
                }
                .accessibilityElement(children: .combine)
                Spacer()
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
        }
        group {
            linkRow("Menu guide", icon: Page.guide.symbol, color: Page.guide.color, hint: "Opens the Menu guide in Settings") {
                model.openSettings(.guide)
            }
            linkRow("About Doze", icon: Page.about.symbol, color: Page.about.color, hint: "Opens About Doze in Settings") {
                model.openSettings(.about)
            }
        }
        let links = model.snapshot.objects("links")
        if !links.isEmpty {
            group {
                ForEach(links.indices, id: \.self) { index in
                    let title = links[index].string("title") ?? ""
                    linkRow(title, icon: "safari.fill", color: Color(white: 0.45), hint: "Opens \(title) in your browser", external: true) {
                        model.panel.close()
                        model.openLink(links[index].string("url") ?? "")
                    }
                }
            }
        }
        footer()
    }

    /// The left of the footer opens Settings, or one page of it from a panel page.
    private func footer(_ title: String = "Settings…", page: Page? = nil) -> some View {
        HStack {
            Button { if let page { model.openSettings(page) } else { model.showSettings() } } label: {
                HStack(spacing: 6) { Text(title); Text("⌘,").foregroundStyle(.tertiary) }
            }
            .keyboardShortcut(",", modifiers: .command)
            Spacer()
            Button { model.send("quit") } label: {
                HStack(spacing: 6) { Text("Quit Doze"); Text("⌘Q").foregroundStyle(.tertiary) }
            }
            .keyboardShortcut("q", modifiers: .command)
        }
        .buttonStyle(.plain)
        .font(.system(size: 12))
        .foregroundStyle(.secondary)
        .padding(.horizontal, 6).padding(.top, 4)
    }

    // MARK: Pieces

    private func header<Trailing: View>(_ title: String, @ViewBuilder trailing: () -> Trailing) -> some View {
        HStack(alignment: .firstTextBaseline) {
            Text(title).font(.system(size: 12, weight: .bold)).accessibilityAddTraits(.isHeader)
            Spacer()
            trailing()
        }
        .padding(.horizontal, 4).padding(.top, 8).padding(.bottom, 2)
    }

    private func group<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        VStack(spacing: 0) { content() }
            .background(Color.dozeSection, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
    }

    private func row<Content: View>(icon: String, color: Color, @ViewBuilder _ content: () -> Content) -> some View {
        HStack(spacing: 10) {
            SettingsIcon(symbol: icon, color: color)
            content()
        }
        .frame(minHeight: 38).padding(.horizontal, 12)
    }

    private func toggle(_ name: String, _ on: Bool, _ flip: @escaping () -> Void) -> some View {
        Toggle(name, isOn: Binding(get: { on }, set: { _ in flip() }))
            .toggleStyle(.switch).controlSize(.small).labelsHidden()
    }

    /// A row of a panel page: its label and control, without an icon.
    private func plainRow<Content: View>(@ViewBuilder _ content: () -> Content) -> some View {
        HStack(spacing: 10) { content() }.frame(minHeight: 38).padding(.horizontal, 12)
    }

    /// A switch for one saved setting; `inverted` shows the setting's opposite, as "Keep the
    /// display on" does for allowDisplaySleep.
    private func settingToggle(_ name: String, key: String, inverted: Bool = false) -> some View {
        toggle(name, model.bool(key) != inverted) { model.set(key, !model.bool(key)) }
    }

    /// A pop-up of durations for one saved setting, keeping a value set elsewhere.
    private func choice(_ name: String, key: String, values: [Int], fallback: Int, label: @escaping (Int) -> String) -> some View {
        let current = model.int(key, fallback)
        return Picker(name, selection: Binding(get: { current }, set: { model.set(key, $0) })) {
            ForEach(Array(Set(values + [current])).sorted(), id: \.self) { Text(label($0)).tag($0) }
        }
        .pickerStyle(.menu).labelsHidden().fixedSize()
    }
}

extension PanelPage {
    var symbol: String {
        switch self {
        case .countdown: return "stopwatch.fill"
        case .quick: return "slider.horizontal.3"
        case .support: return "info"
        }
    }

    var color: Color {
        switch self {
        case .countdown: return Color(.sRGB, red: 1, green: 0.62, blue: 0.04)
        case .quick: return Color(white: 0.56)
        case .support: return Page.about.color
        }
    }
}

/// The Working dot breathes, unless Reduce Motion is on.
struct PulsingDot: View {
    let animated: Bool
    @State private var dim = false
    var body: some View {
        Circle().fill(Color.dozeAccent).frame(width: 6, height: 6)
            .opacity(dim ? 0.35 : 1)
            .onAppear {
                guard animated else { return }
                withAnimation(.easeInOut(duration: 0.8).repeatForever(autoreverses: true)) { dim = true }
            }
            .accessibilityHidden(true)
    }
}
