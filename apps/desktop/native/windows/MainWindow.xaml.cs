using System.Diagnostics;
using System.Text.Json;
using System.Text.Json.Nodes;
using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Media;
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
    private readonly DispatcherTimer refresh = new() { Interval = TimeSpan.FromSeconds(5) };
    private TextBlock? overviewStatus;
    private TextBlock? overviewTimer;
    private static readonly string[] Pages = ["Overview", "General", "Session defaults", "After playback", "Notifications", "Agents", "Advanced", "Menu guide", "About Doze"];
    private static readonly Dictionary<string, string> ActionNames = new()
    {
        ["sleep"] = "Sleep",
        ["hibernate"] = "Hibernate",
        ["shutdown"] = "Shut down",
        ["lock"] = "Lock",
        ["displayOff"] = "Turn display off"
    };

    public MainWindow(EngineBridge bridge, JsonObject initial, Action<string>? changeTheme = null)
    {
        this.bridge = bridge;
        this.changeTheme = changeTheme ?? SetTheme;
        snapshot = initial["snapshot"]!.AsObject();
        preferences = new LiveSettings(ReadPreferences());
        InitializeComponent();
        WindowAppearance.Observe(this, Root);
        SetTheme(draft.Theme);
        Title = "Doze Settings";
        AppWindow.IsShownInSwitchers = true;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
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
        refresh.Tick += async (_, _) =>
        {
            if ((page != "Overview" && page != "Agents") || !AppWindow.IsVisible || saving) return;
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
                if (message["command"]?.GetValue<string>() == "save")
                {
                    preferences.Reject();
                    this.changeTheme(draft.Theme);
                    closeAfterApply = false;
                    var offset = PageScroll.VerticalOffset;
                    ShowPage();
                    PageScroll.ChangeView(null, offset, null, true);
                    _ = ApplySettingsAsync();
                }
                Notify("Couldn't apply changes", error.GetValue<string>(), InfoBarSeverity.Error);
                if (agentDialog is not null) agentDialog.Content = new TextBlock { Text = error.GetValue<string>(), TextWrapping = TextWrapping.Wrap };
                return;
            }
            if (message["snapshot"] is JsonObject next)
            {
                var agentChanged = snapshot["settings"]?["agents"]?.ToJsonString() != next["settings"]?["agents"]?.ToJsonString()
                    || snapshot["agentSessions"]?.ToJsonString() != next["agentSessions"]?.ToJsonString();
                snapshot = next;
                RefreshAgentDialog();
                if (page == "Agents" && agentChanged) { var offset = PageScroll.VerticalOffset; ShowPage(); PageScroll.ChangeView(null, offset, null, true); }
                if (page == "Overview")
                {
                    if (overviewStatus is not null) overviewStatus.Text = snapshot["status"]!.GetValue<string>();
                    if (overviewTimer is not null) overviewTimer.Text = snapshot["timerStatus"]!.GetValue<string>();
                }
                if (message["type"]?.GetValue<string>() == "saved")
                {
                    preferences.Confirm(ReadPreferences());
                    this.changeTheme(draft.Theme);
                    _ = ApplySettingsAsync();
                    if (closeAfterApply && !preferences.HasChanges) HideWindow();
                }
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

    [System.Runtime.InteropServices.DllImport("user32.dll")]
    private static extern uint GetDpiForWindow(nint window);

    // Size in effective pixels for the window's DPI, centered and kept within the work area.
    private void FitToDisplay(int width, int height)
    {
        var scale = GetDpiForWindow(WinRT.Interop.WindowNative.GetWindowHandle(this)) / 96.0;
        var area = Microsoft.UI.Windowing.DisplayArea.GetFromWindowId(AppWindow.Id, Microsoft.UI.Windowing.DisplayAreaFallback.Primary).WorkArea;
        var fittedWidth = Math.Min((int)(width * scale), area.Width * 92 / 100);
        var fittedHeight = Math.Min((int)(height * scale), area.Height * 92 / 100);
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
                if (!ReferenceEquals(SystemBackdrop, windowBackdrop))
                    throw new InvalidOperationException($"Navigation replaced the window backdrop on {name}.");
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
        var before = draft.DefaultAwakeMinutes;
        draft.DefaultAwakeMinutes = 42;
        SelectPage("General");
        SelectPage("Session defaults");
        if (draft.DefaultAwakeMinutes != 42) throw new InvalidOperationException("Navigation discarded edits.");
        draft.DefaultAwakeMinutes = before;
        ResetDraft();
        if (draft.DefaultAwakeMinutes != 30) throw new InvalidOperationException("Reset did not fill defaults.");
    }

    public async Task RenderVerificationAsync(string directory)
    {
        Directory.CreateDirectory(directory);
        // Render only this app's visual tree. The desktop and other windows are never read.
        AppWindow.Show(false);
        try
        {
            foreach (var theme in new[] { ElementTheme.Light, ElementTheme.Dark })
            {
                Root.RequestedTheme = theme;
                foreach (var name in new[] { "Session defaults", "Agents", "Menu guide", "About Doze" })
                {
                    SelectPage(name);
                    await Task.Delay(150);
                    await VisualVerification.SaveAsync(Root, Path.Combine(directory, $"{name.Replace(' ', '-')}-{theme}.png"));
                    if (name == "Agents" && AgentClient("Codex") is JsonObject client)
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
                }
            }
        }
        finally { AppWindow.Hide(); }
    }

    private Preferences ReadPreferences() => snapshot["settings"]!.Deserialize<Preferences>(EngineBridge.Json)!;
    private static string ViewPage(string? view, string fallback) => view switch
    {
        "about" => "About Doze",
        "help" => "Menu guide",
        "agents" => "Agents",
        _ => fallback
    };
    private IEnumerable<string> Actions => snapshot["actions"]!.AsArray().Select(action => action!.GetValue<string>());
    private bool Capability(string name) => snapshot[name]?.GetValue<bool>() == true;

    private void Navigate(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is NavigationViewItem item) { page = (string)item.Tag; ShowPage(); }
    }

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
        Cards.Children.Clear();
        PageTitle.Text = page;
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

    private void Overview()
    {
        PageDescription.Text = "A little more awake when you need it. A little more rest when you don't.";
        overviewStatus = Card("Current session", snapshot["status"]?.GetValue<string>() ?? "Normal sleep allowed", "\uE708");
        overviewTimer = Card("Power action", snapshot["timerStatus"]?.GetValue<string>() ?? "No power action scheduled", "\uE823");
        Section("Quick access");
        Card("Preview the countdown", "Try Cancel and Snooze without scheduling a power action.", "\uE768", ActionButton("Preview", async () => await bridge.SendAsync("preview")));
        Card("Session defaults", "Choose default durations, power actions, and whether the display can sleep.", "\uE713", ActionButton("Open", () => { SelectPage("Session defaults"); return Task.CompletedTask; }));
        Card("Local data", "Open the folder containing your preferences and optional diagnostic logs.", "\uE8B7", ActionButton("Open folder", OpenData));
        Section("How Doze works");
        Card("Everything starts in the tray", "Keep Awake, Power Timer, After Playback, and Quick Settings are available from Doze's moon icon. Disabled actions explain why they aren't available.", "\uE946");
        Card("Windows stays in control", "Normal sleep allowed means Doze is not blocking automatic sleep. No keyboard or mouse activity is simulated. Restarting or waking the computer clears sessions.", "\uE7F4");
    }

    private void General()
    {
        PageDescription.Text = "Choose how Doze starts and where it lives.";
        Toggle("Launch at sign-in", "Start Doze automatically when you sign in to Windows.", "\uE7E8", draft.LaunchAtStartup, value => draft.LaunchAtStartup = value, Capability("startupSupported"));
        Toggle("Start in the tray", "Keep this window closed at launch. Start sessions from the tray menu.", "\uE73F", draft.StartMinimized, value => draft.StartMinimized = value);
        var appearance = new ComboBox { MinWidth = 160, ItemsSource = new[] { "System", "Light", "Dark" }, SelectedItem = draft.Theme switch { "light" => "Light", "dark" => "Dark", _ => "System" } };
        AutomationProperties.SetName(appearance, "Appearance");
        appearance.SelectionChanged += (_, _) =>
        {
            draft.Theme = ((string)appearance.SelectedItem).ToLowerInvariant();
            Changed();
        };
        Card("Appearance", "System follows your Windows theme automatically. Light or Dark overrides it for Doze windows. Changes apply immediately.", "\uE790", appearance);
        Card("Session safety", "Sessions are cleared after restart or suspend/resume. Closing Settings keeps Doze and existing sessions running.", "\uE72E");
    }

    private void SessionDefaults()
    {
        PageDescription.Text = "Defaults for sessions started from the tray. Existing timers keep their deadlines.";
        Toggle("Allow display sleep", "Let Windows turn off the screen while Doze keeps the computer awake. Changes apply immediately.", "\uE7F4", draft.AllowDisplaySleep, value => draft.AllowDisplaySleep = value);
        Number("Keep Awake duration", "Default length in minutes. Between 1 minute and 7 days.", "\uE708", draft.DefaultAwakeMinutes, 1, 10080, value => draft.DefaultAwakeMinutes = value);
        Number("Power Timer duration", "Default time in minutes before the final countdown begins.", "\uE823", draft.DefaultTimerMinutes, 1, 10080, value => draft.DefaultTimerMinutes = value);
        Action("Timer action", "The action performed after a timer and its final countdown finish.", draft.DefaultAction, value => draft.DefaultAction = value);
        ActionGuide();
    }

    private void AfterPlayback()
    {
        PageDescription.Text = "Wait for ongoing audio to stop and the computer to become idle before acting.";
        Action("After playback action", "Enable After Playback from the tray. Brief sounds and silence alone cannot arm it.", draft.PlaybackAction, value => draft.PlaybackAction = value);
        Number("Silence grace period", "Seconds of silence to tolerate between tracks or while buffering.", "\uE995", draft.SilenceSeconds, 10, 3600, value => draft.SilenceSeconds = value);
        Number("Required idle time", "Seconds without keyboard or mouse activity. Both silence and idle checks must pass.", "\uE916", draft.IdleSeconds, 30, 7200, value => draft.IdleSeconds = value);
        Card("Your activity takes priority", "Pausing playback or using the computer restarts the inactivity wait. Resumed audio restarts the silence wait; input during the warning cancels it. Manual Keep Awake blocks the action. An explicit Power Timer takes priority.", "\uE72E");
        if (!Capability("audioSupported")) Card("Audio monitoring unavailable", "After Playback is not available on this platform.", "\uE7BA");
    }

    private void Notifications()
    {
        PageDescription.Text = "A final warning before Doze performs a power action.";
        Toggle("Countdown notifications", "Show a native Windows notification when a countdown starts. The warning window remains available even when notifications are off.", "\uEA8F", draft.Notifications, value => draft.Notifications = value);
        Number("Final countdown", "Seconds to Cancel or Snooze. Applies to both timers and After Playback.", "\uE823", draft.CountdownSeconds, 15, 1800, value => draft.CountdownSeconds = value);
        Card("Try the warning", "A preview never triggers a power action. Cancel and Snooze dismiss the demonstration.", "\uE768", ActionButton("Preview", async () => await bridge.SendAsync("preview")));
        Card("Cancel and Snooze", "Closing the real warning window or pressing Escape cancels the action. Snooze adds 15 minutes. A real countdown always takes priority over a preview.", "\uE946");
    }

    private void Agents()
    {
        PageDescription.Text = "Keep your computer awake while agents work, then let Doze handle completion safely.";
        var settings = snapshot["settings"]?["agents"] as JsonObject;
        var enabled = settings?["enabled"]?.GetValue<bool>() == true;
        var enable = new ToggleSwitch { IsOn = enabled, OnContent = "", OffContent = "" };
        AutomationProperties.SetName(enable, "Enable MCP");
        enable.Toggled += async (_, _) => await bridge.SendAgentAsync("agent-enable");
        Card("Enable MCP", "Agents connect locally. New profiles need your approval before keeping awake or requesting a power action.", "\uE716", enable);
        var leases = new ComboBox { Width = 160, ItemsSource = new[] { 60, 300, 900, 1800, 3600 }, SelectedItem = settings?["leaseSeconds"]?.GetValue<int>() ?? 300 };
        leases.SelectionChanged += async (_, _) => { if (leases.SelectedItem is int seconds) await bridge.SendAgentAsync("agent-lease", seconds: seconds); };
        Card("Default lease (seconds)", "Agents must heartbeat before this expires. A lost connection keeps the computer awake for up to 30 minutes, then releases without a completion action.", "\uE823", leases);
        var completion = new ComboBox { Width = 180 };
        completion.Items.Add(new ComboBoxItem { Content = "Return to normal", Tag = "normal" });
        foreach (var action in Actions) completion.Items.Add(new ComboBoxItem { Content = ActionNames[action], Tag = action });
        var defaultAction = settings?["defaultCompletion"]?.GetValue<string>() ?? "normal";
        completion.SelectedItem = completion.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == defaultAction);
        completion.SelectionChanged += async (_, _) => { if (completion.SelectedItem is ComboBoxItem item) await bridge.SendAgentAsync("agent-default", action: (string)item.Tag == "normal" ? null : (string)item.Tag); };
        Card("Default completion behavior", "Power actions always require permission. Conflicting requests return to normal. Agent countdowns last at least five minutes.", "\uE708", completion);
        Section("Agent connections");
        foreach (var name in new[] { "Codex", "Claude Code", "Generic MCP client" })
        {
            var client = (settings?["clients"] as JsonArray)?.OfType<JsonObject>().FirstOrDefault(c => c["name"]?.GetValue<string>() == name);
            var controls = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
            controls.Children.Add(ActionButton(client is null ? "Set up" : "Configure", () => OpenAgentSetupAsync(name)));
            if (client is not null) controls.Children.Add(ActionButton("Permissions", () => OpenAgentPermissionsAsync(name)));
            Card(name, client is null ? "Set up a local connection to Doze." : "Profile created · " + (client["keepAwake"]?.GetValue<bool>() == true ? "Keep awake allowed" : "Ask before keeping awake"), "\uE8A7", controls);
        }
        Section("Sessions");
        if (snapshot["agentSessions"] is JsonArray sessions)
            foreach (var session in sessions.OfType<JsonObject>().Where(s => new[] { "active", "connection_lost", "awaiting_authorization" }.Contains(s["status"]!.GetValue<string>())))
            {
                var id = session["session_id"]!.GetValue<string>();
                var status = session["status"]!.GetValue<string>();
                var whenDone = session["completion_action"]?.GetValue<string>();
                var action = whenDone is null ? "Return to normal" : ActionNames.GetValueOrDefault(whenDone, whenDone);
                var controls = new StackPanel { Orientation = Orientation.Horizontal, Spacing = 8 };
                if (status == "awaiting_authorization")
                    foreach (var (decision, label) in new[] { ("once", "Allow Once"), ("deny", "Deny") })
                        controls.Children.Add(ActionButton(label, () => bridge.SendAgentAsync("agent-authorize", id: id, decision: decision)));
                else
                {
                    controls.Children.Add(ActionButton("Cancel session", () => bridge.SendAgentAsync("agent-cancel", id: id)));
                    if (status == "connection_lost")
                    {
                        var resolve = new Button { Content = "Resolve" };
                        var menu = new MenuFlyout();
                        var wait = new MenuFlyoutItem { Text = "Wait 30 minutes" };
                        wait.Click += async (_, _) => await bridge.SendAgentAsync("agent-wait", id: id);
                        var end = new MenuFlyoutItem { Text = "End and apply completion action" };
                        end.Click += async (_, _) => await bridge.SendAgentAsync("agent-finish", id: id);
                        menu.Items.Add(wait); menu.Items.Add(end); resolve.Flyout = menu;
                        controls.Children.Add(resolve);
                    }
                }
                var lastActivity = Math.Max(0, (snapshot["agentNow"]?.GetValue<long>() ?? 0) - session["last_heartbeat"]!.GetValue<long>()) / 60;
                Card(session["client_name"]!.GetValue<string>(), $"{session["reason"]!.GetValue<string>()}\n{status.Replace('_', ' ')} · last activity {lastActivity}m ago\nWhen finished: {action}", "\uE716", controls);
            }
        if ((snapshot["agentSessions"] as JsonArray)?.OfType<JsonObject>().Any(s => new[] { "active", "connection_lost", "awaiting_authorization" }.Contains(s["status"]?.GetValue<string>())) != true)
            Cards.Children.Add(new TextBlock { Text = "No active agent sessions", Opacity = 0.7, Margin = new Thickness(0, 4, 0, 12) });
    }

    private void Advanced()
    {
        PageDescription.Text = "Diagnostics and local data. No account, recording, or cloud service.";
        Toggle("Local diagnostic logs", "Record errors for troubleshooting. Off by default; stored locally and bounded in size.", "\uE9D9", draft.Logging, value => draft.Logging = value);
        Card("Data folder", snapshot["settingsPath"]!.GetValue<string>(), "\uE8B7", ActionButton("Open folder", OpenData));
        Card("Available power actions", string.Join(" · ", Actions.Select(action => ActionNames.GetValueOrDefault(action, action))), "\uE7E8");
        Card("Audio monitoring", Capability("audioSupported") ? "Available. Output levels are observed; audio is never recorded." : "Unavailable on this platform.", "\uE995");
        Card("Reset preferences", "Restore default preferences immediately. Existing timers keep their deadlines.", "\uE777", ActionButton("Reset defaults", () => { ResetDraft(); return Task.CompletedTask; }));
    }

    private void About()
    {
        PageDescription.Text = "Your computer knows when it's bedtime.";
        Card("Doze", $"Version {snapshot["version"]!.GetValue<string>()} · Windows {System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture}", "\uE708");
        Card("A quiet desktop companion", "Keep your computer awake when you need it, then let it rest. Native tray controls, audio-aware rules, power timers, and a shared final countdown.", "\uE946");
        Section("Local and private");
        Card("No account. No cloud. No ads.", "Doze does not record audio, simulate input, or send telemetry. Your preferences and optional diagnostic logs stay on this computer.", "\uE72E");
        Section("Built with");
        Card("Rust and Tauri", "The Rust engine owns sessions, validation, Core Audio, native power requests, startup settings, and countdown safety.", "\uE7F4");
        Card("Windows App SDK and WinUI 3", "Native NavigationView, settings cards, ToggleSwitch, NumberBox, ComboBox, and system Mica/Acrylic materials. Open-source components retain their respective licenses.", "\uE713");
        Card("Preferences and diagnostics", snapshot["settingsPath"]!.GetValue<string>(), "\uE8B7", ActionButton("Open folder", OpenData));
    }

    private void MenuGuide()
    {
        PageDescription.Text = "What each tray option means and why some commands are unavailable.";
        Card("Current state", snapshot["status"]!.GetValue<string>(), "\uE708");
        Section("Status and disabled commands");
        Card("Normal sleep allowed", "Doze is not holding the computer awake. Your normal Windows power settings apply. The top two tray rows report current status; clicking either opens this guide.", "\uE946");
        Card("Why an option is grey", "Stop needs an active session. Extend needs a timed awake session. Stop timer needs a timer. Cancel and Snooze need a running countdown. Unsupported power actions are also disabled.", "\uE7BA");
        Section("Sessions and timers");
        Card("Keep Awake", $"Choose a duration, a local end time, or indefinitely. Default starts a {draft.DefaultAwakeMinutes}-minute session. Stop ends manual and audio-based awake sessions; Extend adds 15 minutes to a timed session.", "\uE708");
        Card("Keep awake while audio plays", $"Keeps the computer awake during audible output and brief pauses, then releases the request after {draft.SilenceSeconds} seconds of silence. A checkmark means the rule is enabled; it may still be waiting for audio.", "\uE995");
        Card("Power Timer", $"Select an action and duration. The default is {draft.DefaultTimerMinutes} minutes, followed by a {draft.CountdownSeconds}-second final warning. The computer stays awake while the timer runs. Stop timer removes the timer and its warning.", "\uE823");
        ActionGuide();
        Card("After Playback", $"First waits for ongoing audio, then requires {draft.SilenceSeconds} seconds of silence and {draft.IdleSeconds} seconds without keyboard or mouse activity. Brief sounds and silence alone cannot arm it. Resumed playback or input cancels its countdown. Manual Keep Awake blocks it; an explicit Power Timer takes priority.", "\uE916");
        Section("Warnings and preferences");
        Card("Countdown, Cancel and Snooze", "The native warning shows the action and remaining time. Cancel, Escape or closing the warning removes the action. Snooze adds 15 minutes. Preview demonstrates the warning and cannot perform a power action.", "\uEA8F");
        Card("Quick Settings", "Checkmarks show saved preferences. Changes apply immediately. Duration defaults affect new sessions; display sleep updates the active awake request. Notifications apply to future countdowns.", "\uE713");
        Card("Settings and Reset", "Settings apply as you change them. Advanced contains Reset defaults, diagnostics and your data folder. Reset applies immediately; running timers keep their deadlines.", "\uE777");
        Card("Quit Doze", "Stops Doze and its awake sessions. Windows resumes its normal sleep behavior. Transient sessions are cleared after restart or suspend/resume.", "\uE7E8");
    }

    private void ActionGuide() => Card("What each action means", "Sleep keeps your session in memory. Hibernate saves it to disk. Shut down closes Windows; unsaved work may need attention. Lock secures your session. Turn display off switches off the screen.", "\uE946");
    private void Section(string title) => Cards.Children.Add(new TextBlock { Text = title, FontSize = 16, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, Margin = new Thickness(0, 20, 0, 8) });

    private TextBlock Card(string title, string description, string glyph, FrameworkElement? control = null)
    {
        var grid = new Grid { ColumnSpacing = 16 };
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(28) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = new GridLength(1, GridUnitType.Star) });
        grid.ColumnDefinitions.Add(new ColumnDefinition { Width = GridLength.Auto });
        grid.Children.Add(new FontIcon { Glyph = glyph, FontSize = 20, VerticalAlignment = VerticalAlignment.Center });
        var text = new StackPanel { Spacing = 4, VerticalAlignment = VerticalAlignment.Center };
        text.Children.Add(new TextBlock { Text = title, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold, TextWrapping = TextWrapping.Wrap });
        var descriptionText = new TextBlock { Text = description, Opacity = 0.7, FontSize = 12, TextWrapping = TextWrapping.Wrap };
        text.Children.Add(descriptionText);
        Grid.SetColumn(text, 1);
        grid.Children.Add(text);
        if (control is not null) { Grid.SetColumn(control, 2); control.VerticalAlignment = VerticalAlignment.Center; grid.Children.Add(control); }
        Cards.Children.Add(new Border
        {
            Child = grid,
            Style = (Style)Application.Current.Resources["SettingsCardStyle"]
        });
        return descriptionText;
    }

    private void Changed()
    {
        changeTheme(draft.Theme);
        Notice.IsOpen = false;
        _ = ApplySettingsAsync();
    }

    private void Toggle(string title, string description, string glyph, bool value, Action<bool> apply, bool enabled = true)
    {
        var toggle = new ToggleSwitch { IsOn = value, IsEnabled = enabled, MinWidth = 44, OnContent = "", OffContent = "" };
        AutomationProperties.SetName(toggle, title);
        toggle.Toggled += (_, _) => { apply(toggle.IsOn); Changed(); };
        Card(title, description, glyph, toggle);
    }

    private void Number(string title, string description, string glyph, int value, int minimum, int maximum, Action<int> apply)
    {
        var number = new NumberBox { Value = value, Minimum = minimum, Maximum = maximum, SmallChange = 1, SpinButtonPlacementMode = NumberBoxSpinButtonPlacementMode.Inline, Width = 160 };
        AutomationProperties.SetName(number, title);
        number.ValueChanged += (_, args) =>
        {
            if (!double.IsFinite(args.NewValue) || args.NewValue < minimum || args.NewValue > maximum || args.NewValue != Math.Truncate(args.NewValue))
            {
                number.Value = args.OldValue;
                Notify("Invalid value", $"Choose a whole number between {minimum} and {maximum}.", InfoBarSeverity.Error);
                return;
            }
            apply((int)args.NewValue);
            Changed();
        };
        Card(title, description, glyph, number);
    }

    private void Action(string title, string description, string value, Action<string> apply)
    {
        var combo = new ComboBox { Width = 180 };
        AutomationProperties.SetName(combo, title);
        foreach (var action in Actions) combo.Items.Add(new ComboBoxItem { Content = ActionNames.GetValueOrDefault(action, action), Tag = action });
        combo.SelectedItem = combo.Items.OfType<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == value);
        combo.SelectionChanged += (_, _) => { if (combo.SelectedItem is ComboBoxItem item) { apply((string)item.Tag); Changed(); } };
        Card(title, description, "\uE7E8", combo);
    }

    private Button ActionButton(string label, Func<Task> action)
    {
        var button = new Button { Content = label };
        button.Click += async (_, _) => { try { await action(); } catch (Exception error) { Notify("Couldn't complete action", error.Message, InfoBarSeverity.Error); } };
        return button;
    }

    private Task OpenData()
    {
        var folder = Path.GetDirectoryName(snapshot["settingsPath"]!.GetValue<string>())!;
        Directory.CreateDirectory(folder);
        Process.Start(new ProcessStartInfo(folder) { UseShellExecute = true });
        return Task.CompletedTask;
    }

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
    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args) { if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput) sender.ItemsSource = Pages.Where(name => name.Contains(sender.Text, StringComparison.OrdinalIgnoreCase)).ToArray(); }
    private void SearchChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args) => SelectPage((string)args.SelectedItem);
    private void SearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args) { var match = Pages.FirstOrDefault(name => name.Contains(args.QueryText, StringComparison.OrdinalIgnoreCase)); if (match is not null) SelectPage(match); }
}
