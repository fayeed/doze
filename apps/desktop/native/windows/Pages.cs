using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Doze.SettingsUi;

// Settings pages, built in code from the card helpers in SettingsLayout.cs. Every control
// writes one key of the engine's settings store; the engine's reply redraws the page.
public sealed partial class MainWindow
{
    private bool On(string key) => Flag(Setting(key));
    private int Int(string key, int fallback) => (int)(Number(Setting(key)) ?? fallback);

    private void General()
    {
        Card("Start Doze when I sign in", null, null,
            Switch("Start Doze when I sign in", On("launchAtStartup"), value => _ = Set("launchAtStartup", value), Capability("startupSupported")));
        Section("Tray icon");
        Card("Clicking the tray icon opens", "Right-click always opens the other one.", null,
            Choice("Clicking the tray icon opens", [("panel", "Flyout"), ("menu", "Menu")], Text(Setting("iconClickOpens")) ?? "panel", value => _ = Set("iconClickOpens", value)));
        Card("Show time left in the tray tooltip", null, null,
            Switch("Show time left in the tray tooltip", On("menuBarTime"), value => _ = Set("menuBarTime", value)));
        Section("While keeping awake");
        Card("Keep the display on too", "When off, the screen can turn off while the PC stays awake.", null,
            Switch("Keep the display on too", !On("allowDisplaySleep"), value => _ = Set("allowDisplaySleep", !value)));
        if (Capability("lidClosedSupported"))
            Card("Stay awake with the lid closed", "Only while Doze is keeping your PC awake. Doze sets “When I close the lid” to Do nothing, then puts your setting back.", null,
                Switch("Stay awake with the lid closed", On("lidClosedKeepAwake"), value => _ = Set("lidClosedKeepAwake", value)));
        Card("Stop keeping awake below", "On battery only.", null,
            Choice("Stop keeping awake below", BatteryChoices(Int("batteryFloorPercent", 15)), Int("batteryFloorPercent", 15), value => _ = Set("batteryFloorPercent", value)));
    }

    private static IEnumerable<(int, string)> BatteryChoices(int current) =>
        new[] { 0, 10, 15, 20, 25, 30, 50, current }.Distinct().Order().Select(percent => (percent, percent == 0 ? "Never" : $"{percent}%"));

    private void SessionDefaults()
    {
        Section("Keep Awake");
        Card("Default duration", "Used by “Default” in the tray menu and flyout.", null,
            Choice("Keep Awake default duration", MinuteChoices(Int("defaultAwakeMinutes", 30)), Int("defaultAwakeMinutes", 30), value => _ = Set("defaultAwakeMinutes", value)));
        Card("Remember my last custom duration", null, null,
            Switch("Remember my last custom duration", On("rememberLastCustomDuration"), value => _ = Set("rememberLastCustomDuration", value)));
        Section("Power Timer");
        Card("Default action", null, null,
            Choice("Power Timer default action", ActionChoices, Text(Setting("defaultAction")) ?? "sleep", value => _ = Set("defaultAction", value)));
        Card("Default timer", null, null,
            Choice("Power Timer default duration", MinuteChoices(Int("defaultTimerMinutes", 30)), Int("defaultTimerMinutes", 30), value => _ = Set("defaultTimerMinutes", value)));
        Section("Countdown");
        var snooze = Int("snoozeMinutes", 15);
        Card("Snooze length", null, null,
            Choice("Snooze length", new[] { 5, 10, 15, 20, 30, 45, 60, snooze }.Distinct().Order().Select(m => (m, Labels.Minutes(m))), snooze, value => _ = Set("snoozeMinutes", value)));
        var stay = Number(Setting("stayAwakeMinutes")) is long minutes ? (int)minutes : 0;
        Card("Stay Awake keeps going", "What the Stay Awake button in the countdown does.", null,
            Choice("Stay Awake keeps going", new[] { 0, 30, 60, 120, stay }.Distinct().Order().Select(m => (m, m == 0 ? "Until I stop it" : "For " + Labels.Minutes(m))), stay,
                value => _ = Set("stayAwakeMinutes", value == 0 ? null : value)));
    }

