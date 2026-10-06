using System.Runtime.InteropServices;
using System.Text.Json.Nodes;
using Microsoft.UI.Composition.SystemBackdrops;
using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Automation.Peers;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Markup;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Animation;
using Windows.Graphics;
using Windows.UI.ViewManagement;

namespace Doze.SettingsUi;

/// The tray flyout, modelled on Windows 11 Quick Settings: a status line, approval requests,
/// a grid of tiles whose › half opens a page inside the flyout, the working agents, and a
/// footer. It opens above the tray icon, closes when it loses focus or on Esc, and keeps
/// nothing rendered while hidden.
public sealed partial class TrayFlyout : Window
{
    private readonly Func<string, JsonObject?, Task> send;
    private readonly Action<string> openTimer;
    private readonly Grid root = new() { Width = 360 };
    private readonly ContentPresenter body = new();
    private readonly UISettings system = new();
    private readonly DispatcherTimer clock = new() { Interval = TimeSpan.FromSeconds(1) };
    private readonly System.Diagnostics.Stopwatch received = System.Diagnostics.Stopwatch.StartNew();
    private JsonObject snapshot = new();
    private JsonObject? anchor;
    private string subpage = "";
    private string theme = "system";
    private DateTime hiddenAt = DateTime.MinValue;
    private long? chosenAwake;
    private TextBlock? statusDetail, countdownText;
    private readonly List<(TextBlock Text, JsonObject Session)> workingPills = [];
    private readonly bool verification;

