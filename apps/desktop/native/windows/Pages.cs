using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Doze.SettingsUi;

// Settings pages, built in code from the card helpers in SettingsLayout.cs.
public sealed partial class MainWindow
{
    private void General()
    {
        Section("Startup");
        Card("Launch at sign-in", "Doze starts quietly in the system tray when you sign in to Windows.", "",
            Switch("Launch at sign-in", draft.LaunchAtStartup, value => { draft.LaunchAtStartup = value; Changed(); }, Capability("startupSupported")));
        Card("Start in the system tray", "Opening Doze doesn't show this window. Start sessions from the tray icon.", "",
            Switch("Start in the system tray", draft.StartMinimized, value => { draft.StartMinimized = value; Changed(); }));
        Section("Appearance");
        Card("App theme", "Use system setting follows your Windows mode.", "",
            Choice("App theme", [("system", "Use system setting"), ("light", "Light"), ("dark", "Dark")], draft.Theme, value => { draft.Theme = value; Changed(); }));
    }

    private void SessionDefaults()
    {
        Section("Keep Awake");
        Card("Default duration", "Used by Keep Awake › Default in the tray menu.", "",
            Choice("Keep Awake default duration", MinuteChoices(draft.DefaultAwakeMinutes), draft.DefaultAwakeMinutes, value => { draft.DefaultAwakeMinutes = value; Changed(); }));
        Card("Allow the display to turn off", "The computer stays awake, but Windows can turn off the screen. Applies right away.", "",
            Switch("Allow the display to turn off", draft.AllowDisplaySleep, value => { draft.AllowDisplaySleep = value; Changed(); }));
        Section("Power Timer");
        Card("Default duration", "Used by Power Timer › Default in the tray menu.", "",
            Choice("Power Timer default duration", MinuteChoices(draft.DefaultTimerMinutes), draft.DefaultTimerMinutes, value => { draft.DefaultTimerMinutes = value; Changed(); }));
        Card("Default action", "Applies to new timers.", "",
            Choice("Power Timer default action", ActionChoices, draft.DefaultAction, value => { draft.DefaultAction = value; Changed(); }));
        Footer("A timer keeps the computer awake until its final warning. " + ActionGuide);
    }

    private const string ActionGuide = "Sleep keeps your session in memory, Hibernate saves it to disk, Shut down closes Windows (apps with unsaved work can stop it), Lock secures your session, and Turn display off switches off the screen.";

    private void AfterPlayback()
    {
        var supported = Capability("audioSupported");
        if (!supported)
            Card("Audio monitoring unavailable", "After Playback and keeping awake while audio plays are unavailable in this build.", "");
        Section("When playback ends");
        var rows = ExpanderCard("Action", "What happens once audio has stopped and you're away.", "",
            Choice("After playback action", ActionChoices, draft.PlaybackAction, value => { draft.PlaybackAction = value; Changed(); }, supported));
        Row(rows, "After silence of", "Pauses, buffering and quiet scenes shorter than this are ignored.",
            Choice("After silence of", SecondChoices(draft.SilenceSeconds, 10, 20, 30, 60, 120, 300, 600, 900, 1800), draft.SilenceSeconds, value => { draft.SilenceSeconds = value; Changed(); }, supported));
        Row(rows, "And no activity for", "Using the keyboard or mouse restarts this wait.",
            Choice("And no activity for", SecondChoices(draft.IdleSeconds, 30, 60, 120, 300, 600, 900, 1800, 3600), draft.IdleSeconds, value => { draft.IdleSeconds = value; Changed(); }, supported));
        Footer(PlaybackOneShot);
    }

    private const string PlaybackOneShot = "Turn After Playback on from the tray or Overview before you start watching. It is one-shot: after its action runs, or when you choose Cancel or Stay Awake on its warning, it turns itself off. Using the computer or resumed audio only restarts the wait and keeps it armed.";

    private void Notifications()
    {
        Section("Final warning");
        Card("Warning length", "How long you have to cancel before the action runs.", "",
            Choice("Warning length", SecondChoices(draft.CountdownSeconds, 15, 30, 60, 120, 180, 300, 600, 900, 1800), draft.CountdownSeconds, value => { draft.CountdownSeconds = value; Changed(); }));
        Card("Show a notification", "Also announces the final warning in Windows notifications. The warning window appears either way.", "",
            Switch("Show a notification", draft.Notifications, value => { draft.Notifications = value; Changed(); }));
        Card("Preview", "Shows the warning without scheduling anything.", "", ActionButton("Preview warning", () => Send("preview")));
        Footer("The warning stays on top and always offers Cancel, Snooze 15 minutes and Stay Awake. Closing it or pressing Esc cancels the action. A real warning always replaces a preview.");
    }

