namespace Doze.SettingsUi;

// Wording shared with the tray and the macOS companion. Engine ids and raw seconds never reach the UI.
internal static class Labels
{
    public static string Action(string? action) => action switch
    {
        null or "normal" => "Return to normal",
        "sleep" => "Sleep",
        "hibernate" => "Hibernate",
        "shutdown" => "Shut down",
        "lock" => "Lock",
        "displayOff" => "Turn display off",
        _ => action
    };

    public static string AgentStatus(string? status) => status switch
    {
        "active" => "Working",
        "connection_lost" => "Connection lost · keeping awake",
        "awaiting_authorization" => "Waiting for your approval",
        _ => status?.Replace('_', ' ') ?? ""
    };

    public static string PlaybackPhase(string? phase) => phase switch
    {
        "playing" => "Playing · waiting for it to stop",
        "grace" => "Waiting for silence and inactivity",
        "countdown" => "Final warning shown",
        _ => "Waiting for playback to start"
    };

    public static string CountdownSource(string? source) => source switch
    {
        "timer" => "From the Power Timer",
        "playback" => "After playback stopped",
        "agents" => "Agents and command-line jobs finished",
        _ => "A power action is about to run"
    };

    /// Compact time left, as in the tray: "1h 5m", "42m", "30s".
    public static string Remaining(long seconds)
    {
        if (seconds >= 3600) return $"{seconds / 3600}h {seconds / 60 % 60}m";
        if (seconds >= 60) return $"{(seconds + 59) / 60}m";
        return $"{Math.Max(0, seconds)}s";
    }

    /// The final warning's clock, "4:07".
    public static string Clock(long seconds) => $"{Math.Max(0, seconds) / 60}:{Math.Max(0, seconds) % 60:00}";

    public static string Preset(int minutes) => minutes < 60 ? $"{minutes}m" : $"{minutes / 60}h";

    public static string Minutes(int minutes)
    {
        if (minutes < 60) return Unit(minutes, "minute");
        if (minutes % 60 == 0) return minutes % 1440 == 0 ? Unit(minutes / 1440, "day") : Unit(minutes / 60, "hour");
        return Unit(minutes / 60, "hour") + " " + Unit(minutes % 60, "minute");
    }

    public static string Seconds(int seconds)
    {
        if (seconds < 60) return Unit(seconds, "second");
        if (seconds % 60 == 0) return Minutes(seconds / 60);
        return Unit(seconds / 60, "minute") + " " + Unit(seconds % 60, "second");
    }

    private static string Unit(int value, string name) => $"{value} {name}{(value == 1 ? "" : "s")}";

    /// "Claude Code, Codex and OpenCode".
    public static string List(IReadOnlyList<string> items) => items.Count switch
    {
        0 => "",
        1 => items[0],
        _ => string.Join(", ", items.Take(items.Count - 1)) + " and " + items[^1]
    };

    /// Minutes of work for an agent row: "12m", "1h 5m".
    public static string Elapsed(long seconds) => seconds >= 3600 ? $"{seconds / 3600}h {seconds / 60 % 60}m" : $"{Math.Max(0, seconds) / 60}m";

    public static void Verify()
    {
        var checks = new (string Actual, string Expected)[]
        {
            (Action("displayOff"), "Turn display off"), (Action("shutdown"), "Shut down"), (Action(null), "Return to normal"),
            (Seconds(300), "5 minutes"), (Seconds(60), "1 minute"), (Seconds(3600), "1 hour"), (Seconds(90), "1 minute 30 seconds"),
            (Minutes(1440), "1 day"), (Minutes(90), "1 hour 30 minutes"), (Remaining(3900), "1h 5m"), (Remaining(61), "2m"),
            (Clock(287), "4:47"), (AgentStatus("awaiting_authorization"), "Waiting for your approval"), (Preset(120), "2h"),
            (List(["Claude Code", "Codex", "OpenCode"]), "Claude Code, Codex and OpenCode"), (Elapsed(720), "12m"), (Elapsed(3900), "1h 5m")
        };
        foreach (var (actual, expected) in checks)
            if (actual != expected) throw new InvalidOperationException($"Label \"{actual}\" should read \"{expected}\".");
    }
}
