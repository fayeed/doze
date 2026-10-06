using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;

namespace Doze.SettingsUi;

// Overview doubles as a control center: everything in the tray menu, with live times.
public sealed partial class MainWindow
{
    private TextBlock? overviewStatus, overviewTimer, awakeLeft, timerLeft, countdownClock;
    private static readonly (int Seconds, string Label)[] Presets = [(900, "15m"), (1800, "30m"), (3600, "1h"), (7200, "2h")];

    private JsonObject Session => snapshot["session"] as JsonObject ?? new JsonObject();
    private static bool Flag(JsonNode? node) => node is JsonValue value && value.TryGetValue<bool>(out var flag) && flag;
    private static long? Number(JsonNode? node) => node is JsonValue value && long.TryParse(value.ToJsonString(), out var number) ? number : null;
    private static string? Text(JsonNode? node) => node is JsonValue value && value.TryGetValue<string>(out var text) ? text : null;

    /// A saved setting by its key, such as "snoozeMinutes" or "agents.askBeforeNew".
    private JsonNode? Setting(string key)
    {
        JsonNode? node = snapshot["settings"];
        foreach (var part in key.Split('.')) node = node?[part];
        return node;
    }

    /// The engine's clock now: its time at the last snapshot plus what has passed since.
    private long EngineNow => (Number(snapshot["now"]) ?? 0) + (long)received.Elapsed.TotalSeconds;

    /// Seconds left until an engine deadline, falling back to the remaining time it reported.
    private long Left(JsonNode? part, string deadlineKey = "deadline", string remainingKey = "remaining")
    {
        if (Number(part?[deadlineKey]) is long deadline) return Math.Max(0, deadline - EngineNow);
        return Math.Max(0, (Number(part?[remainingKey]) ?? 0) - (long)received.Elapsed.TotalSeconds);
    }

    private long AwakeLeft => Number(Session["awakeDeadline"]) is long deadline
        ? Math.Max(0, deadline - EngineNow)
        : Math.Max(0, (Number(Session["awakeRemaining"]) ?? 0) - (long)received.Elapsed.TotalSeconds);

    private bool HasDeadline => Number(Session["awakeRemaining"]) is not null || Session["timer"] is JsonObject || Session["countdown"] is JsonObject;

    private string TimerText()
    {
        if (Session["countdown"] is JsonObject countdown)
            return $"{Labels.Action(Text(countdown["action"]))} in {Labels.Clock(Left(countdown))} · final warning";
        if (Session["timer"] is JsonObject timer)
            return $"{Labels.Action(Text(timer["action"]))} in {Labels.Remaining(Left(timer))}";
        return Text(snapshot["statusDetail"]) ?? "No power action scheduled";
    }

    private string AwakeText() => Number(Session["awakeRemaining"]) is not null
        ? $"{Labels.Remaining(AwakeLeft)} left · ends {DateTime.Now.AddSeconds(AwakeLeft):t}"
        : "Until you stop it";

    private string TimerLeftText() => Session["timer"] is JsonObject timer
        ? $"{Labels.Remaining(Left(timer))} left · runs at {DateTime.Now.AddSeconds(Left(timer)):t}, after the final warning"
        : "";

