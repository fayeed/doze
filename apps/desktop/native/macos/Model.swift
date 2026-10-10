import AppKit
import SwiftUI

/// Engine messages are JSON objects; settings and sessions are read by key, so new keys in
/// the engine's settings store need no change here.
typealias JSON = [String: Any]

extension Dictionary where Key == String, Value == Any {
    /// A value by dotted path, such as "agents.askBeforeNew".
    func value(_ path: String) -> Any? {
        var node: Any? = self
        for part in path.split(separator: ".") { node = (node as? JSON)?[String(part)] }
        return node is NSNull ? nil : node
    }
    func bool(_ path: String) -> Bool { (value(path) as? NSNumber)?.boolValue ?? false }
    func int(_ path: String) -> Int? { (value(path) as? NSNumber)?.intValue }
    func string(_ path: String) -> String? { value(path) as? String }
    func object(_ path: String) -> JSON? { value(path) as? JSON }
    func objects(_ path: String) -> [JSON] { value(path) as? [JSON] ?? [] }
}

extension Color {
    static func dynamic(light: NSColor, dark: NSColor) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
        })
    }
    private static func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> NSColor {
        NSColor(srgbRed: CGFloat(hex >> 16 & 0xFF) / 255, green: CGFloat(hex >> 8 & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }
    /// The panel and Settings accent: #F0943F on dark, #AE4F0E on light (4.5:1 or better).
    static let dozeAccent = dynamic(light: rgb(0xAE4F0E), dark: rgb(0xF0943F))
    /// Tinted button backgrounds: rgba(214,110,40,.12) on light, rgba(240,136,62,.15) on dark.
    static let dozeTint = dynamic(light: rgb(0xD66E28, 0.12), dark: rgb(0xF0883E, 0.15))
    /// Prominent buttons such as Allow and Stay Awake.
    static let dozeProminent = dynamic(light: rgb(0xC25B16), dark: rgb(0xE2772F))
    /// The pulsing Working dot and its text.
    static let dozeWorking = dynamic(light: rgb(0xA64A0C), dark: rgb(0xF6A35E))
    static let dozeTile = dynamic(light: rgb(0xE6E4EC), dark: rgb(0x3A3A44))
    static let dozeTileText = dynamic(light: rgb(0x4A4C5E), dark: rgb(0xDAD8E4))
    static let dozeSection = dynamic(light: rgb(0xFFFFFF, 0.7), dark: rgb(0xFFFFFF, 0.06))
    static let statusIdle = LinearGradient(colors: [Color(nsColor: rgb(0x8A7CF8)), Color(nsColor: rgb(0x5B4FE0))], startPoint: .top, endPoint: .bottom)
    static let statusAwake = LinearGradient(colors: [Color(nsColor: rgb(0xF6A04D)), Color(nsColor: rgb(0xE2772F))], startPoint: .top, endPoint: .bottom)
}

/// Labels match the Rust engine, the menu bar and the Windows app.
func actionLabel(_ action: String?) -> String {
    guard let action else { return "Nothing" }
    return ["sleep": "Sleep", "hibernate": "Hibernate", "shutdown": "Shut down", "lock": "Lock",
            "displayOff": "Turn display off", "nothing": "Nothing"][action] ?? action
}

func playbackPhaseLabel(_ phase: String?) -> String {
    switch phase {
    case "playing": return "Playing · waiting for it to stop"
    case "grace": return "Waiting for silence and inactivity"
    case "countdown": return "Final warning shown"
    default: return "Waiting for playback to start"
    }
}

func countdownSourceLabel(_ source: String?) -> String {
    switch source {
    case "agents": return "All agents finished"
    case "timer": return "Power timer finished"
    case "playback": return "Playback stopped"
    default: return "Final warning"
    }
}

/// Compact time left, as in the menu bar: "1h 5m", "42m", "30s".
func remainingText(_ seconds: Int) -> String {
    if seconds >= 3600 { return "\(seconds / 3600)h \(seconds / 60 % 60)m" }
    if seconds >= 60 { return "\((seconds + 59) / 60)m" }
    return "\(max(0, seconds))s"
}

/// The final warning's clock, "4:47".
func clockText(_ seconds: Int) -> String { "\(max(0, seconds) / 60):" + String(format: "%02d", max(0, seconds) % 60) }

/// Minutes of agent work: "12m", "1h 5m".
func elapsedText(_ seconds: Int) -> String {
    seconds >= 3600 ? "\(seconds / 3600)h \(seconds / 60 % 60)m" : "\(max(0, seconds) / 60)m"
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

/// "Claude Code, Codex and OpenCode".
func listText(_ items: [String]) -> String {
    guard items.count > 1 else { return items.first ?? "" }
    return items.dropLast().joined(separator: ", ") + " and " + items.last!
}

/// One agent session as both apps render it (the engine's SessionView).
struct AgentRow: Identifiable {
    let id: String
    let name: String
    let monogram: String
    let project: String?
    let task: String?
    let state: String
    let source: String
    let startedAt: Int
    let stateSince: Int
    let workingSeconds: Int
    let holds: Bool
    let completionAction: String?

    init(_ json: JSON) {
        id = json.string("id") ?? UUID().uuidString
        name = json.string("name") ?? "Agent"
        monogram = json.string("monogram") ?? "?"
        project = json.string("project")
        task = json.string("task")
        state = json.string("state") ?? "idle"
        source = json.string("source") ?? "hooks"
        startedAt = json.int("startedAt") ?? 0
        stateSince = json.int("stateSince") ?? 0
        workingSeconds = json.int("workingSeconds") ?? 0
        holds = json.bool("holdsAssertion")
        completionAction = json.string("completionAction")
    }

    var isProcess: Bool { source == "process" }
}

/// One tool in Settings › Agents › Connected agents.
struct AgentLink: Identifiable {
    let id: String
    let name: String
    let monogram: String
    let method: String
    let installed: Bool
    let connected: Bool
    init(_ json: JSON) {
        id = json.string("id") ?? ""
        name = json.string("name") ?? id
        monogram = json.string("monogram") ?? "?"
        method = json.string("method") ?? "Hooks"
        installed = json.bool("installed")
        connected = json.bool("connected")
    }
}

/// A config change to confirm before Doze writes it.
struct PendingChange: Identifiable {
    let id = UUID()
    let agent: String
    let remove: Bool
    let path: String
    let diff: String
    let note: String?
    let token: String
    init(_ json: JSON) {
        agent = json.string("agent") ?? ""
        remove = json.bool("remove")
        path = json.string("path") ?? ""
        diff = json.string("diff") ?? ""
        note = json.string("note")
        token = json.string("token") ?? ""
    }
}

/// "Use a prompt…": text for the agent, so it adds Doze's hooks itself.
struct SetupPrompt: Identifiable {
    let id = UUID()
    let agent: String
    let name: String
    let path: String
    let prompt: String
    init(_ json: JSON) {
        agent = json.string("agent") ?? ""
        name = json.string("name") ?? agentName(agent)
        path = json.string("path") ?? ""
        prompt = json.string("prompt") ?? ""
    }
}

func agentName(_ id: String) -> String {
    switch id {
    case "claude-code": return "Claude Code"
    case "codex": return "Codex"
    case "opencode": return "OpenCode"
    case "gemini-cli": return "Gemini CLI"
    case "cursor": return "Cursor"
    default: return id.split(separator: ":", maxSplits: 1).last.map(String.init) ?? id
    }
}

/// The nine Settings pages, shared with Windows and the engine's page ids.
enum Page: String, CaseIterable, Identifiable {
    case overview = "Overview"
    case general = "General"
    case session = "Session defaults"
    case playback = "After playback"
    case notifications = "Notifications"
    case agents = "Agents"
    case advanced = "Advanced"
    case guide = "Menu guide"
    case about = "About Doze"

    var id: String { rawValue }

    init?(engineId: String) {
        switch engineId {
        case "overview": self = .overview
        case "general": self = .general
        case "session": self = .session
        case "playback": self = .playback
        case "notifications", "notif": self = .notifications
        case "agents": self = .agents
        case "advanced": self = .advanced
        case "guide", "help": self = .guide
        case "about": self = .about
        default: return nil
        }
    }

    /// Sidebar groups: gaps before Agents and before Menu guide.
    static let groups: [[Page]] = [[.overview, .general, .session, .playback, .notifications], [.agents, .advanced], [.guide, .about]]

    var symbol: String {
        switch self {
        case .overview: return "moon.fill"
        case .general: return "gearshape.fill"
        case .session: return "sun.max.fill"
        case .playback: return "speaker.wave.2.fill"
        case .notifications: return "bell.badge.fill"
        case .agents: return "person.2.fill"
        case .advanced: return "slider.horizontal.3"
        case .guide: return "book.fill"
        case .about: return "info"
        }
    }

    /// The icon colours from the design's sidebar.
    var color: Color {
        func hex(_ value: UInt32) -> Color {
            Color(.sRGB, red: Double(value >> 16 & 0xFF) / 255, green: Double(value >> 8 & 0xFF) / 255, blue: Double(value & 0xFF) / 255)
        }
        switch self {
        case .overview: return hex(0x6C5CE7)
        case .general: return hex(0x8E8E93)
        case .session: return hex(0xFF9F0A)
        case .playback: return hex(0xFF453A)
        case .notifications: return hex(0xFF375F)
        case .agents: return hex(0x30B0C7)
        case .advanced: return hex(0x636366)
        case .guide: return hex(0xFF9F0A)
        case .about: return hex(0x0A84FF)
        }
    }

    /// Extra words people use for settings on the page.
    var keywords: String {
        switch self {
        case .overview: return "status control start stop preset"
        case .general: return "login startup menu bar icon panel battery display screen lid"
        case .session: return "default duration snooze stay awake hibernate shut down lock"
        case .playback: return "audio music video movie silence inactivity idle"
        case .notifications: return "final warning countdown sound alert display"
        case .agents: return "mcp codex claude code opencode gemini cursor hooks plugin lease approval allow deny"
        case .advanced: return "command line cli terminal path doze run mcp server assertion reset logs diagnostics"
        case .guide: return "help icon glyph sun"
        case .about: return "version privacy website"
        }
    }
}

/// A setting as search finds it: every row's title and description on every page.
struct SearchEntry: Identifiable, Hashable {
    let page: Page
    let title: String
    let detail: String
    var id: String { page.rawValue + "/" + title }
}