    private const string ActionGuide = "Doze locks the desktop before Sleep or Hibernate. Sleep keeps your session in memory, Hibernate saves it to disk, Shut down closes Windows (apps with unsaved work can stop it), Lock secures your session, and Turn display off switches off the screen.";

    private void AfterPlayback()
    {
        var supported = Capability("audioSupported");
        if (!supported)
            Card("Audio monitoring unavailable", "After Playback and keeping awake while audio plays are unavailable in this build.", null);
        Card("Sleep after playback stops", "Turn on before you start watching; it turns itself off after it runs.", null,
            Switch("Sleep after playback stops", Flag(Session["playbackEnabled"]), value => _ = Send("playback-toggle"), supported));
        var idle = Int("idleSeconds", 600);
        Card("Wait for inactivity", "No keyboard or mouse input for this long.", null,
            Choice("Wait for inactivity", SecondChoices(idle, 60, 120, 300, 600, 900, 1200, 1800, 3600), idle, value => _ = Set("idleSeconds", value), supported));
        Card("Then", null, null,
            Choice("Then", ActionChoices, Text(Setting("playbackAction")) ?? "sleep", value => _ = Set("playbackAction", value), supported));
        Section("Audio");
        Card("Keep awake while audio plays", "Holds the computer awake while sound is playing.", null,
            Switch("Keep awake while audio plays", Flag(Session["whileAudio"]), value => _ = Send("audio-toggle"), supported));
        var monitoring = Flag(Session["whileAudio"]) || Flag(Session["playbackEnabled"]);
        var playing = Flag(Session["audioActive"]);
        Card("Live output level", "Read from the system output. Doze never records audio." + (monitoring ? "" : " Shown while one of the switches above is on."), null,
            OutputLevel(monitoring, playing));
    }