    private void Overview()
    {
        var session = Session;
        var audio = Capability("audioSupported");
        StatusHeader();

        if (session["countdown"] is JsonObject countdown)
        {
            Section("Final warning");
            countdownClock = new TextBlock
            {
                Text = Labels.Clock(Left(countdown)), Style = Style("SubtitleTextBlockStyle"),
                FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("Segoe UI Variable Display")
            };
            countdownClock.SetValue(TextBlock.FontSizeProperty, 28.0);
            Card(Labels.Action(Text(countdown["action"])) + " in", Labels.CountdownSource(Text(countdown["source"])), Tinted("", "DozeRedBrush"), countdownClock);
            var snooze = Number(Setting("snoozeMinutes")) ?? 15;
            Card("Choose what happens", $"Snooze waits {snooze} more minutes. Stay Awake keeps the computer awake instead of acting.", "",
                Buttons(ActionButton($"Snooze {snooze} min", () => Send("snooze"), "Snooze"), ActionButton("Stay Awake", () => Send("stay-awake")),
                    ActionButton("Cancel", () => Send("cancel"), "Cancel the action", accent: true)));
        }
        else countdownClock = null;

        Section("Keep Awake");
        if (Flag(session["awake"]))
        {
            var controls = Buttons();
            if (Number(session["awakeRemaining"]) is not null) controls.Children.Add(ActionButton("Extend 15 minutes", () => Send("extend")));
            controls.Children.Add(ActionButton("Stop", () => Send("stop-awake"), "Stop keeping awake"));
            awakeLeft = Card("Keeping awake", AwakeText(), Tinted("", "DozeOrangeBrush"), controls);
            timerLeft = null;
        }
        else
        {
            var controls = Buttons(Presets.Select(preset => (FrameworkElement)ActionButton(preset.Label,
                () => Send("awake", new JsonObject { ["seconds"] = preset.Seconds }), "Keep awake for " + preset.Label)).ToArray());
            controls.Children.Add(ActionButton("Indefinitely", () => Send("awake-forever"), "Keep awake indefinitely"));
            controls.Children.Add(More("More ways to keep awake", ("Custom duration…", "awakeDuration"), ("Until a specific time…", "awakeTime")));
            StackedCard("Keep awake for", "Normal sleep settings apply again afterwards.", controls);
            awakeLeft = null;
        }
        Card("Keep awake while audio plays", audio ? "Holds the computer awake while sound is playing." : "Audio monitoring is unavailable in this build.", null,
            Switch("Keep awake while audio plays", Flag(session["whileAudio"]), value => _ = Send("audio-toggle"), audio));

        Section("Power Timer");
        var selected = Text(session["selectedAction"]) ?? Text(Setting("defaultAction")) ?? "sleep";
        Card("Action", "Used by the timer presets here and in the tray menu.", null,
            Choice("Power Timer action", ActionChoices, selected, value => _ = Send("select-action", new JsonObject { ["action"] = value })));
        if (session["timer"] is JsonObject timer)
        {
            timerLeft = Card(Labels.Action(Text(timer["action"])) + " scheduled", TimerLeftText(), Tinted("", "DozeBlueBrush"),
                ActionButton("Stop timer", () => Send("stop-timer")));
        }
        else
        {
            var name = Labels.Action(selected);
            var controls = Buttons(Presets.Select(preset => (FrameworkElement)ActionButton(preset.Label,
                () => Send("timer", new JsonObject { ["seconds"] = preset.Seconds, ["action"] = selected }), $"{name} in {preset.Label}")).ToArray());
            controls.Children.Add(More("More timer options", ("Custom duration…", "timerDuration"), ("At a specific time…", "timerTime")));
            StackedCard("Start a timer", $"{name} after the time you choose, following a final warning.", controls);
            timerLeft = null;
        }
        var playbackAction = Labels.Action(Text(Setting("playbackAction")) ?? "sleep");
        var playback = Flag(session["playbackEnabled"]);
        Card($"{playbackAction} after playback stops",
            !audio ? "Audio monitoring is unavailable in this build."
                : playback ? Labels.PlaybackPhase(Text(session["playbackPhase"]))
                : "Turn on before you start watching; it turns itself off after it runs.",
            null, Switch("Sleep after playback stops", playback, value => _ = Send("playback-toggle"), audio));

        Section("Agents");
        var agents = snapshot["agents"] as JsonObject;
        var working = Number(agents?["working"]) ?? 0;
        var pending = Number(agents?["pending"]) ?? 0;
        var connected = (snapshot["agentLinks"] as JsonArray)?.OfType<JsonObject>().Where(link => Flag(link["connected"]))
            .Select(link => Text(link["name"])).OfType<string>().ToList() ?? [];
        var title = pending > 0 ? $"{pending} {(pending == 1 ? "agent asks" : "agents ask")} to keep the PC awake"
            : working > 0 ? $"{working} {(working == 1 ? "agent" : "agents")} working"
            : "No agents running";
        Card(title, connected.Count > 0 ? $"{Labels.List(connected)} {(connected.Count == 1 ? "is" : "are")} connected." : "Connect Claude Code, Codex, OpenCode or Gemini CLI.",
            Tinted("", "DozeTealBrush"), ActionButton("Agent settings", () => { SelectPage("Agents"); return Task.CompletedTask; }));
    }

    /// A card with its buttons on a row underneath, as the design's preset rows.
    private void StackedCard(string title, string description, FrameworkElement controls)
    {
        collecting?.Add((title, description, controls));
        var text = new StackPanel { Spacing = 1 };
        text.Children.Add(new TextBlock { Text = title, Style = Style("CardTitleStyle") });
        text.Children.Add(new TextBlock { Text = description, Style = Style("CardDescriptionStyle") });
        var stack = new StackPanel { Spacing = 10 };
        stack.Children.Add(text);
        stack.Children.Add(controls);
        group.Children.Add(new Border { Style = Style("SettingsCardStyle"), Child = stack });
    }

    private void StatusHeader()
    {
        var session = Session;
        var holding = Flag(session["holdingAwake"]) || session["countdown"] is JsonObject;
        var tile = (Border)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"56\" Height=\"56\" CornerRadius=\"6\" Background=\"{ThemeResource SubtleFillColorSecondaryBrush}\" />");
        tile.Child = Glyph(Text(snapshot["iconState"]) ?? (holding ? "awake" : "normal"), 28);
        overviewStatus = new TextBlock { Text = Text(snapshot["statusShort"]) ?? Text(snapshot["status"]) ?? "Normal sleep allowed", Style = Style("SubtitleTextBlockStyle"), TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        overviewTimer = new TextBlock { Text = TimerText(), Style = Style("CardDescriptionStyle"), FontSize = 14 };
        var text = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(overviewStatus);
        text.Children.Add(overviewTimer);
        var row = new Grid { ColumnSpacing = 16 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.Children.Add(tile);
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var card = new Border { Style = Style("SettingsCardStyle"), Child = row, Padding = new Thickness(16), MinHeight = 88 };
        AutomationProperties.SetName(card, "Status");
        group.Children.Add(card);
    }

    private DropDownButton More(string name, params (string Label, string View)[] items)
    {
        var menu = new MenuFlyout();
        foreach (var (label, view) in items)
        {
            var item = new MenuFlyoutItem { Text = label };
            item.Click += (_, _) => openTimer?.Invoke(view);
            menu.Items.Add(item);
        }
        var button = new DropDownButton { Content = "More", Flyout = menu };
        AutomationProperties.SetName(button, name);
        return button;
    }

    /// Updates times in place once a second while a countdown or timer is visible.
    private void UpdateLive()
    {
        if (page != "Overview") return;
        if (overviewStatus is not null) overviewStatus.Text = Text(snapshot["statusShort"]) ?? "Normal sleep allowed";
        if (overviewTimer is not null) overviewTimer.Text = TimerText();
        if (awakeLeft is not null) awakeLeft.Text = AwakeText();
        if (timerLeft is not null) timerLeft.Text = TimerLeftText();
        if (countdownClock is not null && Session["countdown"] is JsonObject countdown) countdownClock.Text = Labels.Clock(Left(countdown));
    }
}
