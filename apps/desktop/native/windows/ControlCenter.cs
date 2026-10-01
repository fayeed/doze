using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Markup;

namespace Doze.SettingsUi;

// Overview doubles as a control center: everything in the tray menu, with live times.
public sealed partial class MainWindow
{
    private TextBlock? overviewStatus, overviewTimer, overviewEvent, awakeLeft, timerLeft, countdownClock;
    private static readonly (int Seconds, string Label)[] Presets = [(900, "15m"), (1800, "30m"), (3600, "1h"), (7200, "2h")];

    private JsonObject Session => snapshot["session"] as JsonObject ?? new JsonObject();
    private static bool Flag(JsonNode? node) => node is JsonValue value && value.TryGetValue<bool>(out var flag) && flag;
    private static long? Number(JsonNode? node) => node is JsonValue value && long.TryParse(value.ToJsonString(), out var number) ? number : null;
    private static string? Text(JsonNode? node) => node is JsonValue value && value.TryGetValue<string>(out var text) ? text : null;

    private bool HasDeadline => Number(Session["awakeRemaining"]) is not null || Session["timer"] is JsonObject || Session["countdown"] is JsonObject;

    /// Everything that changes which controls Overview shows; times are excluded.
    private static string OverviewShape(JsonObject snapshot)
    {
        var s = snapshot["session"] as JsonObject ?? new JsonObject();
        return string.Join("|", Flag(s["awake"]), Number(s["awakeRemaining"]) is null, Flag(s["whileAudio"]), Flag(s["playbackEnabled"]),
            Text(s["playbackPhase"]), Text(s["selectedAction"]), Text(s["timer"]?["action"]), Text(s["countdown"]?["action"]),
            Text(s["countdown"]?["source"]), LastEvent(snapshot) is null, snapshot["audioSupported"]?.ToJsonString(),
            snapshot["actions"]?.ToJsonString(), snapshot["settings"]?["playbackAction"]?.ToJsonString());
    }

    private static string? LastEvent(JsonObject snapshot)
    {
        var message = Text(snapshot["session"]?["message"]);
        return string.IsNullOrWhiteSpace(message) || message.StartsWith("Doze skill", StringComparison.Ordinal) ? null : message;
    }

    private string TimerText()
    {
        if (snapshot["session"] is not JsonObject session) return Text(snapshot["timerStatus"]) ?? "No power action scheduled";
        if (session["countdown"] is JsonObject countdown)
            return $"{Labels.Action(Text(countdown["action"]))} in {Labels.Clock(Number(countdown["remaining"]) ?? 0)} · final warning";
        if (session["timer"] is JsonObject timer)
            return $"{Labels.Action(Text(timer["action"]))} in {Labels.Remaining(Number(timer["remaining"]) ?? 0)}";
        return "No power action scheduled";
    }

    private string AwakeText() => Number(Session["awakeRemaining"]) is long left
        ? $"{Labels.Remaining(left)} left · ends {DateTime.Now.AddSeconds(left):t}"
        : "Until you stop it";

    private string TimerLeftText() => Number(Session["timer"]?["remaining"]) is long left
        ? $"{Labels.Remaining(left)} left · runs at {DateTime.Now.AddSeconds(left):t}, after the final warning"
        : "";

