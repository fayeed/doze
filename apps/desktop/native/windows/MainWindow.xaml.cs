using System.Diagnostics;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.Graphics;

namespace Doze.SettingsUi;

public sealed partial class MainWindow : Window
{
    private readonly EngineBridge bridge;
    private readonly Action<string> changeTheme;
    private JsonObject snapshot;
    /// When the latest snapshot arrived. Remaining times count down from the engine's `now`
    /// on this clock, so the window never asks the engine for updates.
    private readonly Stopwatch received = Stopwatch.StartNew();
    private string page = "Overview";
    private readonly bool verification = Environment.GetCommandLineArgs().Contains("--verify-ui");
    /// Redraws countdowns once a second while one is visible. It never contacts the engine.
    private readonly DispatcherTimer clock = new() { Interval = TimeSpan.FromSeconds(1) };
    private static readonly string[] Pages = ["Overview", "General", "Session defaults", "After playback", "Notifications", "Agents", "Advanced", "Menu guide", "About Doze"];
    // Smallest size in effective pixels; it fits a 1080p display at 200% scaling with the taskbar.
    private const int MinimumWidth = 680, MinimumHeight = 480;

    private readonly Action<string>? openTimer;

    public MainWindow(EngineBridge bridge, JsonObject initial, Action<string>? changeTheme = null, Action<string>? openTimer = null)
    {
        this.bridge = bridge;
        this.changeTheme = changeTheme ?? SetTheme;
        this.openTimer = openTimer;
        snapshot = initial["snapshot"]!.AsObject();
        InitializeComponent();
        WindowAppearance.Observe(this, Root);
        SetTheme(Text(Setting("theme")) ?? "system");
        Title = "Doze Settings";
        AppWindow.IsShownInSwitchers = true;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
        var titleIcon = Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.png");
        if (File.Exists(titleIcon)) TitleIcon.Source = new BitmapImage(new Uri(titleIcon)) { DecodePixelWidth = 32 };
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(TitleBar);
        SystemBackdrop = new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt };
        FitToDisplay(1120, 780);
        AppWindow.Closing += (window, args) =>
        {
            if (verification) return;
            // Every change is already applied; closing only hides the window.
            args.Cancel = true;
            HideWindow();
        };
        // Ctrl+F finds a setting, as in Windows Settings.
        Root.KeyboardAccelerators.Add(Accelerator(Windows.System.VirtualKey.F, Windows.System.VirtualKeyModifiers.Control, () => Search.Focus(FocusState.Keyboard)));
        SelectPage(ViewPage(initial["view"]?.GetValue<string>(), Text(initial["page"]), "Overview"));
        clock.Tick += (_, _) =>
        {
            if (!AppWindow.IsVisible || !HasDeadline) { clock.Stop(); return; }
            UpdateLive();
        };
        Closed += (_, _) => clock.Stop();
        StartClock();
    }

    private static Microsoft.UI.Xaml.Input.KeyboardAccelerator Accelerator(Windows.System.VirtualKey key, Windows.System.VirtualKeyModifiers modifiers, Action invoke)
    {
        var accelerator = new Microsoft.UI.Xaml.Input.KeyboardAccelerator { Key = key, Modifiers = modifiers };
        accelerator.Invoked += (_, args) => { args.Handled = true; invoke(); };
        return accelerator;
    }

    private void StartClock()
    {
        if (HasDeadline && AppWindow.IsVisible) clock.Start();
    }

    public void Receive(JsonObject message)
    {
        // The pipe is read from the UI synchronization context; use the dispatcher defensively.
        DispatcherQueue.TryEnqueue(() =>
        {
            var command = message["command"]?.GetValue<string>();
            if (message["error"] is JsonValue error)
            {
                // A refused change leaves its control as the engine has it.
                KeepScroll(ShowPage);
                Notify(command is "set" or "save" ? "Couldn't apply that change" : "Couldn't complete that", error.GetValue<string>(), InfoBarSeverity.Error);
                return;
            }
            if (message["snapshot"] is JsonObject next)
            {
                var previous = snapshot;
                // Streamed updates leave out file-backed parts; keep the last ones.
                foreach (var key in new[] { "agentLinks", "agentSkills", "agentConnections" })
                    if (next[key] is null && previous[key] is JsonNode kept) next[key] = kept.DeepClone();
                snapshot = next;
                received.Restart();
                if (Text(previous["settings"]?["theme"]) != Text(next["settings"]?["theme"]))
                    changeTheme(Text(Setting("theme")) ?? "system");
                UpdateLivePage(previous);
                StartClock();
            }
            if (message["result"] is JsonObject result) HandleResult(command, result);
            if (message["type"]?.GetValue<string>() == "open")
            {
                SelectPage(ViewPage(message["view"]?.GetValue<string>(), Text(message["page"]), page));
                AppWindow.Show();
                Activate();
                StartClock();
            }
        });
    }

    /// Rebuilds the visible page only when what it shows changed; times update in place so
    /// focus and scroll position survive.
    private void UpdateLivePage(JsonObject previous)
    {
        if (PageShape(previous) != PageShape(snapshot)) KeepScroll(ShowPage);
        else UpdateLive();
    }

    /// Everything the current page shows except values that only count down.
    private string PageShape(JsonObject from)
    {
        var copy = from.DeepClone().AsObject();
        foreach (var key in new[] { "now", "agentNow", "timerStatus", "status", "statusDetail", "assertions", "battery" }) copy.Remove(key);
        if (copy["session"] is JsonObject session)
        {
            session.Remove("awakeRemaining");
            if (session["timer"] is JsonObject timer) timer.Remove("remaining");
            if (session["countdown"] is JsonObject countdown) countdown.Remove("remaining");
        }
        return copy.ToJsonString();
    }

    private void KeepScroll(Action rebuild)
    {
        var offset = PageScroll.VerticalOffset;
        var focus = FocusedInPage();
        rebuild();
        PageScroll.UpdateLayout();
        PageScroll.ChangeView(null, offset, null, true);
        if (focus is var (name, section)) RestoreFocus(name, section);
    }

    /// The focused control's name and section, when focus is on the page itself.
    private (string? Name, int Section)? FocusedInPage()
    {
        // Only while Settings is in front: focusing one of its controls activates the window,
        // which would take the foreground from the tray flyout or the countdown.
        if (GetForegroundWindow() != WinRT.Interop.WindowNative.GetWindowHandle(this)) return null;
        if (Root.XamlRoot is null || Microsoft.UI.Xaml.Input.FocusManager.GetFocusedElement(Root.XamlRoot) is not FrameworkElement focused) return null;
        for (DependencyObject? node = focused; node is not null; node = VisualTreeHelper.GetParent(node))
            if (node is UIElement child && VisualTreeHelper.GetParent(node) == Cards)
                return (ControlName(focused), Cards.Children.IndexOf(child));
        return null;
    }

    private static string? ControlName(DependencyObject element) =>
        Microsoft.UI.Xaml.Automation.AutomationProperties.GetName(element) is { Length: > 0 } name ? name : (element as ContentControl)?.Content as string;

    /// After a rebuild, keyboard focus returns to the same control, or to the first control in
    /// the same section when that one is gone (Stop becomes the presets), never to the top.
    private void RestoreFocus(string? name, int section)
    {
        static bool Focusable(Control control) => control.IsEnabled && control.IsTabStop && control.Visibility == Visibility.Visible;
        var target = Descendants(Cards).OfType<Control>().FirstOrDefault(control => Focusable(control) && name is not null && ControlName(control) == name)
            ?? (section >= 0 && section < Cards.Children.Count ? Descendants(Cards.Children[section]).OfType<Control>().FirstOrDefault(Focusable) : null);
        target?.Focus(FocusState.Keyboard);
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint window);
    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern nint GetForegroundWindow();

    private double Scale => GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;

    // Size in effective pixels for the window's DPI, centered and kept within the work area.
    private void FitToDisplay(int width, int height)
    {
        var scale = Scale;
        var area = Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Primary).WorkArea;
        var fittedWidth = Math.Min((int)(width * scale), area.Width * 92 / 100);
        var fittedHeight = Math.Min((int)(height * scale), area.Height * 92 / 100);
        if (AppWindow.Presenter is Microsoft.UI.Windowing.OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = Math.Min((int)(MinimumWidth * scale), area.Width);
            presenter.PreferredMinimumHeight = Math.Min((int)(MinimumHeight * scale), area.Height);
        }
        AppWindow.MoveAndResize(new RectInt32(area.X + (area.Width - fittedWidth) / 2, area.Y + (area.Height - fittedHeight) / 2, fittedWidth, fittedHeight));
    }

    private void HideWindow()
    {
        clock.Stop();
        AppWindow.Hide();
        // A hidden window keeps no page alive.
        Cards.Children.Clear();
    }

    public void SetTheme(string value) => WindowAppearance.Apply(Root, value);
    public void StopAppearance() => WindowAppearance.Stop(Root);

    // A construction smoke test of the actual WinUI pages, without showing a window or
    // requesting any engine operation. CI invokes this against inherited test pipes.
    public void VerifyPages()
    {
        Labels.Verify();
        VerifySearch();
        foreach (var appearance in new[] { "light", "dark", "system" })
        {
            SetTheme(appearance);
            if (appearance == "light" && Root.RequestedTheme != ElementTheme.Light
                || appearance == "dark" && Root.RequestedTheme != ElementTheme.Dark)
                throw new InvalidOperationException("Appearance override was not applied.");
        }
        var windowBackdrop = SystemBackdrop;
        foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
        {
            Root.RequestedTheme = theme;
            foreach (var name in Pages)
            {
                SelectPage(name);
                if (Cards.Children.Count == 0 || PageTitle.Text != name)
                    throw new InvalidOperationException($"Could not construct {name} in {theme} mode.");
                if (Cards.Children.OfType<TextBlock>().Any(header => header.Text == name))
                    throw new InvalidOperationException($"{name} repeats its title as a section header.");
                if (!ReferenceEquals(SystemBackdrop, windowBackdrop))
                    throw new InvalidOperationException($"Navigation replaced the window backdrop on {name}.");
                var names = Descendants(Cards).OfType<Control>().Where(c => c is Button or ComboBox or ToggleSwitch or Expander)
                    .Select(ControlName).Where(n => !string.IsNullOrEmpty(n)).ToList();
                if (names.GroupBy(n => n).FirstOrDefault(g => g.Count() > 1) is { } duplicate)
                    throw new InvalidOperationException($"{name} has two controls named \"{duplicate.Key}\".");
                if (Descendants(Cards).OfType<Control>().Any(c => c is Button or ComboBox or ToggleSwitch && string.IsNullOrEmpty(ControlName(c))))
                    throw new InvalidOperationException($"{name} has a control without a Narrator name.");
                if (Descendants(Cards).OfType<NumberBox>().Any())
                    throw new InvalidOperationException($"{name} still uses a number field for a duration.");
            }
        }
        VerifyText();
        VerifyControlCenter();
        foreach (var (pageName, expected) in new[]
        {
            ("General", new[] { "Start Doze when I sign in", "Clicking the tray icon opens", "Show time left in the tray tooltip", "Keep the display on too", "Stay awake with the lid closed", "Stop keeping awake below" }),
            ("Session defaults", new[] { "Default duration", "Remember my last custom duration", "Default action", "Default timer", "Snooze length", "Stay Awake keeps going" }),
            ("After playback", new[] { "Sleep after playback stops", "Wait for inactivity", "Then", "Keep awake while audio plays", "Live output level" }),
            ("Notifications", new[] { "Warning length", "Play a sound when it appears", "Show on every display", "When an agent asks to keep the PC awake", "When all agents have finished", "When an agent stops checking in", "When a Keep Awake session ends" }),
            ("Agents", new[] { "Let agents keep the PC awake", "Ask before a new agent holds a lease", "When agents finish", "If an agent stops checking in", "Claude Code", "Codex", "OpenCode", "Gemini CLI", "Cursor", "Other MCP clients" }),
            ("Advanced", new[] { "doze command-line tool", "MCP server for agents", "How Doze keeps the PC awake", "Local data", "Export a diagnostics report", "Reset all settings" }),
            ("Menu guide", new[] { "Hollow sun", "Whole sun", "Sun with a dot", "Banded sun", "Click the tray icon", "Right-click the tray icon" }),
            ("About Doze", new[] { "Doze", "getdoze.app", "MCP guide", "No account · No cloud · No telemetry" }),
        })
        {
            SelectPage(pageName);
            foreach (var title in expected)
                if (!Shows(title)) throw new InvalidOperationException($"{pageName} is missing \"{title}\".");
        }
        SelectPage("About Doze");
        if (Shows("Source on GitHub")) throw new InvalidOperationException("About links to source code.");
        SelectPage("Advanced");
        if (ResetDialog().PrimaryButtonText != "Reset" || ResetDialog().DefaultButton != ContentDialogButton.Close)
            throw new InvalidOperationException("Reset does not ask for confirmation.");
        if (ConnectDialog(SampleChange()).PrimaryButtonText != "Connect")
            throw new InvalidOperationException("Connect does not show the change for confirmation.");
        SelectPage("Agents");
        // Only agents that are not connected yet offer a prompt; the sample connects all but Gemini CLI.
        if (!Shows("Connect Gemini CLI with a prompt") || Shows("Connect Claude Code with a prompt"))
            throw new InvalidOperationException("Agents does not offer Use a prompt… for exactly the agents not connected.");
        if (PromptDialog(SamplePrompt()).PrimaryButtonText != "Copy prompt")
            throw new InvalidOperationException("Use a prompt… does not show the prompt to copy.");
    }

    internal JsonObject Snapshot => snapshot;

    // Sample sessions for verification and renders: nothing running, then a timed Keep Awake
    // with a timer whose final warning is showing.
    private static JsonObject IdleSession() => new()
    {
        ["awake"] = false, ["whileAudio"] = false, ["holdingAwake"] = false, ["playbackEnabled"] = false,
        ["playbackPhase"] = "waiting", ["selectedAction"] = "sleep", ["message"] = "Doze skill installed"
    };

    private static JsonObject BusySession() => new()
    {
        ["awake"] = true, ["awakeRemaining"] = 2520, ["awakeDeadline"] = 3120, ["whileAudio"] = true, ["holdingAwake"] = true, ["playbackEnabled"] = true,
        ["playbackPhase"] = "grace", ["selectedAction"] = "displayOff",
        ["timer"] = new JsonObject { ["action"] = "displayOff", ["remaining"] = 3900, ["deadline"] = 4500 },
        ["countdown"] = new JsonObject { ["action"] = "displayOff", ["remaining"] = 287, ["deadline"] = 887, ["source"] = "timer", ["length"] = 300 },
        ["message"] = "Snoozed for 15 minutes"
    };

    private static JsonObject SampleChange() => new()
    {
        ["agent"] = "claude-code", ["remove"] = false, ["path"] = @"C:\Users\example\.claude\settings.json",
        ["diff"] = "  {\n+   \"hooks\": {\n+     \"Stop\": []\n+   }\n  }\n", ["token"] = "0", ["note"] = null
    };

    private static JsonObject SamplePrompt() => new()
    {
        ["agent"] = "claude-code", ["name"] = "Claude Code", ["path"] = @"C:\Users\example\.claude\settings.json",
        ["prompt"] = "Set up Doze for Claude Code on this computer. Doze is a desktop app that keeps the computer awake while you work, and it learns when you start and stop from hooks in your settings.\n\n"
            + "Edit C:\\Users\\example\\.claude\\settings.json (create it containing {} if it doesn't exist):\n"
            + "1. First copy it to C:\\Users\\example\\.claude\\settings.json.doze-backup-<date and time>, so it can be restored.\n"
            + "2. Under \"hooks\", append each entry below to that event's list.\n\n```json\n{\n  \"hooks\": {\n    \"Stop\": [ { \"hooks\": [ { \"type\": \"command\", \"command\": \"\\\"C:/Program Files/Doze/doze-cli.exe\\\" hook claude-code Stop\", \"timeout\": 10 } ] } ]\n  }\n}\n```\n"
    };

    private bool Shows(string name) => Descendants(Cards).OfType<FrameworkElement>().Any(element =>
        Microsoft.UI.Xaml.Automation.AutomationProperties.GetName(element) == name || (element as ContentControl)?.Content as string == name
        || (element as TextBlock)?.Text == name);

    private void VerifyControlCenter()
    {
        var original = snapshot["session"]?.DeepClone();
        try
        {
            snapshot["session"] = IdleSession();
            SelectPage("Overview");
            foreach (var name in new[] { "Keep awake for 15m", "Keep awake for 2h", "Keep awake indefinitely", "Sleep in 15m", "Sleep in 2h",
                         "More ways to keep awake", "More timer options", "Keep awake while audio plays", "Sleep after playback stops", "Agent settings" })
                if (!Shows(name)) throw new InvalidOperationException($"Overview is missing \"{name}\".");
            if (Cards.Children.OfType<TextBlock>().Any(header => header.Text == "Overview"))
                throw new InvalidOperationException("Overview repeats its title.");

            var previous = snapshot.DeepClone().AsObject();
            snapshot["session"] = BusySession();
            UpdateLivePage(previous);
            foreach (var name in new[] { "Extend 15 minutes", "Stop keeping awake", "Stop timer", "Snooze", "Stay Awake", "Cancel the action" })
                if (!Shows(name)) throw new InvalidOperationException($"Overview is missing \"{name}\" during a session.");
            if (Shows("Keep awake for 15m")) throw new InvalidOperationException("Overview offers presets while keeping awake.");

            // A snapshot that only changes times updates text in place instead of rebuilding.
            var first = Cards.Children[0];
            previous = snapshot.DeepClone().AsObject();
            snapshot["session"]!["countdown"]!["remaining"] = 286;
            snapshot["session"]!["awakeRemaining"] = 2519;
            UpdateLivePage(previous);
            if (!ReferenceEquals(first, Cards.Children[0]))
                throw new InvalidOperationException("Overview rebuilt instead of updating its clock.");
        }
        finally
        {
            snapshot["session"] = original;
            SelectPage("Overview");
        }
    }

    /// No engine ids, raw seconds or inconsistent action names on any page.
    private void VerifyText()
    {
        foreach (var name in Pages)
        {
            SelectPage(name);
            foreach (var text in Descendants(Cards).OfType<TextBlock>().Select(block => block.Text)
                         .Concat(Descendants(Cards).OfType<ComboBoxItem>().Select(item => item.Content as string ?? "")))
            {
                foreach (var banned in new[] { "displayOff", "Display off", "Shutdown ", "awaiting_authorization", "connection_lost", "(seconds)", "claude-code", "gemini-cli" })
                    if (text.Contains(banned, StringComparison.Ordinal))
                        throw new InvalidOperationException($"{name} shows \"{banned}\" in \"{text}\".");
            }
        }
    }

    internal static IEnumerable<DependencyObject> Descendants(DependencyObject root)
    {
        var children = root switch
        {
            Panel panel => panel.Children.Cast<DependencyObject>(),
            Border border when border.Child is not null => [border.Child],
            ContentControl { Content: DependencyObject content } => [content],
            ContentPresenter { Content: DependencyObject presented } => [presented],
            Expander expander => new[] { expander.Header, expander.Content }.OfType<DependencyObject>(),
            ComboBox combo => combo.Items.OfType<DependencyObject>(),
            _ => []
        };
        foreach (var child in children)
        {
            yield return child;
            foreach (var descendant in Descendants(child)) yield return descendant;
        }
    }

    public async Task RenderVerificationAsync(string directory)
    {
        Directory.CreateDirectory(directory);
        // Render only this app's visual tree. The desktop and other windows are never read.
        AppWindow.Show(false);
        try
        {
            // Effective sizes: the default window, a 1080p display at 200% scaling, and the minimum.
            foreach (var (label, width, height) in new[] { ("", 1120, 780), ("-200pct", 883, 475), ("-minimum", MinimumWidth, MinimumHeight) })
            {
                AppWindow.Resize(new SizeInt32((int)(width * Scale), (int)(height * Scale)));
                foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
                {
                    Root.RequestedTheme = theme;
                    // RenderTargetBitmap omits Mica. Give exported verification images
                    // an opaque surface so the title and navigation remain readable.
                    Root.Background = new SolidColorBrush(theme == ElementTheme.Dark
                        ? Windows.UI.Color.FromArgb(255, 32, 32, 32)
                        : Windows.UI.Color.FromArgb(255, 243, 243, 243));
                    foreach (var name in Pages)
                    {
                        SelectPage(name);
                        await Task.Delay(400);
                        var file = $"{name.Replace(' ', '-')}{label}-{theme}";
                        AssertNothingClipped(name + label);
                        await VisualVerification.SaveAsync(Root, Path.Combine(directory, file + ".png"));
                        if (PageScroll.ScrollableHeight > 0)
                        {
                            PageScroll.ChangeView(null, PageScroll.ScrollableHeight, null, true);
                            await Task.Delay(150);
                            await VisualVerification.SaveAsync(Root, Path.Combine(directory, file + "-end.png"));
                        }
                        if (name == "Overview" && label == "") await RenderSessionsAsync(directory, file);
                        if (label == "" && name == "Advanced") await RenderDialogAsync(ResetDialog(), theme, Path.Combine(directory, $"Advanced-Reset-{theme}.png"));
                        if (label == "" && name == "Agents") await RenderDialogAsync(ConnectDialog(SampleChange()), theme, Path.Combine(directory, $"Agents-Connect-{theme}.png"));
                        if (label == "" && name == "Agents") await RenderDialogAsync(PromptDialog(SamplePrompt()), theme, Path.Combine(directory, $"Agents-Prompt-{theme}.png"));
                    }
                }
            }
        }
        finally { Root.Background = null; AppWindow.Hide(); }
    }

    private async Task RenderSessionsAsync(string directory, string file)
    {
        var original = snapshot["session"]?.DeepClone();
        foreach (var (variant, session) in new[] { ("idle", IdleSession()), ("busy", BusySession()) })
        {
            snapshot["session"] = session;
            ShowPage();
            await Task.Delay(300);
            AssertNothingClipped($"{file} {variant}");
            await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"{file}-{variant}.png"));
        }
        snapshot["session"] = original;
        ShowPage();
    }

    private static async Task RenderDialogAsync(ContentDialog dialog, ElementTheme theme, string path)
    {
        dialog.RequestedTheme = theme;
        var shown = dialog.ShowAsync();
        try { await Task.Delay(300); await VisualVerification.SaveAsync(dialog, path); }
        finally { dialog.Hide(); await shown; }
    }

    /// Fails when text is trimmed, a control is narrower than its content, or a card extends
    /// past the page at the current window size.
    private void AssertNothingClipped(string name)
    {
        Root.UpdateLayout();
        var width = Cards.ActualWidth;
        foreach (var element in Descendants(Cards).OfType<FrameworkElement>().Where(e => e.Visibility == Visibility.Visible && e.ActualWidth > 0))
        {
            if (element is TextBlock { IsTextTrimmed: true } trimmed)
                throw new InvalidOperationException($"{name}: \"{trimmed.Text}\" is truncated.");
            if (element is Button or ComboBox && element.ActualWidth + element.Margin.Left + element.Margin.Right + 0.5 < element.DesiredSize.Width)
                throw new InvalidOperationException($"{name}: {element.GetType().Name} \"{(element as ContentControl)?.Content}\" is narrower than its content ({element.ActualWidth:0} < {element.DesiredSize.Width:0}).");
            var right = element.TransformToVisual(Cards).TransformPoint(new Windows.Foundation.Point(element.ActualWidth, 0)).X;
            if (right > width + 1)
                throw new InvalidOperationException($"{name}: {element.GetType().Name} {(element as TextBlock)?.Text} extends past the page ({right:0} > {width:0}).");
        }
    }

    private static string ViewPage(string? view, string? requested, string fallback)
    {
        if (requested is not null && PageId(requested) is string named) return named;
        return view switch
        {
            "about" => "About Doze",
            "help" => "Menu guide",
            "agents" => "Agents",
            "settings" => "Overview",
            _ => fallback
        };
    }

    /// Page ids shared with the engine and the panel, such as "agents" or "session".
    private static string? PageId(string id) => id switch
    {
        "overview" => "Overview",
        "general" => "General",
        "session" => "Session defaults",
        "playback" => "After playback",
        "notifications" or "notif" => "Notifications",
        "agents" => "Agents",
        "advanced" => "Advanced",
        "guide" => "Menu guide",
        "about" => "About Doze",
        _ => Pages.Contains(id) ? id : null
    };

    private IEnumerable<string> Actions => snapshot["actions"]!.AsArray().Select(action => action!.GetValue<string>());
    private bool Capability(string name) => snapshot[name]?.GetValue<bool>() == true;

    private void Navigate(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is NavigationViewItem item && (string)item.Tag != page) { page = (string)item.Tag; ShowPage(); }
    }

    // Like Windows Settings, the menu button appears only when the pane is collapsed to icons.
    private void DisplayModeChanged(NavigationView sender, NavigationViewDisplayModeChangedEventArgs args) =>
        sender.IsPaneToggleButtonVisible = args.DisplayMode != NavigationViewDisplayMode.Expanded;

    private void SelectPage(string name)
    {
        page = name;
        Navigation.SelectedItem = Navigation.MenuItems.Concat(Navigation.FooterMenuItems)
            .OfType<NavigationViewItem>().First(item => (string)item.Tag == name);
        ShowPage();
    }

    private void ShowPage()
    {
        if (Cards is null) return;
        BeginPage(page);
        Notice.IsOpen = false;
        Build(page);
        // Pages whose text continues below their last control scroll with the keyboard.
        PageScroll.IsTabStop = page is "Menu guide" or "About Doze";
        PageScroll.ChangeView(null, 0, null, true);
    }

    private void Build(string name)
    {
        switch (name)
        {
            case "Overview": Overview(); break;
            case "General": General(); break;
            case "Session defaults": SessionDefaults(); break;
            case "After playback": AfterPlayback(); break;
            case "Notifications": Notifications(); break;
            case "Agents": Agents(); break;
            case "Advanced": Advanced(); break;
            case "Menu guide": MenuGuide(); break;
            case "About Doze": About(); break;
        }
    }

    private static Task OpenLink(string url)
    {
        Process.Start(new ProcessStartInfo(url) { UseShellExecute = true });
        return Task.CompletedTask;
    }

    private Task OpenData()
    {
        var folder = Path.GetDirectoryName(snapshot["settingsPath"]!.GetValue<string>())!;
        Directory.CreateDirectory(folder);
        Process.Start(new ProcessStartInfo(folder) { UseShellExecute = true });
        return Task.CompletedTask;
    }

    private Task Send(string command, JsonObject? fields = null) => verification ? Task.CompletedTask : bridge.SendCommandAsync(command, fields);

    /// Changes one setting in the engine's store. The engine's reply redraws the page.
    private Task Set(string key, JsonNode? value) => Send("set", new JsonObject { ["key"] = key, ["value"] = value });

    private void Notify(string title, string message, InfoBarSeverity severity) { Notice.Title = title; Notice.Message = message; Notice.Severity = severity; Notice.IsOpen = true; }
}
