using System.Diagnostics;
using System.Text.Json;
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
    private readonly LiveSettings preferences;
    private Preferences draft => preferences.Draft;
    private string page = "Overview";
    private bool saving => preferences.IsApplying;
    private bool closeAfterApply;
    private readonly bool verification = Environment.GetCommandLineArgs().Contains("--verify-ui");
    private readonly DispatcherTimer refresh = new() { Interval = TimeSpan.FromSeconds(1) };
    private int ticks;
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
        preferences = new LiveSettings(ReadPreferences());
        InitializeComponent();
        WindowAppearance.Observe(this, Root);
        SetTheme(draft.Theme);
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
            args.Cancel = true;
            if (!preferences.HasChanges && !saving) { HideWindow(); return; }
            closeAfterApply = true;
            _ = ApplySettingsAsync();
        };
        SelectPage(ViewPage(initial["view"]?.GetValue<string>(), "Overview"));
        // Overview counts down every second while something has a deadline; otherwise
        // Overview and Agents refresh every five seconds. Hidden windows never poll.
        refresh.Tick += async (_, _) =>
        {
            ticks++;
            if ((page != "Overview" && page != "Agents") || !AppWindow.IsVisible || saving) return;
            if (ticks % (page == "Overview" && HasDeadline ? 1 : 5) != 0) return;
            try { await bridge.SendAsync("refresh"); }
            catch (IOException) { refresh.Stop(); }
        };
        Closed += (_, _) => refresh.Stop();
        refresh.Start();
    }

    public void Receive(JsonObject message)
    {
        // The pipe is read from the UI synchronization context; use the dispatcher defensively.
        DispatcherQueue.TryEnqueue(() =>
        {
            if (message["error"] is JsonValue error)
            {
                var command = message["command"]?.GetValue<string>();
                if (command == "save")
                {
                    preferences.Reject();
                    this.changeTheme(draft.Theme);
                    closeAfterApply = false;
                    KeepScroll(ShowPage);
                    _ = ApplySettingsAsync();
                }
                // A refused control-center command leaves its switch or picker as it was.
                else if (page == "Overview") KeepScroll(ShowPage);
                Notify(command == "save" ? "Couldn't apply changes" : "Couldn't complete that", error.GetValue<string>(), InfoBarSeverity.Error);
                if (agentDialog is not null) agentDialog.Content = new TextBlock { Text = error.GetValue<string>(), TextWrapping = TextWrapping.Wrap };
                return;
            }
            if (message["snapshot"] is JsonObject next)
            {
                var previous = snapshot;
                snapshot = next;
                RefreshAgentDialog();
                if (message["type"]?.GetValue<string>() == "saved")
                {
                    preferences.Confirm(ReadPreferences());
                    this.changeTheme(draft.Theme);
                    _ = ApplySettingsAsync();
                    if (closeAfterApply && !preferences.HasChanges) HideWindow();
                }
                UpdateLivePage(previous);
            }
            if (message["type"]?.GetValue<string>() == "open")
            {
                preferences.Refresh(ReadPreferences());
                this.changeTheme(draft.Theme);
                SelectPage(ViewPage(message["view"]?.GetValue<string>(), page));
                closeAfterApply = false;
                refresh.Start();
                Activate();
            }
        });
    }

    /// Rebuilds the visible page only when its structure changed; times update in place so
    /// focus and scroll position survive the once-a-second refresh.
    private void UpdateLivePage(JsonObject previous)
    {
        if (page == "Overview")
        {
            if (OverviewShape(previous) != OverviewShape(snapshot)) KeepScroll(ShowPage);
            else UpdateOverview();
        }
        else if (page == "Agents" && (previous["settings"]?["agents"]?.ToJsonString() != snapshot["settings"]?["agents"]?.ToJsonString()
                 || AgentSessionShape(previous) != AgentSessionShape(snapshot)))
            KeepScroll(ShowPage);
    }

    private void KeepScroll(Action rebuild)
    {
        var offset = PageScroll.VerticalOffset;
        rebuild();
        PageScroll.UpdateLayout();
        PageScroll.ChangeView(null, offset, null, true);
    }

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint window);

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
        closeAfterApply = false;
        refresh.Stop();
        AppWindow.Hide();
    }

    public void SetTheme(string value) => WindowAppearance.Apply(Root, value);
    public void StopAppearance() => WindowAppearance.Stop(Root);

    // A construction smoke test of the actual WinUI pages, without showing a window or
    // requesting any engine operation. CI invokes this against inherited test pipes.
    public void VerifyPages()
    {
        LiveSettings.Verify();
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
                if (Descendants(Cards).OfType<NumberBox>().Any())
                    throw new InvalidOperationException($"{name} still uses a number field for a duration.");
                if (name == "Agents" && AgentClient("Codex") is JsonObject client)
                {
                    var permissions = AgentPermissionContent(client);
                    var grid = permissions.Children.OfType<Grid>().Single();
                    if (grid.Children.OfType<ToggleSwitch>().Count() != Actions.Count() + 1)
                        throw new InvalidOperationException("Agent permission controls are missing.");
                    var setup = AgentSetupContent("Codex", (snapshot["agentConnections"] as JsonArray)?.OfType<JsonObject>().FirstOrDefault());
                    if (!setup.Children.OfType<TextBox>().Any(t => t.IsReadOnly))
                        throw new InvalidOperationException("Agent setup has no configuration to copy.");
                }
            }
        }
        VerifyText();
        VerifyControlCenter();
        SelectPage("About Doze");
        if (Descendants(Cards).OfType<Image>().Count() != 2 || !Shows("Visit Clypy") || !Shows("Source Code") || !Shows("Open data folder") || !Shows("Made by Fayeed Pawaskar"))
            throw new InvalidOperationException("About is missing Doze's or Clypy's icon, or a link.");
        SelectPage("Advanced");
        if (!Shows("Reset preferences") || ResetDialog().PrimaryButtonText != "Reset" || ResetDialog().DefaultButton != ContentDialogButton.Close)
            throw new InvalidOperationException("Reset does not ask for confirmation.");
        if (!Shows("Copy the example command") || !Shows("Copy the PowerShell alias")
            || !Descendants(Cards).OfType<TextBlock>().Any(t => t.Text.StartsWith(@"& 'C:\Users\example\AppData\Local\Doze\doze-cli.exe' run --then sleep -- ", StringComparison.Ordinal)))
            throw new InvalidOperationException("Advanced does not show the doze-cli.exe command line.");
        SelectPage("Agents");
        foreach (var expected in new[] { "Keep sessions alive while connected", "5 minutes", "Waiting for your approval", "Connection lost · keeping awake", "Command line", "Running ffmpeg" })
            if (!Descendants(Cards).Any(e => (e as TextBlock)?.Text.Contains(expected) == true || (e as ComboBoxItem)?.Content as string == expected
                    || Microsoft.UI.Xaml.Automation.AutomationProperties.GetName(e) == expected))
                throw new InvalidOperationException($"Agents is missing \"{expected}\".");
        var before = draft.DefaultAwakeMinutes;
        draft.DefaultAwakeMinutes = 42;
        SelectPage("General");
        SelectPage("Session defaults");
        if (draft.DefaultAwakeMinutes != 42) throw new InvalidOperationException("Navigation discarded edits.");
        if (!Descendants(Cards).OfType<ComboBoxItem>().Any(item => item.Content as string == "42 minutes"))
            throw new InvalidOperationException("A saved custom duration is not selectable.");
        draft.DefaultAwakeMinutes = before;
        ResetDraft();
        if (draft.DefaultAwakeMinutes != 30) throw new InvalidOperationException("Reset did not fill defaults.");
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
        ["awake"] = true, ["awakeRemaining"] = 2520, ["whileAudio"] = true, ["holdingAwake"] = true, ["playbackEnabled"] = true,
        ["playbackPhase"] = "grace", ["selectedAction"] = "displayOff",
        ["timer"] = new JsonObject { ["action"] = "displayOff", ["remaining"] = 3900 },
        ["countdown"] = new JsonObject { ["action"] = "displayOff", ["remaining"] = 287, ["source"] = "timer" },
        ["message"] = "Snoozed for 15 minutes"
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
                         "More ways to keep awake", "More timer options", "Keep awake while audio plays", "Sleep after playback stops" })
                if (!Shows(name)) throw new InvalidOperationException($"Overview is missing \"{name}\".");
            if (Shows("Last event: Doze skill installed")) throw new InvalidOperationException("Overview shows skill messages as events.");
            if (Cards.Children.OfType<TextBlock>().Any(header => header.Text == "Overview"))
                throw new InvalidOperationException("Overview repeats its title.");

            var previous = snapshot.DeepClone().AsObject();
            snapshot["session"] = BusySession();
            UpdateLivePage(previous);
            foreach (var name in new[] { "Extend 15 minutes", "Stop keeping awake", "Stop timer", "Snooze 15 minutes", "Stay Awake",
                         "Cancel the action", "Turn display off in", "4:47", "Last event: Snoozed for 15 minutes", "Waiting for silence and inactivity" })
                if (!Shows(name)) throw new InvalidOperationException($"Overview is missing \"{name}\" during a session.");
            if (Shows("Keep awake for 15m")) throw new InvalidOperationException("Overview offers presets while keeping awake.");

            // A tick that only changes times updates text in place instead of rebuilding.
            var first = Cards.Children[0];
            previous = snapshot.DeepClone().AsObject();
            snapshot["session"]!["countdown"]!["remaining"] = 286;
            snapshot["session"]!["awakeRemaining"] = 2519;
            UpdateLivePage(previous);
            if (!ReferenceEquals(first, Cards.Children[0]) || !Shows("4:46"))
                throw new InvalidOperationException("Overview rebuilt instead of updating its clock.");
            if (!HasDeadline) throw new InvalidOperationException("Overview would not refresh every second.");
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
                foreach (var banned in new[] { "displayOff", "Display off", "Shutdown ", "awaiting_authorization", "connection_lost", "(seconds)" })
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
                        if (name == "Overview") await RenderSessionsAsync(directory, file);
                        if (label == "" && name == "Advanced") await RenderDialogAsync(ResetDialog(), theme, Path.Combine(directory, $"Advanced-Reset-{theme}.png"));
                        if (label == "" && name == "Agents" && AgentClient("Codex") is JsonObject client)
                            await RenderAgentDialogsAsync(directory, theme, client);
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
            if (PageScroll.ScrollableHeight > 0)
            {
                PageScroll.ChangeView(null, PageScroll.ScrollableHeight, null, true);
                await Task.Delay(150);
                await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"{file}-{variant}-end.png"));
            }
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

    private async Task RenderAgentDialogsAsync(string directory, ElementTheme theme, JsonObject client)
    {
        foreach (var permissions in new[] { false, true })
        {
            var dialog = new ContentDialog
            {
                XamlRoot = Root.XamlRoot, RequestedTheme = theme,
                Title = permissions ? "Codex permissions" : "Set up Codex", CloseButtonText = "Done",
                Content = permissions ? AgentPermissionContent(client) : AgentSetupContent("Codex", (snapshot["agentConnections"] as JsonArray)?.OfType<JsonObject>().FirstOrDefault())
            };
            var shown = dialog.ShowAsync();
            try { await Task.Delay(150); await VisualVerification.SaveAsync(dialog, Path.Combine(directory, $"Agents-{(permissions ? "Permissions" : "Setup")}-{theme}.png")); }
            finally { dialog.Hide(); await shown; }
        }
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

    private Preferences ReadPreferences() => snapshot["settings"]!.Deserialize<Preferences>(EngineBridge.Json)!;
    private static string ViewPage(string? view, string fallback) => view switch
    {
        "about" => "About Doze",
        "help" => "Menu guide",
        "agents" => "Agents",
        "settings" => "Overview",
        _ => fallback
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
        switch (page)
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
        PageScroll.ChangeView(null, 0, null, true);
    }

    private void Changed()
    {
        changeTheme(draft.Theme);
        Notice.IsOpen = false;
        _ = ApplySettingsAsync();
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

    private async Task ApplySettingsAsync()
    {
        if (verification) return;
        var change = preferences.BeginApply();
        if (change is null) return;
        try { await bridge.SendAsync("save", change); }
        catch (Exception error)
        {
            preferences.Reject();
            changeTheme(draft.Theme);
            ShowPage();
            Notify("Couldn't apply settings", error.Message, InfoBarSeverity.Error);
        }
    }

    private void ResetDraft()
    {
        var defaults = new Preferences();
        if (!Actions.Contains("sleep")) defaults.DefaultAction = defaults.PlaybackAction = Actions.First();
        preferences.Reset(defaults);
        ShowPage();
        Changed();
    }

    private void Notify(string title, string message, InfoBarSeverity severity) { Notice.Title = title; Notice.Message = message; Notice.Severity = severity; Notice.IsOpen = true; }
}