    private void Overview()
    {
        var session = Session;
        var actions = Actions.ToList();
        var audio = Capability("audioSupported");
        StatusHeader();

        if (session["countdown"] is JsonObject countdown)
        {
            Section("Final warning");
            countdownClock = new TextBlock
            {
                Text = Labels.Clock(Number(countdown["remaining"]) ?? 0), Style = Style("SubtitleTextBlockStyle"),
                FontFamily = new Microsoft.UI.Xaml.Media.FontFamily("Segoe UI Variable Display")
            };
            countdownClock.SetValue(TextBlock.FontSizeProperty, 28.0);
            Card(Labels.Action(Text(countdown["action"])) + " in", Labels.CountdownSource(Text(countdown["source"])), Tinted("", "DozeRedBrush"), countdownClock);
            Card("Choose what happens", "Snooze waits 15 more minutes. Stay Awake keeps the computer awake instead of acting.", "",
                Buttons(ActionButton("Snooze 15 minutes", () => Send("snooze")), ActionButton("Stay Awake", () => Send("stay-awake")),
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
            Card("Keep awake for", "Normal sleep settings apply again afterwards.", Tinted("", "DozeOrangeBrush"), controls);
            awakeLeft = null;
        }
        Card("Keep awake while audio plays", audio ? "Holds the computer awake while sound is playing." : "Audio monitoring is unavailable in this build.", "",
            Switch("Keep awake while audio plays", Flag(session["whileAudio"]), value => _ = Send("audio-toggle"), audio));

        Section("Power Timer");
        var selected = Text(session["selectedAction"]) ?? draft.DefaultAction;
        Card("Action", "Used by the timer presets here and in the tray menu.", "",
            Choice("Power Timer action", ActionChoices, selected, value => _ = Send("select-action", new JsonObject { ["action"] = value })));
        if (session["timer"] is JsonObject timer)
        {
            timerLeft = Card(Labels.Action(Text(timer["action"])) + " scheduled", TimerLeftText(), Tinted("", "DozeBlueBrush"),
                ActionButton("Stop timer", () => Send("stop-timer")));
        }
        else
        {
            var name = Labels.Action(selected);
            var controls = Buttons(Presets.Select(preset => (FrameworkElement)ActionButton(preset.Label,
                () => Send("timer", new JsonObject { ["seconds"] = preset.Seconds, ["action"] = selected }), $"{name} in {preset.Label}")).ToArray());
            controls.Children.Add(More("More timer options", ("Custom duration…", "timerDuration"), ("At a specific time…", "timerTime")));
            Card("Start a timer", $"{name} after the time you choose, following a final warning.", Tinted("", "DozeBlueBrush"), controls);
            timerLeft = null;
        }
        var playbackAction = Labels.Action(snapshot["settings"]?["playbackAction"]?.GetValue<string>() ?? "sleep");
        var playback = Flag(session["playbackEnabled"]);
        Card($"{playbackAction} after playback stops",
            !audio ? "Audio monitoring is unavailable in this build."
                : playback ? Labels.PlaybackPhase(Text(session["playbackPhase"]))
                : "Turn on before you start watching; it turns itself off after it runs.",
            "", Switch($"{playbackAction} after playback stops", playback, value => _ = Send("playback-toggle"), audio));

        Section("Related");
        Card("Preview the final warning", "Try Cancel and Snooze without scheduling a power action.", "", ActionButton("Preview", () => Send("preview"), "Preview the final warning"));
        Card("Session defaults", "Default durations, the default action and display sleep.", "",
            ActionButton("Open", () => { SelectPage("Session defaults"); return Task.CompletedTask; }, "Open session defaults"));
    }

    private void StatusHeader()
    {
        var session = Session;
        var holding = Flag(session["holdingAwake"]) || session["countdown"] is JsonObject;
        var tile = (Border)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"56\" Height=\"56\" CornerRadius=\"8\" Background=\"{ThemeResource SubtleFillColorSecondaryBrush}\" />");
        tile.Child = Tinted(session["countdown"] is JsonObject ? "" : holding ? "" : "", holding ? "DozeOrangeBrush" : "DozeIndigoBrush", 28);
        ((FrameworkElement)tile.Child).HorizontalAlignment = HorizontalAlignment.Center;
        overviewStatus = new TextBlock { Text = Text(snapshot["status"]) ?? "Normal sleep allowed", Style = Style("SubtitleTextBlockStyle"), TextWrapping = TextWrapping.Wrap, IsTextSelectionEnabled = true };
        overviewTimer = new TextBlock { Text = TimerText(), Style = Style("CardDescriptionStyle"), FontSize = 14 };
        var lastEvent = LastEvent(snapshot);
        overviewEvent = new TextBlock { Text = "Last event: " + lastEvent, Style = Style("CardDescriptionStyle"), IsTextSelectionEnabled = true, Visibility = lastEvent is null ? Visibility.Collapsed : Visibility.Visible };
        var text = new StackPanel { Spacing = 2, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(overviewStatus);
        text.Children.Add(overviewTimer);
        text.Children.Add(overviewEvent);
        var row = new Grid { ColumnSpacing = 16 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.Children.Add(tile);
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var card = new Border { Style = Style("SettingsCardStyle"), Child = row, Padding = new Thickness(16) };
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

    private void UpdateOverview()
    {
        if (overviewStatus is not null) overviewStatus.Text = Text(snapshot["status"]) ?? "Normal sleep allowed";
        if (overviewTimer is not null) overviewTimer.Text = TimerText();
        if (overviewEvent is not null && LastEvent(snapshot) is string lastEvent) overviewEvent.Text = "Last event: " + lastEvent;
        if (awakeLeft is not null) awakeLeft.Text = AwakeText();
        if (timerLeft is not null) timerLeft.Text = TimerLeftText();
        if (countdownClock is not null && Number(Session["countdown"]?["remaining"]) is long remaining) countdownClock.Text = Labels.Clock(remaining);
    }
}