    private void Advanced()
    {
        Section("Diagnostics");
        Card("Write diagnostic logs", "Errors are logged on this computer, up to about 256 KB.", "",
            Switch("Write diagnostic logs", draft.Logging, value => { draft.Logging = value; Changed(); }));
        Card("Preferences file", snapshot["settingsPath"]?.GetValue<string>() ?? "Loading…", "", ActionButton("Open folder", OpenData));
        Card("This computer", "Power actions: " + string.Join(", ", Actions.Select(Labels.Action)) + ". Audio monitoring " + (Capability("audioSupported") ? "available; audio is never recorded." : "unavailable."), "");
        CommandLine();
        Section("Reset");
        Card("Reset preferences", "Restores the defaults on every page. Agent connections and sessions are kept.", "", ActionButton("Reset…", ConfirmResetAsync, "Reset preferences"));
        Footer("Doze validates every change and owns all power actions. Closing this window keeps Doze running; sessions are never restored after a restart or after the computer sleeps.");
    }

    /// doze-cli.exe beside the engine: the console front end that shells wait for.
    private string CommandLinePath()
    {
        var engine = Text(snapshot["executable"]);
        if (engine is null) return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Doze", "doze-cli.exe");
        return Path.Combine(Path.GetDirectoryName(engine) ?? "", "doze-cli.exe");
    }

    private static string PowerShellQuoted(string text) => "'" + text.Replace("'", "''") + "'";

    private void CommandLine()
    {
        var path = CommandLinePath();
        var example = $"& {PowerShellQuoted(path)} run --then sleep -- ffmpeg -i in.mov out.mp4";
        var alias = $"Set-Alias doze {PowerShellQuoted(path)}";
        Section("Command line");
        var rows = ExpanderCard("doze run and doze watch", "Keep the computer awake while a job runs in PowerShell or Command Prompt, then optionally sleep.", "");
        Code(Row(rows, "Program", path, CopyButton("Copy path", path, "Copy the doze-cli.exe path")));
        Code(Row(rows, "PowerShell alias", alias, CopyButton("Copy alias", alias, "Copy the PowerShell alias")));
        Code(Row(rows, "Example", example + "\ndoze watch --pid 1234 --then sleep", CopyButton("Copy example", example, "Copy the example command")));
        Footer("Add the alias to your PowerShell profile (notepad $PROFILE) to type doze run and doze watch. The -⁠-⁠then option accepts nothing, sleep, display-off, lock, hibernate or shutdown. The action runs only after the job succeeds and its final warning ends; a failed job or Ctrl-C releases the computer without any action. Running jobs appear in Agents. Doze must be running.");
    }

    private static void Code(TextBlock text) => text.Style = Style("CodeTextStyle");

    private Button CopyButton(string label, string text, string accessibleName)
    {
        var button = new Button { Content = label };
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(button, accessibleName);
        button.Click += (_, _) =>
        {
            try
            {
                var data = new Windows.ApplicationModel.DataTransfer.DataPackage();
                data.SetText(text);
                Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(data);
                button.Content = "Copied";
            }
            catch (Exception error) { Notify("Couldn't copy", error.Message, InfoBarSeverity.Error); }
        };
        return button;
    }

    private void MenuGuide()
    {
        Section("In the tray");
        Card("Normal sleep allowed", "No Doze session is holding the computer awake, so your Windows power settings apply.", Tinted("", "DozeIndigoBrush"));
        Card("Keep Awake", "Choose a duration, an end time or indefinitely. Stop releases Doze's power request; Extend adds 15 minutes to a timed session.", Tinted("", "DozeOrangeBrush"));
        Card("Power Timer", "Choose an action and a duration. A final warning lets you cancel or snooze before the action runs.", Tinted("", "DozeBlueBrush"));
        Card("Keep awake while audio plays", "Holds the computer awake while sound is playing, through the silence grace period. Muted output counts as silence.", Tinted("", "DozePinkBrush"));
        Card("After Playback", "Once playback has been heard, Doze waits for silence and inactivity, then shows the final warning. It is one-shot: it turns itself off after its action runs or when you choose Cancel or Stay Awake. Using the computer or resumed audio keeps it armed.", Tinted("", "DozePurpleBrush"));
        Card("Final warning", "Every power action shows a warning first. Cancel removes the action, Snooze waits 15 more minutes, and Stay Awake keeps the computer awake instead.", Tinted("", "DozeRedBrush"));
        Section("More ways to use Doze");
        Card("Command line", "doze run --then sleep -- your-command stays awake until a job finishes; doze watch --pid follows one that is already running. Advanced has the doze-cli.exe path and a PowerShell alias.", Tinted("", "DozeGrayBrush"));
        Card("Agents", "Coding agents connected through MCP keep the computer awake while they work. New requests need your approval in Agents unless you granted them there.", Tinted("", "DozeTealBrush"));
        Card("Quick Settings", "Checkmarks are saved preferences. Duration defaults apply to new sessions; other changes, such as display sleep, apply right away.", Tinted("", "DozeGrayBrush"));
        Card("Unavailable commands", "Stop, Extend, Cancel and Snooze are dimmed when there is nothing to stop. Actions this computer doesn't support stay dimmed.", Tinted("", "DozeGrayBrush"));
        Footer(ActionGuide);
    }