    /// Doze reads whether sound reaches the output, not the sound itself.
    private static FrameworkElement OutputLevel(bool monitoring, bool playing)
    {
        var panel = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12, VerticalAlignment = VerticalAlignment.Center };
        var bars = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 3, Height = 22, VerticalAlignment = VerticalAlignment.Center };
        foreach (var height in new[] { 0.4, 0.8, 0.55, 0.95, 0.35, 0.7, 0.25 })
        {
            var bar = (Border)Microsoft.UI.Xaml.Markup.XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"4\" CornerRadius=\"2\" VerticalAlignment=\"Bottom\" Background=\"{ThemeResource DozeTrayAccentBrush}\" />");
            bar.Height = 22 * (playing ? height : 0.12);
            bar.Opacity = monitoring ? 1 : 0.35;
            bars.Children.Add(bar);
        }
        AutomationProperties.SetAccessibilityView(bars, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        panel.Children.Add(new TextBlock { Text = !monitoring ? "Off" : playing ? "Sound playing" : "Silent", VerticalAlignment = VerticalAlignment.Center });
        panel.Children.Add(bars);
        return panel;
    }

    private void Notifications()
    {
        Section("Final warning");
        var length = Int("countdownSeconds", 300);
        Card("Warning length", "How long the countdown runs before any power action.", null,
            Choice("Warning length", SecondChoices(length, 60, 120, 180, 300, 600, 900), length, value => _ = Set("countdownSeconds", value)));
        Card("Play a sound when it appears", null, null,
            Switch("Play a sound when it appears", On("warningSound"), value => _ = Set("warningSound", value)));
        Card("Show on every display", null, null,
            Switch("Show on every display", On("warningAllDisplays"), value => _ = Set("warningAllDisplays", value)));
        Card("Preview", "Shows the warning without scheduling anything.", null, ActionButton("Preview warning", () => Send("preview")));
        Section("Agents");
        Card("When an agent asks to keep the PC awake", null, null,
            Switch("When an agent asks to keep the PC awake", On("notifyAgentApproval"), value => _ = Set("notifyAgentApproval", value)));
        Card("When all agents have finished", null, null,
            Switch("When all agents have finished", On("notifyAgentsFinished"), value => _ = Set("notifyAgentsFinished", value)));
        Card("When an agent stops checking in", null, null,
            Switch("When an agent stops checking in", On("notifyAgentStalled"), value => _ = Set("notifyAgentStalled", value)));
        Section("Keep Awake");
        Card("When a Keep Awake session ends", null, null,
            Switch("When a Keep Awake session ends", On("notifyKeepAwakeEnded"), value => _ = Set("notifyKeepAwakeEnded", value)));
    }

    private void Agents()
    {
        Card("Let agents keep the PC awake", "Through hooks and the local MCP server.", null,
            Switch("Let agents keep the PC awake", On("agents.enabled"), value => _ = Set("agents.enabled", value)));
        Card("Ask before a new agent holds a lease", null, null,
            Switch("Ask before a new agent holds a lease", On("agents.askBeforeNew"), value => _ = Set("agents.askBeforeNew", value)));
        Card("When agents finish", "After the final warning.", null,
            Choice("When agents finish", ActionChoices.Prepend(("nothing", "Nothing")), Text(Setting("agents.defaultCompletion")) ?? "nothing",
                value => _ = Set("agents.defaultCompletion", value == "nothing" ? null : value)));
        Card("If an agent stops checking in", "Doze stays awake, then lets go without acting.", null, Muted("Up to 30 minutes"));

        Section("Connected agents");
        var links = (snapshot["agentLinks"] as JsonArray)?.OfType<JsonObject>().ToList() ?? [];
        foreach (var link in links)
        {
            var id = Text(link["id"])!;
            var name = Text(link["name"]) ?? id;
            var connected = Flag(link["connected"]);
            var controls = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 12 };
            controls.Children.Add(connected ? Positive("Connected") : Muted(Flag(link["installed"]) ? "Not set up" : "Not installed"));
            controls.Children.Add(ActionButton(connected ? "Remove" : "Connect",
                () => Send("connect-preview", new JsonObject { ["agent"] = id, ["remove"] = connected }),
                (connected ? "Remove " : "Connect ") + name, accent: !connected));
            Card(name, Text(link["method"]), Monogram(Text(link["monogram"]) ?? "?"), controls);
        }
        var tools = Setting("agents.processTools") as JsonArray ?? [];
        var cursor = tools.OfType<JsonObject>().FirstOrDefault(tool => Text(tool["id"]) == "cursor");
        Card("Cursor", "Process detection · no lifecycle events", Monogram("Cu"),
            Switch("Detect Cursor", Flag(cursor?["detect"]), value => _ = Set("agents.processTools", ProcessTools(tools, "cursor", detect: value))));
        if (Flag(cursor?["detect"]))
            Card("Keep the PC awake while Cursor is open", "Cursor has no lifecycle events, so Doze can't tell when it's working.", null,
                Switch("Keep the PC awake while Cursor is open", Flag(cursor?["keepAwake"]), value => _ = Set("agents.processTools", ProcessTools(tools, "cursor", keepAwake: value))));
        var address = Text(snapshot["mcpAddress"]);
        var other = Card("Other MCP clients", (address ?? "Unavailable") + " · loopback only", Monogram("+"),
            ActionButton("Copy config", () => Send("copy-config"), "Copy MCP config"));
        other.Style = Style("CodeCaptionStyle");

        var trusted = (Setting("agents.trusted") as JsonArray)?.Select(Text).OfType<string>().ToList() ?? [];
        var clients = (Setting("agents.clients") as JsonArray)?.OfType<JsonObject>().Where(c => Flag(c["keepAwake"])).ToList() ?? [];
        if (trusted.Count + clients.Count > 0)
        {
            Section("Allowed agents");
            foreach (var id in trusted)
                Card(AgentName(id), "Holds the PC awake without asking.", null,
                    ActionButton("Forget", () => Set("agents.trusted", new JsonArray(trusted.Where(t => t != id).Select(t => (JsonNode)t!).ToArray())), "Forget " + AgentName(id)));
            foreach (var client in clients)
            {
                var name = Text(client["name"]) ?? "MCP client";
                Card(name, "MCP client · holds the PC awake without asking.", null,
                    ActionButton("Forget", () => Send("agent-permission", new JsonObject { ["id"] = Text(client["id"]) }), "Forget " + name));
            }
        }
        Footer("Connect adds Doze's hooks to the tool's own settings file after showing you the change, keeps a timestamped backup beside it, and never touches other entries. Remove takes out exactly what Connect added.");
    }

    private static string AgentName(string id) => id switch
    {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "opencode" => "OpenCode",
        "gemini-cli" => "Gemini CLI",
        "cursor" => "Cursor",
        _ => id.Contains(':') ? id[(id.IndexOf(':') + 1)..] : id
    };

    private static JsonArray ProcessTools(JsonArray tools, string id, bool? detect = null, bool? keepAwake = null)
    {
        var copy = tools.DeepClone().AsArray();
        if (copy.OfType<JsonObject>().FirstOrDefault(tool => Text(tool["id"]) == id) is not JsonObject tool)
            copy.Add(tool = new JsonObject { ["id"] = id, ["detect"] = false, ["keepAwake"] = false });
        if (detect is bool d) tool["detect"] = d;
        if (keepAwake is bool k) tool["keepAwake"] = k;
        return copy;
    }

    private static TextBlock Muted(string text) => new() { Text = text, Style = Style("CardDescriptionStyle"), FontSize = 14, VerticalAlignment = VerticalAlignment.Center };

    private static TextBlock Positive(string text)
    {
        var block = (TextBlock)Microsoft.UI.Xaml.Markup.XamlReader.Load("<TextBlock xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Foreground=\"{ThemeResource SystemFillColorSuccessBrush}\" VerticalAlignment=\"Center\" />");
        block.Text = text;
        return block;
    }

    /// The two-letter tile for an agent.
    private static Border Monogram(string letters)
    {
        var tile = (Border)Microsoft.UI.Xaml.Markup.XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"28\" Height=\"28\" CornerRadius=\"4\" Background=\"{ThemeResource DozeTileBrush}\" />");
        var text = (TextBlock)Microsoft.UI.Xaml.Markup.XamlReader.Load("<TextBlock xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" FontSize=\"11\" FontWeight=\"Bold\" HorizontalAlignment=\"Center\" VerticalAlignment=\"Center\" Foreground=\"{ThemeResource DozeTileTextBrush}\" />");
        text.Text = letters;
        tile.Child = text;
        return tile;
    }

    private void Advanced()
    {
        var cli = Text(snapshot["cliPath"]) ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Doze", "doze-cli.exe");
        var folder = Path.GetDirectoryName(cli.Replace('/', '\\')) ?? "";
        var onPath = CommandLineInstall.OnPath(folder);
        var tool = Card("doze command-line tool", "doze run --then sleep -- <command>", null,
            onPath ? Muted("Added to PATH") : ActionButton("Add to PATH", () => AddToPathAsync(folder), "Add doze to PATH"));
        tool.Style = Style("CodeCaptionStyle");
        var address = Text(snapshot["mcpAddress"]);
        var server = Card("MCP server for agents", (address ?? "Unavailable") + " · loopback only", null,
            Switch("MCP server for agents", On("mcpServerEnabled"), value => _ = Set("mcpServerEnabled", value)));
        server.Style = Style("CodeCaptionStyle");
        Card("How Doze keeps the PC awake", "System power requests, released when you quit. No simulated input.", null,
            ActionButton("Show active requests", ShowAssertionsAsync));
        Section("Diagnostics");
        Card("Local data", "Preferences and diagnostics never leave this PC.", null, ActionButton("Open folder", OpenData, "Open the local data folder"));
        Card("Write diagnostic logs", "Errors are logged on this computer, up to about 256 KB.", null,
            Switch("Write diagnostic logs", On("logging"), value => _ = Set("logging", value)));
        Card("Export a diagnostics report", null, null, ActionButton("Export…", ExportDiagnosticsAsync, "Export a diagnostics report"));
        Card("Reset all settings", null, null, ActionButton("Reset…", ConfirmResetAsync, "Reset all settings"));
    }

    private async Task AddToPathAsync(string folder)
    {
        if (verification) return;
        CommandLineInstall.AddToPath(folder);
        Notify("Added to PATH", "Open a new terminal and run doze help.", InfoBarSeverity.Success);
        KeepScroll(ShowPage);
        await Task.CompletedTask;
    }

    private async Task ShowAssertionsAsync()
    {
        var held = (snapshot["assertions"] as JsonArray)?.Select(Text).OfType<string>().ToList() ?? [];
        var dialog = new ContentDialog
        {
            XamlRoot = Root.XamlRoot, RequestedTheme = Root.ActualTheme, Title = "Active power requests",
            Content = new TextBlock
            {
                Text = held.Count == 0 ? "Doze isn't holding any power requests right now." : string.Join("\n", held),
                TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true
            },
            CloseButtonText = "Close", DefaultButton = ContentDialogButton.Close
        };
        await dialog.ShowAsync();
    }

    private async Task ExportDiagnosticsAsync()
    {
        var picker = new Windows.Storage.Pickers.FileSavePicker { SuggestedFileName = $"Doze diagnostics {DateTime.Now:yyyy-MM-dd}" };
        picker.FileTypeChoices.Add("JSON", [".json"]);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, WinRT.Interop.WindowNative.GetWindowHandle(this));
        if (await picker.PickSaveFileAsync() is { } file)
            await Send("export-diagnostics", new JsonObject { ["path"] = file.Path });
    }

    private void MenuGuide()
    {
        Section("The tray icon");
        Card("Hollow sun", "Normal sleep allowed. Doze isn't holding anything.", Glyph("normal", 20));
        Card("Whole sun", "Keeping awake, for a timer or for agents. Hover for the time left.", Glyph("awake", 20));
        Card("Sun with a dot", "An agent is waiting for your approval.", Glyph("attention", 20));
        Card("Banded sun", "The final warning is counting down.", Glyph("countdown", 20));
        Section("Opening Doze");
        var flyout = Text(Setting("iconClickOpens")) != "menu";
        Card("Click the tray icon", null, null, Muted(flyout ? "Flyout" : "Menu"));
        Card("Right-click the tray icon", null, null, Muted(flyout ? "Menu" : "Flyout"));
        Card("Settings", null, null, Muted("Ctrl+,"));
        Footer(ActionGuide);
    }

    private void About()
    {
        var text = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = "Doze", Style = Style("SubtitleTextBlockStyle") });
        text.Children.Add(new TextBlock { Text = $"Version {Text(snapshot["version"]) ?? "0.2.0"}", Style = Style("CardDescriptionStyle"), FontSize = 14, IsTextSelectionEnabled = true });
        text.Children.Add(new TextBlock { Text = "Your computer knows when it's bedtime.", Style = Style("CardDescriptionStyle"), FontSize = 14 });
        var hero = new Grid { ColumnSpacing = 22 };
        hero.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        hero.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        var icon = Asset("Doze.png", 64, "");
        AutomationProperties.SetAccessibilityView(icon, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        hero.Children.Add(icon);
        Grid.SetColumn(text, 1);
        hero.Children.Add(text);
        group.Children.Add(new Border { Style = Style("SettingsCardStyle"), Padding = new Thickness(20), MinHeight = 120, Child = hero });

        Section("Links");
        foreach (var link in (snapshot["links"] as JsonArray)?.OfType<JsonObject>() ?? [])
        {
            var title = Text(link["title"]) ?? "";
            var url = Text(link["url"]) ?? "";
            var open = new HyperlinkButton { Content = "↗", NavigateUri = Uri.TryCreate(url, UriKind.Absolute, out var uri) ? uri : null };
            AutomationProperties.SetName(open, "Open " + title);
            Card(title, null, null, open);
        }
        Section("Privacy");
        Card("No account · No cloud · No telemetry", "Doze does not record audio or simulate input. Preferences and optional diagnostics stay on this PC.", null);
    }

    private ContentDialog ResetDialog() => new()
    {
        XamlRoot = Root.XamlRoot, RequestedTheme = Root.ActualTheme,
        Title = "Reset all settings?",
        Content = new TextBlock { Text = "Every page returns to its defaults, including Start Doze when I sign in and the agents you allowed. Connected tools keep their hooks.", TextWrapping = TextWrapping.Wrap },
        PrimaryButtonText = "Reset", CloseButtonText = "Cancel", DefaultButton = ContentDialogButton.Close
    };

    private async Task ConfirmResetAsync()
    {
        if (await ResetDialog().ShowAsync() == ContentDialogResult.Primary) await Send("reset");
    }

    /// Shows the exact change to a tool's config before Doze writes it.
    private ContentDialog ConnectDialog(JsonObject change)
    {
        var remove = Flag(change["remove"]);
        var agent = AgentName(Text(change["agent"]) ?? "");
        var body = new StackPanel { Spacing = 12, MaxWidth = 560 };
        body.Children.Add(new TextBlock
        {
            Text = (remove ? $"Doze will remove its hooks from {agent}'s settings. " : $"Doze will add these lines to {agent}'s settings. ")
                + "A timestamped backup is kept beside the file, and nothing else in it changes.",
            TextWrapping = TextWrapping.Wrap
        });
        body.Children.Add(new TextBlock { Text = Text(change["path"]) ?? "", Style = Style("CodeTextStyle") });
        var diff = new TextBlock { Text = Text(change["diff"]) ?? "", Style = Style("CodeTextStyle"), TextWrapping = TextWrapping.NoWrap };
        body.Children.Add(new ScrollViewer
        {
            Content = diff, MaxHeight = 280, HorizontalScrollBarVisibility = ScrollBarVisibility.Auto,
            VerticalScrollBarVisibility = ScrollBarVisibility.Auto, Padding = new Thickness(0, 0, 12, 12)
        });
        if (Text(change["note"]) is string note) body.Children.Add(new TextBlock { Text = note, TextWrapping = TextWrapping.Wrap, Style = Style("CardDescriptionStyle") });
        return new ContentDialog
        {
            XamlRoot = Root.XamlRoot, RequestedTheme = Root.ActualTheme,
            Title = remove ? $"Remove Doze from {agent}?" : $"Connect {agent}?",
            Content = body, PrimaryButtonText = remove ? "Remove" : "Connect", CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Primary
        };
    }

    /// Data returned by a command: a change to confirm, a config to copy, or a message.
    private async void HandleResult(string? command, JsonObject result)
    {
        try
        {
            switch (command)
            {
                case "connect-preview":
                    if (await ConnectDialog(result).ShowAsync() == ContentDialogResult.Primary)
                        await Send("connect-apply", new JsonObject { ["agent"] = Text(result["agent"]), ["remove"] = Flag(result["remove"]), ["token"] = Text(result["token"]) });
                    break;
                case "connect-apply":
                    Notify("Done", Text(result["message"]) ?? "", InfoBarSeverity.Success);
                    break;
                case "copy-config":
                    var data = new Windows.ApplicationModel.DataTransfer.DataPackage();
                    data.SetText(Text(result["config"]) ?? "");
                    Windows.ApplicationModel.DataTransfer.Clipboard.SetContent(data);
                    Notify("MCP config copied", "Paste it into your MCP client's settings. It starts Doze's MCP bridge with this PC's key.", InfoBarSeverity.Success);
                    break;
            }
        }
        catch (Exception error) { Notify("Couldn't complete that", error.Message, InfoBarSeverity.Error); }
    }
}