    public TrayFlyout(Func<string, JsonObject?, Task> send, Action<string> openTimer, bool verification = false)
    {
        this.send = send;
        this.openTimer = openTimer;
        this.verification = verification;
        Title = "Doze";
        root.Children.Add(body);
        root.RenderTransform = new TranslateTransform();
        Content = root;
        SystemBackdrop = new DesktopAcrylicBackdrop();
        AppWindow.IsShownInSwitchers = false;
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.SetBorderAndTitleBar(false, false);
            presenter.IsResizable = false;
            presenter.IsMaximizable = false;
            presenter.IsMinimizable = false;
            presenter.IsAlwaysOnTop = true;
        }
        Rounded();
        Activated += (_, args) =>
        {
            if (args.WindowActivationState == WindowActivationState.Deactivated && AppWindow.IsVisible && !verification) Dismiss();
        };
        var escape = new KeyboardAccelerator { Key = Windows.System.VirtualKey.Escape };
        escape.Invoked += (_, args) =>
        {
            args.Handled = true;
            if (subpage != "") Navigate("");
            else Dismiss();
        };
        root.KeyboardAccelerators.Add(escape);
        clock.Tick += (_, _) => Tick();
        system.ColorValuesChanged += (_, _) => DispatcherQueue.TryEnqueue(() => ApplyTheme(theme));
        AppWindow.Closing += (_, args) => { args.Cancel = !verification; Dismiss(); };
    }

    public bool IsOpen => AppWindow.IsVisible;

    public void SetTheme(string value)
    {
        theme = value;
        ApplyTheme(value);
    }

    private void ApplyTheme(string value) => WindowAppearance.Apply(root, value);
    public void StopAppearance() => WindowAppearance.Stop(root);

    /// Opens at the tray icon, or closes when it is already open (a second click on the icon).
    public void Open(JsonObject message)
    {
        if (message["snapshot"] is JsonObject next) Update(next, rebuild: false);
        anchor = message["anchor"] as JsonObject;
        if (AppWindow.IsVisible) { Dismiss(); return; }
        // The click that took focus away from an open flyout already closed it.
        if ((DateTime.UtcNow - hiddenAt).TotalMilliseconds < 350) return;
        subpage = "";
        ApplyTheme(theme);
        Render();
        Place();
        AppWindow.Show(true);
        Activate();
        Animate();
        FocusFirst();
        clock.Start();
    }

    public void Receive(JsonObject message)
    {
        if (message["snapshot"] is JsonObject next) Update(next, rebuild: AppWindow.IsVisible);
    }

    private void Update(JsonObject next, bool rebuild)
    {
        foreach (var key in new[] { "agentLinks" })
            if (next[key] is null && snapshot[key] is JsonNode kept) next[key] = kept.DeepClone();
        var shape = Shape(snapshot);
        snapshot = next;
        received.Restart();
        if (!rebuild) return;
        if (Shape(next) != shape)
        {
            var focus = FocusedName();
            Render();
            Resize();
            Refocus(focus);
        }
        else Tick();
    }

    private static string Shape(JsonObject from)
    {
        var copy = from.DeepClone().AsObject();
        foreach (var key in new[] { "now", "agentNow", "timerStatus", "status", "statusDetail", "assertions" }) copy.Remove(key);
        if (copy["session"] is JsonObject s)
        {
            s.Remove("awakeRemaining");
            if (s["timer"] is JsonObject t) t.Remove("remaining");
            if (s["countdown"] is JsonObject c) c.Remove("remaining");
        }
        return copy.ToJsonString();
    }

    private void Dismiss()
    {
        if (!AppWindow.IsVisible) return;
        hiddenAt = DateTime.UtcNow;
        clock.Stop();
        AppWindow.Hide();
        // Hidden, nothing stays alive in the tree.
        body.Content = null;
        workingPills.Clear();
    }

    // ---------- Data ----------

    private JsonObject Session => snapshot["session"] as JsonObject ?? new JsonObject();
    private static bool Flag(JsonNode? node) => node is JsonValue value && value.TryGetValue<bool>(out var flag) && flag;
    private static long? Number(JsonNode? node) => node is JsonValue value && long.TryParse(value.ToJsonString(), out var number) ? number : null;
    private static string? Text(JsonNode? node) => node is JsonValue value && value.TryGetValue<string>(out var text) ? text : null;
    private JsonNode? Setting(string key)
    {
        JsonNode? node = snapshot["settings"];
        foreach (var part in key.Split('.')) node = node?[part];
        return node;
    }
    private long EngineNow => (Number(snapshot["now"]) ?? 0) + (long)received.Elapsed.TotalSeconds;
    private long Left(long? deadline, long? remaining) => deadline is long d
        ? Math.Max(0, d - EngineNow)
        : Math.Max(0, (remaining ?? 0) - (long)received.Elapsed.TotalSeconds);
    private DateTime WallClock(long engineSeconds) => DateTime.Now.AddSeconds(engineSeconds - EngineNow);
    private IEnumerable<JsonObject> Sessions => (snapshot["agents"]?["sessions"] as JsonArray)?.OfType<JsonObject>() ?? [];
    private IEnumerable<string> Actions => (snapshot["actions"] as JsonArray)?.Select(Text).OfType<string>() ?? [];
    private Task Send(string command, JsonObject? fields = null) => send(command, fields);
    private Task Set(string key, JsonNode? value) => Send("set", new JsonObject { ["key"] = key, ["value"] = value });

    // ---------- Pages ----------

    private void Render()
    {
        workingPills.Clear();
        statusDetail = countdownText = null;
        body.Content = subpage switch
        {
            "keep" => KeepAwakePage(),
            "timer" => TimerPage(),
            "agents" => AgentsPage(),
            "countdown" => CountdownPage(),
            _ => MainPage(),
        };
    }

    private void Navigate(string page)
    {
        var from = subpage;
        subpage = page;
        Render();
        Resize();
        if (page == "") Refocus("More " + PageTitle(from));
        else FocusFirst();
    }

    private static string PageTitle(string page) => page switch
    {
        "keep" => "Keep awake",
        "timer" => "Power timer",
        "agents" => "Agents",
        "countdown" => "Countdown",
        _ => ""
    };

    private FrameworkElement MainPage()
    {
        var stack = new StackPanel { Spacing = 16, Padding = new Thickness(16) };
        stack.Children.Add(StatusLine());
        foreach (var request in Sessions.Where(s => Text(s["state"]) == "needsApproval"))
            stack.Children.Add(Approval(request));
        stack.Children.Add(Tiles());
        var active = Sessions.Where(s => Text(s["state"]) is "working" or "idle").ToList();
        if (active.Count > 0) stack.Children.Add(AgentList(active, compact: true));
        return Framed(stack, Footer(BatteryText(), ("Quick settings", "", () => Send("menu", new JsonObject { ["name"] = "quick" })),
            ("Settings", "", () => OpenSettings("overview"))));
    }

    private FrameworkElement StatusLine()
    {
        var row = new Grid { ColumnSpacing = 12 };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var state = Text(snapshot["iconState"]) ?? "normal";
        var glyph = Glyphs.Create(state, 20, state == "normal" ? "{ThemeResource TextFillColorSecondaryBrush}" : "{ThemeResource DozeTrayAccentBrush}");
        glyph.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(glyph);
        var text = new StackPanel();
        text.Children.Add(new TextBlock { Text = Text(snapshot["statusShort"]) ?? "Normal sleep allowed", FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap });
        statusDetail = Caption(StatusDetail());
        text.Children.Add(statusDetail);
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var holding = Flag(Session["awake"]) || Flag(Session["whileAudio"]) || Number(snapshot["agents"]?["working"]) > 0;
        if (Session["countdown"] is JsonObject)
        {
            var stack = new StackPanel { Spacing = 10 };
            stack.Children.Add(row);
            var snooze = Number(Setting("snoozeMinutes")) ?? 15;
            var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            buttons.Children.Add(Button($"Snooze {snooze} min", () => Send("snooze"), "Snooze"));
            buttons.Children.Add(Button("Cancel", () => Send("cancel"), "Cancel the action"));
            buttons.Children.Add(Button("Stay Awake", () => Send("stay-awake"), accent: true));
            stack.Children.Add(buttons);
            AutomationProperties.SetName(stack, "Status");
            return stack;
        }
        if (holding)
        {
            var stop = Button("Stop", StopEverything, "Stop keeping awake");
            Grid.SetColumn(stop, 2);
            row.Children.Add(stop);
        }
        AutomationProperties.SetName(row, "Status");
        return row;
    }

    /// Stop ends manual keep-awake and audio keep-awake, and releases working agents.
    private async Task StopEverything()
    {
        if (Flag(Session["awake"]) || Flag(Session["whileAudio"])) await Send("stop-awake");
        foreach (var session in Sessions.Where(s => Flag(s["holdsAssertion"])))
            await Send("agent-release", new JsonObject { ["id"] = Text(session["id"]) });
    }

    private string StatusDetail()
    {
        if (Session["countdown"] is JsonObject countdown)
        {
            var left = Left(Number(countdown["deadline"]), Number(countdown["remaining"]));
            return $"{Labels.Action(Text(countdown["action"]))} in {Labels.Clock(left)}";
        }
        var working = Sessions.Where(s => Flag(s["holdsAssertion"])).ToList();
        if (working.Count > 0)
        {
            var since = working.Min(s => Number(s["startedAt"]) ?? EngineNow);
            var names = working.Select(s => Text(s["name"])).Distinct().ToList();
            return $"For {(names.Count == 1 ? names[0] : $"{working.Count} agents")} · since {WallClock(since):t}";
        }
        if (Flag(Session["awake"]))
            return Number(Session["awakeRemaining"]) is not null
                ? $"{Labels.Remaining(Left(Number(Session["awakeDeadline"]), Number(Session["awakeRemaining"])))} left"
                : "Until you stop it";
        if (Session["timer"] is JsonObject timer)
            return $"{Labels.Action(Text(timer["action"]))} in {Labels.Remaining(Left(Number(timer["deadline"]), Number(timer["remaining"])))}";
        return Text(snapshot["statusDetail"]) ?? "No power action scheduled";
    }

    /// An agent asking to hold the PC awake, styled like an InfoBar.
    private FrameworkElement Approval(JsonObject request)
    {
        var id = Text(request["id"]);
        var name = Text(request["name"]) ?? "An agent";
        var border = (Border)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" CornerRadius=\"4\" Padding=\"12\" BorderThickness=\"1\" Background=\"{ThemeResource DozeInfoBackgroundBrush}\" BorderBrush=\"{ThemeResource DozeInfoBorderBrush}\" />");
        var stack = new StackPanel { Spacing = 10 };
        var top = new Grid { ColumnSpacing = 10 };
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        top.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        top.Children.Add(Monogram(Text(request["monogram"]) ?? "?"));
        var text = new StackPanel();
        text.Children.Add(new TextBlock { Text = $"{name} wants to keep your PC awake", FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap });
        var parts = new List<string>();
        if (Text(request["project"]) is string project) parts.Add(project);
        if (Text(request["task"]) is string task) parts.Add($"“{task}”");
        parts.Add("then " + Labels.Action(Text(request["completionAction"]) ?? Text(Setting("agents.defaultCompletion")) ?? "normal"));
        text.Children.Add(Caption(string.Join(" · ", parts)));
        Grid.SetColumn(text, 1);
        top.Children.Add(text);
        stack.Children.Add(top);
        var buttons = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, HorizontalAlignment = HorizontalAlignment.Right };
        buttons.Children.Add(Button("Allow", () => Send("agent-allow", new JsonObject { ["id"] = id }), $"Allow {name}", accent: true));
        buttons.Children.Add(Button("Deny", () => Send("agent-deny", new JsonObject { ["id"] = id }), $"Deny {name}"));
        stack.Children.Add(buttons);
        border.Child = stack;
        AutomationProperties.SetName(border, $"{name} wants to keep your PC awake");
        AutomationProperties.SetLiveSetting(border, AutomationLiveSetting.Polite);
        return border;
    }

    /// Three columns of Quick Settings tiles. Arrow keys move between them; Space and Enter
    /// toggle; the › half opens the tile's page.
    private FrameworkElement Tiles()
    {
        var grid = new Grid { ColumnSpacing = 12, RowSpacing = 14, XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled, TabFocusNavigation = KeyboardNavigationMode.Once };
        for (var i = 0; i < 3; i++) grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.RowDefinitions.Add(new RowDefinition());
        grid.RowDefinitions.Add(new RowDefinition());
        var audio = Flag(snapshot["audioSupported"]);
        var timer = Session["timer"] as JsonObject;
        var countdown = Session["countdown"] as JsonObject;
        var working = Number(snapshot["agents"]?["working"]) ?? 0;
        var pending = Number(snapshot["agents"]?["pending"]) ?? 0;
        var action = Labels.Action(Text(Session["selectedAction"]) ?? Text(Setting("defaultAction")) ?? "sleep");
        var tiles = new[]
        {
            Tile("Keep awake", null, "", Flag(Session["awake"]), () => Send(Flag(Session["awake"]) ? "stop-awake" : "awake-default"), "keep"),
            Tile("Awake while\naudio plays", null, "", Flag(Session["whileAudio"]), () => Send("audio-toggle"), null, audio),
            Tile("Sleep after\nplayback", null, "", Flag(Session["playbackEnabled"]), () => Send("playback-toggle"), null, audio),
            Tile("Power timer", timer is null ? action : $"{action} · {Labels.Remaining(Left(Number(timer["deadline"]), Number(timer["remaining"])))}", "",
                timer is not null, () => Send(timer is null ? "timer-default" : "stop-timer"), "timer"),
            Tile("Agents", pending > 0 ? $"{pending} asking" : working > 0 ? $"{working} working" : "None running", "",
                Flag(Setting("agents.enabled")), () => Set("agents.enabled", !Flag(Setting("agents.enabled"))), "agents"),
            Tile("Countdown", countdown is null ? Labels.Minutes((int)((Number(Setting("countdownSeconds")) ?? 300) / 60)).Replace("minutes", "min").Replace("minute", "min") : "Running",
                "", countdown is not null, () =>
                {
                    // Off, the countdown tile opens its page; on, it cancels the warning.
                    if (countdown is not null) return Send("cancel");
                    Navigate("countdown");
                    return Task.CompletedTask;
                }, "countdown"),
        };
        for (var i = 0; i < tiles.Length; i++)
        {
            Grid.SetColumn(tiles[i], i % 3);
            Grid.SetRow(tiles[i], i / 3);
            grid.Children.Add(tiles[i]);
        }
        return grid;
    }

    private FrameworkElement Tile(string label, string? caption, string glyph, bool on, Func<Task> toggle, string? page, bool enabled = true)
    {
        var name = label.Replace('\n', ' ');
        var stack = new StackPanel { Spacing = 7 };
        var shell = new Grid { Height = 48 };
        shell.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        if (page is not null) shell.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(28) });
        var main = new ToggleButton
        {
            IsChecked = on, IsEnabled = enabled, HorizontalAlignment = HorizontalAlignment.Stretch, VerticalAlignment = VerticalAlignment.Stretch,
            Content = new FontIcon { Glyph = glyph, FontSize = 16 },
            CornerRadius = page is null ? new CornerRadius(4) : new CornerRadius(4, 0, 0, 4), Padding = new Thickness(0)
        };
        AutomationProperties.SetName(main, name);
        if (caption is not null) AutomationProperties.SetHelpText(main, caption);
        main.Click += async (_, _) =>
        {
            // The engine's reply sets the real state; the button never drifts from it.
            main.IsChecked = on;
            try { await toggle(); } catch { }
        };
        shell.Children.Add(main);
        if (page is not null)
        {
            var more = new Button
            {
                Content = new FontIcon { Glyph = "", FontSize = 10 }, Padding = new Thickness(0),
                HorizontalAlignment = HorizontalAlignment.Stretch, VerticalAlignment = VerticalAlignment.Stretch,
                CornerRadius = new CornerRadius(0, 4, 4, 0), IsEnabled = enabled
            };
            if (on) more.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
            AutomationProperties.SetName(more, "More " + PageTitle(page));
            more.Click += (_, _) => Navigate(page);
            Grid.SetColumn(more, 1);
            shell.Children.Add(more);
        }
        stack.Children.Add(shell);
        var text = new TextBlock { Text = label, FontSize = 12, TextAlignment = TextAlignment.Center, HorizontalAlignment = HorizontalAlignment.Center, TextWrapping = TextWrapping.Wrap, LineHeight = 15 };
        AutomationProperties.SetAccessibilityView(text, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        stack.Children.Add(text);
        if (caption is not null)
        {
            var sub = Caption(caption);
            sub.HorizontalAlignment = HorizontalAlignment.Center;
            sub.TextAlignment = TextAlignment.Center;
            AutomationProperties.SetAccessibilityView(sub, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
            stack.Children.Add(sub);
        }
        return stack;
    }

    private FrameworkElement AgentList(IReadOnlyList<JsonObject> sessions, bool compact)
    {
        var stack = new StackPanel { Spacing = 6 };
        var header = new Grid();
        header.Children.Add(new TextBlock { Text = "Agents", FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        var counts = new List<string>();
        var working = sessions.Count(s => Text(s["state"]) == "working");
        var idle = sessions.Count(s => Text(s["state"]) == "idle");
        if (working > 0) counts.Add($"{working} working");
        if (idle > 0) counts.Add($"{idle} idle");
        var count = Caption(string.Join(" · ", counts));
        count.HorizontalAlignment = HorizontalAlignment.Right;
        header.Children.Add(count);
        AutomationProperties.SetHeadingLevel(header.Children[0], AutomationHeadingLevel.Level2);
        stack.Children.Add(header);
        var list = new StackPanel { Margin = new Thickness(-8, 0, -8, 0), XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
        foreach (var session in sessions) list.Children.Add(AgentRow(session, compact));
        stack.Children.Add(list);
        var finish = new Grid { Padding = new Thickness(0, 4, 0, 0) };
        finish.Children.Add(new TextBlock { Text = "When agents finish", FontSize = 13, VerticalAlignment = VerticalAlignment.Center });
        var combo = FinishChoice();
        combo.HorizontalAlignment = HorizontalAlignment.Right;
        finish.Children.Add(combo);
        stack.Children.Add(finish);
        return stack;
    }

    private ComboBox FinishChoice()
    {
        var combo = new ComboBox { MinWidth = 140 };
        AutomationProperties.SetName(combo, "When agents finish");
        var current = Text(Setting("agents.defaultCompletion")) ?? "nothing";
        foreach (var (value, label) in Actions.Select(a => (a, Labels.Action(a))).Prepend(("nothing", "Nothing")))
            combo.Items.Add(new ComboBoxItem { Content = label, Tag = value });
        combo.SelectedItem = combo.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == current);
        combo.SelectionChanged += (_, _) =>
        {
            if (combo.SelectedItem is ComboBoxItem { Tag: string value } && value != current)
                _ = Set("agents.defaultCompletion", value == "nothing" ? null : value);
        };
        return combo;
    }

    private FrameworkElement AgentRow(JsonObject session, bool compact)
    {
        var id = Text(session["id"]);
        var name = Text(session["name"]) ?? "Agent";
        var state = Text(session["state"]) ?? "idle";
        var process = Text(session["source"]) == "process";
        var row = new Grid { ColumnSpacing = 12, MinHeight = 48, Padding = new Thickness(8, 6, 8, 6) };
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        row.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        var tile = Monogram(Text(session["monogram"]) ?? "?");
        tile.VerticalAlignment = VerticalAlignment.Center;
        row.Children.Add(tile);
        var text = new StackPanel { VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = name });
        text.Children.Add(Caption(AgentCaption(session, process)));
        Grid.SetColumn(text, 1);
        row.Children.Add(text);
        var right = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, VerticalAlignment = VerticalAlignment.Center };
        right.Children.Add(Pill(session, state, process));
        if (!compact)
        {
            if (state == "needsApproval")
            {
                right.Children.Add(Button("Allow", () => Send("agent-allow", new JsonObject { ["id"] = id }), $"Allow {name}", accent: true));
                right.Children.Add(Button("Deny", () => Send("agent-deny", new JsonObject { ["id"] = id }), $"Deny {name}"));
            }
            else if (state is "working" or "idle")
                right.Children.Add(Button("Release", () => Send("agent-release", new JsonObject { ["id"] = id }), $"Release {name}"));
        }
        Grid.SetColumn(right, 2);
        row.Children.Add(right);
        var holder = new Border { CornerRadius = new CornerRadius(4), Child = row, IsTabStop = compact };
        if (compact) holder.UseSystemFocusVisuals = true;
        AutomationProperties.SetName(holder, $"{name}, {StateLabel(session, state, process)}, {AgentCaption(session, process)}");
        return holder;
    }

    private string AgentCaption(JsonObject session, bool process)
    {
        if (process) return "Detected by process";
        var parts = new List<string>();
        if (Text(session["project"]) is string project) parts.Add(project);
        var state = Text(session["state"]);
        if (state == "idle" && Number(session["stateSince"]) is long since)
            parts.Add($"Finished {Math.Max(1, (EngineNow - since) / 60)} min ago");
        else if (Text(session["task"]) is string task) parts.Add(task);
        return parts.Count > 0 ? string.Join(" · ", parts) : Text(session["source"]) == "mcpLease" ? "MCP" : "";
    }

    private string StateLabel(JsonObject session, string state, bool process) => state switch
    {
        "working" => "Working " + Labels.Elapsed(WorkingSeconds(session)),
        "needsApproval" => "Asking",
        "done" => "Done",
        _ => process ? "Open" : "Idle"
    };

    private long WorkingSeconds(JsonObject session) =>
        (Number(session["workingSeconds"]) ?? 0) + Math.Max(0, EngineNow - (Number(session["stateSince"]) ?? EngineNow));

    private FrameworkElement Pill(JsonObject session, string state, bool process)
    {
        var pill = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 6, VerticalAlignment = VerticalAlignment.Center };
        if (state == "working")
        {
            var dot = (FrameworkElement)XamlReader.Load("<Ellipse xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"7\" Height=\"7\" Fill=\"{ThemeResource DozeTrayAccentBrush}\" VerticalAlignment=\"Center\" />");
            Pulse(dot);
            pill.Children.Add(dot);
            var minutes = (TextBlock)XamlReader.Load("<TextBlock xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" FontSize=\"12\" Foreground=\"{ThemeResource DozeWorkingBrush}\" />");
            minutes.Text = Labels.Elapsed(WorkingSeconds(session));
            workingPills.Add((minutes, session));
            pill.Children.Add(minutes);
        }
        else pill.Children.Add(Caption(StateLabel(session, state, process)));
        return pill;
    }

    /// The working dot breathes, unless the user turned animations off.
    private void Pulse(FrameworkElement dot)
    {
        if (!system.AnimationsEnabled) return;
        var animation = new DoubleAnimation { From = 1, To = 0.35, Duration = TimeSpan.FromMilliseconds(800), AutoReverse = true, RepeatBehavior = RepeatBehavior.Forever, EasingFunction = new SineEase() };
        Storyboard.SetTarget(animation, dot);
        Storyboard.SetTargetProperty(animation, "Opacity");
        var story = new Storyboard();
        story.Children.Add(animation);
        dot.Loaded += (_, _) => story.Begin();
        dot.Unloaded += (_, _) => story.Stop();
    }

    // ---------- Sub-pages ----------

    private FrameworkElement KeepAwakePage()
    {
        var items = new List<FrameworkElement>();
        var awake = Flag(Session["awake"]);
        var indefinite = awake && Number(Session["awakeRemaining"]) is null;
        var ends = awake && !indefinite ? $"Ends {WallClock(Number(Session["awakeDeadline"]) ?? EngineNow + (Number(Session["awakeRemaining"]) ?? 0)):t}" : null;
        var defaultMinutes = (int)(Number(Setting("defaultAwakeMinutes")) ?? 30);
        items.Add(Item("Default", Labels.Minutes(defaultMinutes), () => Choose("awake-default", defaultMinutes * 60)));
        items.Add(Separator());
        foreach (var (seconds, label) in new[] { (900, "15 minutes"), (1800, "30 minutes"), (3600, "1 hour"), (7200, "2 hours") })
            items.Add(Item(label, awake && chosenAwake == seconds ? ends : null, () => Choose("awake", seconds), awake && chosenAwake == seconds));
        items.Add(Item("Indefinitely", null, () => Choose("awake-forever", null), indefinite));
        items.Add(Separator());
        items.Add(Item("Custom duration…", null, () => { Dismiss(); openTimer("awakeDuration"); return Task.CompletedTask; }));
        items.Add(Item("Until a specific time…", null, () => { Dismiss(); openTimer("awakeTime"); return Task.CompletedTask; }));
        if (awake) { items.Add(Separator()); items.Add(Item("Stop keeping awake", null, () => Send("stop-awake"))); }
        return SubPage("keep", items, "More Keep awake settings", "session");
    }

    private Task Choose(string command, long? seconds)
    {
        chosenAwake = seconds;
        return Send(command, seconds is long s && command == "awake" ? new JsonObject { ["seconds"] = s } : null);
    }

    private FrameworkElement TimerPage()
    {
        var items = new List<FrameworkElement>();
        var action = Text(Session["selectedAction"]) ?? Text(Setting("defaultAction")) ?? "sleep";
        var row = new Grid { Padding = new Thickness(16, 4, 12, 8) };
        row.Children.Add(new TextBlock { Text = "Action", VerticalAlignment = VerticalAlignment.Center });
        var combo = new ComboBox { MinWidth = 150, HorizontalAlignment = HorizontalAlignment.Right };
        AutomationProperties.SetName(combo, "Power timer action");
        foreach (var value in Actions) combo.Items.Add(new ComboBoxItem { Content = Labels.Action(value), Tag = value });
        combo.SelectedItem = combo.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == action);
        combo.SelectionChanged += (_, _) =>
        {
            if (combo.SelectedItem is ComboBoxItem { Tag: string value } && value != action)
                _ = Send("select-action", new JsonObject { ["action"] = value });
        };
        row.Children.Add(combo);
        items.Add(row);
        if (Session["timer"] is JsonObject timer)
        {
            var left = Left(Number(timer["deadline"]), Number(timer["remaining"]));
            items.Add(Item($"{Labels.Action(Text(timer["action"]))} in {Labels.Remaining(left)}", $"At {DateTime.Now.AddSeconds(left):t}", () => Task.CompletedTask, true));
            items.Add(Item("Stop timer", null, () => Send("stop-timer")));
            items.Add(Separator());
        }
        var defaultMinutes = (int)(Number(Setting("defaultTimerMinutes")) ?? 30);
        items.Add(Item("Default", Labels.Minutes(defaultMinutes), () => Send("timer-default")));
        items.Add(Separator());
        foreach (var (seconds, label) in new[] { (900, "15 minutes"), (1800, "30 minutes"), (3600, "1 hour"), (7200, "2 hours") })
            items.Add(Item(label, null, () => Send("timer", new JsonObject { ["seconds"] = seconds, ["action"] = action })));
        items.Add(Separator());
        items.Add(Item("Custom duration…", null, () => { Dismiss(); openTimer("timerDuration"); return Task.CompletedTask; }));
        items.Add(Item("At a specific time…", null, () => { Dismiss(); openTimer("timerTime"); return Task.CompletedTask; }));
        return SubPage("timer", items, "More Power timer settings", "session");
    }

    private FrameworkElement AgentsPage()
    {
        var items = new List<FrameworkElement>();
        var sessions = Sessions.ToList();
        if (sessions.Count == 0)
        {
            var empty = new StackPanel { Spacing = 8, Padding = new Thickness(16, 8, 16, 12) };
            empty.Children.Add(new TextBlock { Text = "No agents running" });
            empty.Children.Add(Caption("Connect Claude Code, Codex, OpenCode or Gemini CLI and they appear here while they work."));
            empty.Children.Add(Button("Connect…", () => OpenSettings("agents"), "Connect an agent"));
            items.Add(empty);
        }
        else
        {
            var list = new StackPanel { Padding = new Thickness(8, 0, 8, 0), XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
            foreach (var session in sessions) list.Children.Add(AgentRow(session, compact: false));
            items.Add(list);
        }
        items.Add(Separator());
        var finish = new Grid { Padding = new Thickness(16, 4, 12, 8) };
        finish.Children.Add(new TextBlock { Text = "When agents finish", VerticalAlignment = VerticalAlignment.Center });
        var combo = FinishChoice();
        combo.HorizontalAlignment = HorizontalAlignment.Right;
        finish.Children.Add(combo);
        items.Add(finish);
        items.Add(Caption("If an agent stops checking in, Doze stays awake for up to 30 minutes, then lets go without acting.", new Thickness(16, 0, 16, 8)));
        return SubPage("agents", items, "More agent settings", "agents");
    }

    private FrameworkElement CountdownPage()
    {
        var items = new List<FrameworkElement>();
        if (Session["countdown"] is JsonObject countdown)
        {
            countdownText = new TextBlock { Text = StatusDetail(), FontSize = 20, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Margin = new Thickness(16, 0, 16, 4) };
            items.Add(countdownText);
            items.Add(Caption(Labels.CountdownSource(Text(countdown["source"])), new Thickness(16, 0, 16, 8)));
            var snooze = Number(Setting("snoozeMinutes")) ?? 15;
            items.Add(Item($"Snooze {snooze} minutes", null, () => Send("snooze")));
            items.Add(Item("Cancel the action", null, () => Send("cancel")));
            items.Add(Item("Stay Awake", null, () => Send("stay-awake")));
            items.Add(Separator());
        }
        var length = (int)(Number(Setting("countdownSeconds")) ?? 300);
        var row = new Grid { Padding = new Thickness(16, 4, 12, 8) };
        row.Children.Add(new TextBlock { Text = "Warning length", VerticalAlignment = VerticalAlignment.Center });
        var combo = new ComboBox { MinWidth = 140, HorizontalAlignment = HorizontalAlignment.Right };
        AutomationProperties.SetName(combo, "Warning length");
        foreach (var seconds in new[] { 60, 120, 180, 300, 600, 900, length }.Distinct().Order())
            combo.Items.Add(new ComboBoxItem { Content = Labels.Seconds(seconds), Tag = seconds });
        combo.SelectedItem = combo.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (int)item.Tag == length);
        combo.SelectionChanged += (_, _) =>
        {
            if (combo.SelectedItem is ComboBoxItem { Tag: int value } && value != length) _ = Set("countdownSeconds", value);
        };
        row.Children.Add(combo);
        items.Add(row);
        var sound = new Grid { Padding = new Thickness(16, 0, 4, 4) };
        sound.Children.Add(new TextBlock { Text = "Play a sound", VerticalAlignment = VerticalAlignment.Center });
        var toggle = new ToggleSwitch { IsOn = Flag(Setting("warningSound")), HorizontalAlignment = HorizontalAlignment.Right, MinWidth = 0, OnContent = "On", OffContent = "Off" };
        AutomationProperties.SetName(toggle, "Play a sound when the warning appears");
        toggle.Toggled += (_, _) => _ = Set("warningSound", toggle.IsOn);
        sound.Children.Add(toggle);
        items.Add(sound);
        items.Add(Item("Preview the warning", null, () => { Dismiss(); return Send("preview"); }));
        return SubPage("countdown", items, "More notification settings", "notifications");
    }

    private FrameworkElement SubPage(string page, List<FrameworkElement> items, string more, string settingsPage)
    {
        var stack = new StackPanel();
        var header = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8, Padding = new Thickness(12, 12, 12, 8) };
        var back = new Button
        {
            Content = new FontIcon { Glyph = "", FontSize = 14 }, Width = 32, Height = 32, Padding = new Thickness(0),
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent), BorderThickness = new Thickness(0)
        };
        AutomationProperties.SetName(back, "Back");
        back.Click += (_, _) => Navigate("");
        header.Children.Add(back);
        var title = new TextBlock { Text = PageTitle(page), FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, VerticalAlignment = VerticalAlignment.Center };
        AutomationProperties.SetHeadingLevel(title, AutomationHeadingLevel.Level1);
        header.Children.Add(title);
        stack.Children.Add(header);
        var list = new StackPanel { Spacing = 2, Padding = new Thickness(8, 0, 8, 8), XYFocusKeyboardNavigation = XYFocusKeyboardNavigationMode.Enabled };
        foreach (var item in items) list.Children.Add(item);
        stack.Children.Add(new ScrollViewer { Content = list, MaxHeight = 460, VerticalScrollBarVisibility = ScrollBarVisibility.Auto });
        var link = new HyperlinkButton { Content = more, Padding = new Thickness(0), FontSize = 13 };
        link.Click += async (_, _) => await OpenSettings(settingsPage);
        return Framed(stack, FooterWith(link, ("Settings", "", () => OpenSettings("overview"))));
    }

    /// One row of a sub-page: a command, with an optional caption on the right and the
    /// accent marker when it is the current choice.
    private FrameworkElement Item(string label, string? caption, Func<Task> invoke, bool selected = false)
    {
        var grid = new Grid { Height = 40 };
        grid.Children.Add(new TextBlock { Text = label, VerticalAlignment = VerticalAlignment.Center });
        if (caption is not null)
        {
            var right = Caption(caption);
            right.HorizontalAlignment = HorizontalAlignment.Right;
            right.VerticalAlignment = VerticalAlignment.Center;
            grid.Children.Add(right);
        }
        var button = new Button
        {
            Content = grid, HorizontalAlignment = HorizontalAlignment.Stretch, HorizontalContentAlignment = HorizontalAlignment.Stretch,
            Padding = new Thickness(16, 0, 12, 0), BorderThickness = new Thickness(0), MinHeight = 40,
            Background = selected ? (Brush)Application.Current.Resources["SubtleFillColorSecondaryBrush"] : new SolidColorBrush(Microsoft.UI.Colors.Transparent)
        };
        AutomationProperties.SetName(button, caption is null ? label : $"{label}, {caption}");
        if (selected) AutomationProperties.SetItemStatus(button, "Current");
        button.Click += async (_, _) => { try { await invoke(); } catch { } };
        if (!selected) return button;
        var marked = new Grid();
        marked.Children.Add(button);
        var bar = (FrameworkElement)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"3\" CornerRadius=\"2\" Margin=\"0,12\" HorizontalAlignment=\"Left\" Background=\"{ThemeResource DozeTrayAccentBrush}\" />");
        marked.Children.Add(bar);
        return marked;
    }

    private static FrameworkElement Separator() => (FrameworkElement)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Height=\"1\" Margin=\"8,6\" Background=\"{ThemeResource DividerStrokeColorDefaultBrush}\" />");

    // ---------- Pieces ----------

    private string BatteryText()
    {
        if (snapshot["battery"] is not JsonObject battery) return "";
        var percent = Number(battery["percent"]) ?? 0;
        if (!Flag(battery["onBattery"])) return "Plugged in";
        var floor = Number(Setting("batteryFloorPercent")) ?? 0;
        return floor > 0 ? $"{percent}% · stops below {floor}%" : $"{percent}%";
    }

    private Task OpenSettings(string page)
    {
        Dismiss();
        return Send("open-settings", new JsonObject { ["page"] = page });
    }

    private FrameworkElement Framed(FrameworkElement content, FrameworkElement footer)
    {
        var grid = new Grid();
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.RowDefinitions.Add(new RowDefinition { Height = GridLength.Auto });
        grid.Children.Add(content);
        Grid.SetRow(footer, 1);
        grid.Children.Add(footer);
        return grid;
    }

    private FrameworkElement Footer(string text, params (string Name, string Glyph, Func<Task> Invoke)[] buttons) =>
        FooterWith(new TextBlock { Text = text, FontSize = 12, VerticalAlignment = VerticalAlignment.Center }, buttons);

    private FrameworkElement FooterWith(FrameworkElement left, params (string Name, string Glyph, Func<Task> Invoke)[] buttons)
    {
        var footer = (Border)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Height=\"48\" Padding=\"16,0,8,0\" BorderThickness=\"0,1,0,0\" Background=\"{ThemeResource LayerOnAcrylicFillColorDefaultBrush}\" BorderBrush=\"{ThemeResource DividerStrokeColorDefaultBrush}\" />");
        var grid = new Grid();
        left.VerticalAlignment = VerticalAlignment.Center;
        grid.Children.Add(left);
        var right = new StackPanel { Orientation = Orientation.Horizontal, HorizontalAlignment = HorizontalAlignment.Right, VerticalAlignment = VerticalAlignment.Center };
        foreach (var (name, glyph, invoke) in buttons)
        {
            var button = new Button
            {
                Content = new FontIcon { Glyph = glyph, FontSize = 16 }, Width = 32, Height = 32, Padding = new Thickness(0),
                Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent), BorderThickness = new Thickness(0)
            };
            AutomationProperties.SetName(button, name);
            ToolTipService.SetToolTip(button, name);
            button.Click += async (_, _) => { try { await invoke(); } catch { } };
            right.Children.Add(button);
        }
        grid.Children.Add(right);
        footer.Child = grid;
        return footer;
    }

    private static TextBlock Caption(string text, Thickness? margin = null)
    {
        var block = (TextBlock)XamlReader.Load("<TextBlock xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" FontSize=\"12\" TextWrapping=\"Wrap\" Foreground=\"{ThemeResource TextFillColorSecondaryBrush}\" />");
        block.Text = text;
        if (margin is Thickness m) block.Margin = m;
        return block;
    }

    private static Border Monogram(string letters)
    {
        var tile = (Border)XamlReader.Load("<Border xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" Width=\"28\" Height=\"28\" CornerRadius=\"4\" Background=\"{ThemeResource DozeTileBrush}\" />");
        var text = (TextBlock)XamlReader.Load("<TextBlock xmlns=\"http://schemas.microsoft.com/winfx/2006/xaml/presentation\" FontSize=\"11\" FontWeight=\"Bold\" HorizontalAlignment=\"Center\" VerticalAlignment=\"Center\" Foreground=\"{ThemeResource DozeTileTextBrush}\" />");
        text.Text = letters;
        tile.Child = text;
        AutomationProperties.SetAccessibilityView(tile, Microsoft.UI.Xaml.Automation.Peers.AccessibilityView.Raw);
        return tile;
    }

    private static Button Button(string label, Func<Task> invoke, string? name = null, bool accent = false)
    {
        var button = new Button { Content = label };
        if (accent) button.Style = (Style)Application.Current.Resources["AccentButtonStyle"];
        if (name is not null) AutomationProperties.SetName(button, name);
        button.Click += async (_, _) => { try { await invoke(); } catch { } };
        return button;
    }

    /// Once a second while open: countdowns, time left and working minutes. Never contacts the engine.
    private void Tick()
    {
        if (statusDetail is not null) statusDetail.Text = StatusDetail();
        if (countdownText is not null) countdownText.Text = StatusDetail();
        foreach (var (text, session) in workingPills) text.Text = Labels.Elapsed(WorkingSeconds(session));
    }

    // ---------- Window ----------

    private string? FocusedName()
    {
        if (root.XamlRoot is null || FocusManager.GetFocusedElement(root.XamlRoot) is not DependencyObject focused) return null;
        return AutomationProperties.GetName(focused);
    }

    private void Refocus(string? name)
    {
        root.UpdateLayout();
        var target = name is null ? null : MainWindow.Descendants(root).OfType<Control>().FirstOrDefault(c => AutomationProperties.GetName(c) == name && c.IsEnabled);
        if (target is not null) target.Focus(FocusState.Keyboard);
        else FocusFirst();
    }

    private void FocusFirst()
    {
        root.UpdateLayout();
        var first = subpage == ""
            ? MainWindow.Descendants(root).OfType<ToggleButton>().FirstOrDefault(c => c.IsEnabled)
            : MainWindow.Descendants(root).OfType<Control>().Skip(1).FirstOrDefault(c => c.IsEnabled && c.IsTabStop);
        first?.Focus(FocusState.Programmatic);
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint window, int attribute, ref int value, int size);
    [DllImport("user32.dll")]
    private static extern nint MonitorFromPoint(PointInt32 point, uint flags);
    [DllImport("shcore.dll")]
    private static extern int GetDpiForMonitor(nint monitor, int type, out uint dpiX, out uint dpiY);

    /// Windows 11 rounds borderless windows only when asked: 8 px corners, like Quick Settings.
    private void Rounded()
    {
        var round = 2; // DWMWCP_ROUND
        _ = DwmSetWindowAttribute(WinRT.Interop.WindowNative.GetWindowHandle(this), 33, ref round, sizeof(int));
    }

    private (PointInt32 Point, double Scale) AnchorPoint()
    {
        var x = (int)(Number(anchor?["x"]) ?? 0);
        var y = (int)(Number(anchor?["y"]) ?? 0);
        var center = new PointInt32(x + (int)((Number(anchor?["width"]) ?? 0) / 2), y + (int)((Number(anchor?["height"]) ?? 0) / 2));
        if (anchor is null)
        {
            var primary = DisplayArea.Primary.WorkArea;
            center = new PointInt32(primary.X + primary.Width - 1, primary.Y + primary.Height - 1);
        }
        var scale = 1.0;
        if (GetDpiForMonitor(MonitorFromPoint(center, 2), 0, out var dpi, out _) == 0) scale = dpi / 96.0;
        return (center, scale);
    }

    /// The height the content needs at 360 effective pixels wide.
    private double ContentHeight()
    {
        root.Measure(new Windows.Foundation.Size(360, double.PositiveInfinity));
        return Math.Ceiling(root.DesiredSize.Height);
    }

    /// Above the tray icon, whichever edge the taskbar is on, inside that display's work area.
    private void Place()
    {
        var (center, scale) = AnchorPoint();
        var display = DisplayArea.GetFromPoint(center, DisplayAreaFallback.Nearest);
        var work = display.WorkArea;
        var outer = display.OuterBounds;
        var width = (int)Math.Ceiling(360 * scale);
        var height = Math.Min((int)Math.Ceiling(ContentHeight() * scale), work.Height - (int)(24 * scale));
        var margin = (int)(12 * scale);
        int left, top;
        edge = work.Y > outer.Y ? Edge.Top : work.X > outer.X ? Edge.Left : work.X + work.Width < outer.X + outer.Width ? Edge.Right : Edge.Bottom;
        switch (edge)
        {
            case Edge.Top:
                left = Math.Clamp(center.X - width / 2, work.X + margin, work.X + work.Width - width - margin);
                top = work.Y + margin;
                break;
            case Edge.Left:
                left = work.X + margin;
                top = Math.Clamp(center.Y - height / 2, work.Y + margin, work.Y + work.Height - height - margin);
                break;
            case Edge.Right:
                left = work.X + work.Width - width - margin;
                top = Math.Clamp(center.Y - height / 2, work.Y + margin, work.Y + work.Height - height - margin);
                break;
            default:
                left = Math.Clamp(center.X - width / 2, work.X + margin, work.X + work.Width - width - margin);
                top = work.Y + work.Height - height - margin;
                break;
        }
        AppWindow.MoveAndResize(new RectInt32(left, top, width, height));
    }

    private enum Edge { Bottom, Top, Left, Right }
    private Edge edge = Edge.Bottom;

    /// Keeps the flyout's edge against the taskbar when its content grows or shrinks.
    private void Resize()
    {
        if (!AppWindow.IsVisible) return;
        Place();
    }

    /// The Quick Settings entrance: a short slide away from the taskbar and a fade.
    private void Animate()
    {
        var transform = (TranslateTransform)root.RenderTransform;
        if (!system.AnimationsEnabled) { transform.X = transform.Y = 0; root.Opacity = 1; return; }
        var (property, from) = edge switch
        {
            Edge.Top => ("Y", -24.0),
            Edge.Left => ("X", -24.0),
            Edge.Right => ("X", 24.0),
            _ => ("Y", 24.0)
        };
        var story = new Storyboard();
        var slide = new DoubleAnimation { From = from, To = 0, Duration = TimeSpan.FromMilliseconds(250), EasingFunction = new ExponentialEase { EasingMode = EasingMode.EaseOut, Exponent = 6 } };
        Storyboard.SetTarget(slide, transform);
        Storyboard.SetTargetProperty(slide, property);
        var fade = new DoubleAnimation { From = 0, To = 1, Duration = TimeSpan.FromMilliseconds(120) };
        Storyboard.SetTarget(fade, root);
        Storyboard.SetTargetProperty(fade, "Opacity");
        story.Children.Add(slide);
        story.Children.Add(fade);
        story.Begin();
    }

    // ---------- Verification ----------

    /// Builds every page of the flyout in both themes with sample sessions, without showing it.
    public void Verify(JsonObject sample)
    {
        snapshot = sample;
        foreach (var appearance in new[] { ElementTheme.Light, ElementTheme.Dark })
        {
            root.RequestedTheme = appearance;
            foreach (var page in new[] { "", "keep", "timer", "agents", "countdown" })
            {
                subpage = page;
                Render();
                root.UpdateLayout();
                var controls = MainWindow.Descendants(root).OfType<Control>().Where(c => c is ButtonBase or ComboBox or ToggleSwitch).ToList();
                if (controls.Count == 0) throw new InvalidOperationException($"Flyout page \"{page}\" has no controls.");
                if (controls.FirstOrDefault(c => string.IsNullOrEmpty(AutomationProperties.GetName(c)) && (c as ContentControl)?.Content is not string) is Control unnamed)
                    throw new InvalidOperationException($"Flyout page \"{page}\" has a {unnamed.GetType().Name} without a Narrator name.");
            }
        }
        subpage = "";
        Render();
        var tiles = MainWindow.Descendants(root).OfType<ToggleButton>().Select(t => AutomationProperties.GetName(t)).ToList();
        foreach (var name in new[] { "Keep awake", "Awake while audio plays", "Sleep after playback", "Power timer", "Agents", "Countdown" })
            if (!tiles.Contains(name)) throw new InvalidOperationException($"Flyout is missing the {name} tile.");
        if (ContentHeight() < 200) throw new InvalidOperationException("Flyout content did not lay out.");
    }

    public async Task RenderVerificationAsync(string directory, JsonObject sample)
    {
        snapshot = sample;
        anchor = null;
        Place();
        AppWindow.Show(false);
        try
        {
            foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
            {
                root.RequestedTheme = theme;
                root.Background = new SolidColorBrush(theme == ElementTheme.Dark ? Windows.UI.Color.FromArgb(255, 38, 38, 42) : Windows.UI.Color.FromArgb(255, 242, 242, 244));
                foreach (var page in new[] { "", "keep", "timer", "agents", "countdown" })
                {
                    subpage = page;
                    Render();
                    Place();
                    await Task.Delay(300);
                    await VisualVerification.SaveAsync(root, Path.Combine(directory, $"Flyout-{(page == "" ? "main" : page)}-{theme}.png"));
                }
            }
        }
        finally { root.Background = null; AppWindow.Hide(); }
    }
}