    private void About()
    {
        var hero = new Grid { ColumnSpacing = 20 };
        hero.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        hero.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var icon = Asset("Doze.png", 72, "");
        icon.VerticalAlignment = VerticalAlignment.Top;
        Microsoft.UI.Xaml.Automation.AutomationProperties.SetAccessibilityView(icon, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        hero.Children.Add(icon);
        var text = new StackPanel { Spacing = 2 };
        text.Children.Add(new TextBlock { Text = "Doze", Style = Style("SubtitleTextBlockStyle") });
        text.Children.Add(new TextBlock { Text = $"Version {snapshot["version"]?.GetValue<string>() ?? "0.1.0"} · Windows {System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString().ToLowerInvariant()}", Style = Style("CardDescriptionStyle"), FontSize = 14, IsTextSelectionEnabled = true });
        text.Children.Add(new TextBlock { Text = "Your computer knows when it's bedtime.", Style = Style("CardTitleStyle"), Margin = new Thickness(0, 8, 0, 0) });
        text.Children.Add(new TextBlock { Text = "Made by Fayeed Pawaskar", Style = Style("CardDescriptionStyle") });
        var links = new WrapPanel { Spacing = 4, Margin = new Thickness(-12, 6, 0, 0) };
        links.Children.Add(Link("Source Code", () => OpenLink("https://github.com/fayeed/doze")));
        links.Children.Add(Link("Open data folder", OpenData));
        text.Children.Add(links);
        Grid.SetColumn(text, 1);
        hero.Children.Add(text);
        group.Children.Add(new Border { Style = Style("SettingsCardStyle"), Padding = new Thickness(20), Child = hero });

        Section("Also from the developer");
        Card("Clypy", "A clipboard manager for Mac, Windows, Linux and phones, also by Fayeed Pawaskar.", Asset("Clypy.png", 32, ""),
            ActionButton("Visit", () => OpenLink("https://clypy.app"), "Visit Clypy"));
        Section("Privacy");
        Card("No account. No cloud. No ads.", "Doze does not record audio, simulate input or send telemetry. Preferences and optional diagnostics stay on this computer.", Tinted("", "DozeGreenBrush"));
        Section("Built with");
        Card("Rust and Tauri", "The engine owns sessions, power requests, validation and countdown safety.", Tinted("", "DozeOrangeBrush"));
        Card("Windows App SDK and WinUI 3", "Settings, timers and the final warning use native Windows controls and Mica.", Tinted("", "DozeBlueBrush"));
        Footer("Open-source components retain their respective licenses.");
    }

    private HyperlinkButton Link(string label, Func<Task> open)
    {
        var link = new HyperlinkButton { Content = label };
        link.Click += async (_, _) =>
        {
            try { await open(); }
            catch (Exception error) { Notify("Couldn't open " + label, error.Message, InfoBarSeverity.Error); }
        };
        return link;
    }

    private ContentDialog ResetDialog() => new()
    {
        XamlRoot = Root.XamlRoot, RequestedTheme = Root.ActualTheme,
        Title = "Reset all preferences?",
        Content = new TextBlock { Text = "Every page returns to its defaults, including launch at sign-in. Agent connections, permissions and running sessions are kept.", TextWrapping = TextWrapping.Wrap },
        PrimaryButtonText = "Reset", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close
    };

    private async Task ConfirmResetAsync()
    {
        if (await ResetDialog().ShowAsync() == ContentDialogResult.Primary) ResetDraft();
    }

    private void Agents()
    {
        var settings = snapshot["settings"]?["agents"] as JsonObject;
        var enabled = settings?["enabled"]?.GetValue<bool>() == true;
        var rows = ExpanderCard("Allow agents to connect", "Coding agents such as Codex and Claude Code connect through MCP on this computer.", "",
            Switch("Allow agents to connect", enabled, value => _ = Send("agent-enable")));
        var lease = settings?["leaseSeconds"]?.GetValue<int>() ?? 300;
        Row(rows, "Heartbeat lease", "How long a session stays awake between check-ins.",
            Choice("Heartbeat lease", SecondChoices(lease, 60, 300, 900, 1800, 3600), lease, value => _ = bridge.SendAgentAsync("agent-lease", seconds: value)));
        var completion = settings?["defaultCompletion"]?.GetValue<string>() ?? "normal";
        Row(rows, "When an agent finishes", "Used when the agent doesn't ask for an action.",
            Choice("When an agent finishes", ActionChoices.Prepend(("normal", "Return to normal")), completion, value => _ = bridge.SendAgentAsync("agent-default", action: value == "normal" ? null : value)));
        var keepAlive = settings?["keepAliveWhileConnected"]?.GetValue<bool>() ?? true;
        Row(rows, "Keep sessions alive while connected", "Renews a session through long steps without check-ins, such as a build, for up to 24 hours while the agent app stays connected. It never finishes a session.",
            Switch("Keep sessions alive while connected", keepAlive, value => _ = Send("agent-keepalive")));
        Footer("A lost connection keeps the computer awake for up to 30 minutes, then releases without any action. An automatic action needs every overlapping agent to finish with the same approved action, followed by a final warning of at least five minutes.");
        Section("Agent connections");
        foreach (var name in new[] { "Codex", "Claude Code", "Generic MCP client" })
        {
            var client = (settings?["clients"] as JsonArray)?.OfType<JsonObject>().FirstOrDefault(c => c["name"]?.GetValue<string>() == name);
            var controls = Buttons(ActionButton(client is null ? "Set up" : "Configure", () => OpenAgentSetupAsync(name), (client is null ? "Set up " : "Configure ") + name));
            if (client is not null) controls.Children.Add(ActionButton("Permissions", () => OpenAgentPermissionsAsync(name), name + " permissions"));
            Card(name, client is null ? "Set up a local connection to Doze." : "Profile created · " + (client["keepAwake"]?.GetValue<bool>() == true ? "Keep awake allowed" : "Ask before keeping awake"), "", controls);
        }
        Section("Sessions");
        var sessions = LiveAgentSessions(snapshot).ToList();
        if (sessions.Count == 0) Card("No active sessions", "Agent sessions and command-line jobs appear here while they keep the computer awake.", "");
        foreach (var session in sessions)
        {
            var id = session["session_id"]!.GetValue<string>();
            var status = session["status"]!.GetValue<string>();
            var client = session["client_name"]?.GetValue<string>() ?? "Agent";
            var controls = Buttons();
            if (status == "awaiting_authorization")
                foreach (var (decision, label, spoken) in new[] { ("once", "Allow Once", $"Allow {client} once"), ("deny", "Deny", $"Deny {client}") })
                    controls.Children.Add(ActionButton(label, () => bridge.SendAgentAsync("agent-authorize", id: id, decision: decision), spoken, accent: decision == "once"));
            else
            {
                controls.Children.Add(ActionButton("Cancel session", () => bridge.SendAgentAsync("agent-cancel", id: id), $"Cancel {client} session"));
                if (status == "connection_lost")
                {
                    var menu = new MenuFlyout();
                    var wait = new MenuFlyoutItem { Text = "Wait 30 minutes" };
                    wait.Click += async (_, _) => await bridge.SendAgentAsync("agent-wait", id: id);
                    var end = new MenuFlyoutItem { Text = "End and apply completion action" };
                    end.Click += async (_, _) => await bridge.SendAgentAsync("agent-finish", id: id);
                    menu.Items.Add(wait); menu.Items.Add(end);
                    var resolve = new DropDownButton { Content = "Resolve", Flyout = menu };
                    Microsoft.UI.Xaml.Automation.AutomationProperties.SetName(resolve, $"Resolve {client} session");
                    controls.Children.Add(resolve);
                }
            }
            var details = $"{session["reason"]?.GetValue<string>()}\n{Labels.AgentStatus(status)} · When finished: {Labels.Action(session["completion_action"]?.GetValue<string>())}";
            if (status == "connection_lost")
            {
                var idle = Math.Max(0, (snapshot["agentNow"]?.GetValue<long>() ?? 0) - (session["last_heartbeat"]?.GetValue<long>() ?? 0)) / 60;
                details += $" · Last activity {idle}m ago";
            }
            Card(client, details, client == "Command line" ? "" : "", controls);
        }
    }

    private static IEnumerable<JsonObject> LiveAgentSessions(JsonObject snapshot) =>
        (snapshot["agentSessions"] as JsonArray)?.OfType<JsonObject>()
            .Where(s => s["status"]?.GetValue<string>() is "active" or "connection_lost" or "awaiting_authorization") ?? [];

    /// What the Sessions list shows, without heartbeat times that change on every refresh.
    private static string AgentSessionShape(JsonObject snapshot) => string.Join("|", LiveAgentSessions(snapshot).Select(s =>
        $"{s["session_id"]}:{s["status"]}:{s["completion_action"]}:{(s["status"]?.GetValue<string>() == "connection_lost" ? ((snapshot["agentNow"]?.GetValue<long>() ?? 0) - (s["last_heartbeat"]?.GetValue<long>() ?? 0)) / 60 : 0)}"));
}
