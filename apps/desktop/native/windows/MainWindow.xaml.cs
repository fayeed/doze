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
    private JsonObject snapshot;
    private Preferences draft;
    private Preferences saved;
    private string page = "Overview";
    private bool saving;
    private bool dirty;
    private readonly DispatcherTimer refresh = new() { Interval = TimeSpan.FromSeconds(5) };
    private TextBlock? overviewStatus;
    private TextBlock? overviewTimer;
    private static readonly string[] Pages = ["Overview", "General", "Session defaults", "After playback", "Notifications", "Advanced", "About Doze"];
    private static readonly Dictionary<string, string> ActionNames = new()
    {
        ["sleep"] = "Sleep",
        ["hibernate"] = "Hibernate",
        ["shutdown"] = "Shut down",
        ["lock"] = "Lock",
        ["displayOff"] = "Turn display off"
    };

    public MainWindow(EngineBridge bridge, JsonObject initial)
    {
        this.bridge = bridge;
        snapshot = initial["snapshot"]!.AsObject();
        saved = ReadPreferences();
        draft = saved with { };
        InitializeComponent();
        Title = "Doze Settings";
        AppWindow.IsShownInSwitchers = true;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "Doze.ico"));
        ExtendsContentIntoTitleBar = true;
        SetTitleBar(TitleBar);
        SystemBackdrop = new MicaBackdrop { Kind = Microsoft.UI.Composition.SystemBackdrops.MicaKind.BaseAlt };
        AppWindow.Resize(new SizeInt32(1120, 780));
        SelectPage(initial["view"]?.GetValue<string>() == "about" ? "About Doze" : "Overview");
        refresh.Tick += async (_, _) =>
        {
            if (page != "Overview" || !AppWindow.IsVisible || saving) return;
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
                saving = false;
                SetBusy(false);
                Notify("Couldn't apply changes", error.GetValue<string>(), InfoBarSeverity.Error);
                return;
            }
            if (message["snapshot"] is JsonObject next)
            {
                snapshot = next;
                if (page == "Overview")
                {
                    if (overviewStatus is not null) overviewStatus.Text = snapshot["status"]!.GetValue<string>();
                    if (overviewTimer is not null) overviewTimer.Text = snapshot["timerStatus"]!.GetValue<string>();
                }
                if (message["type"]?.GetValue<string>() == "saved")
                {
                    saved = ReadPreferences();
                    draft = saved with { };
                    dirty = false;
                    saving = false;
                    SetBusy(false);
                    ShowPage();
                    Notify("Settings saved", "Your preferences are stored on this computer.", InfoBarSeverity.Success);
                }
            }
            if (message["type"]?.GetValue<string>() == "open")
            {
                if (!dirty) { saved = ReadPreferences(); draft = saved with { }; }
                SelectPage(message["view"]?.GetValue<string>() == "about" ? "About Doze" : page);
                Activate();
            }
        });
    }

    // A construction smoke test of the actual WinUI pages, without showing a window or
    // requesting any engine operation. CI invokes this against inherited test pipes.
    public void VerifyPages()
    {
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
                foreach (var name in new[] { "Session defaults", "About Doze" })
                {
                    SelectPage(name);
                    await Task.Delay(150);
                    Root.UpdateLayout();
                    var bitmap = new Microsoft.UI.Xaml.Media.Imaging.RenderTargetBitmap();
                    await bitmap.RenderAsync(Root);
                    var pixels = await bitmap.GetPixelsAsync();
                    var bytes = new byte[pixels.Length];
                    using (var reader = Windows.Storage.Streams.DataReader.FromBuffer(pixels)) reader.ReadBytes(bytes);
                    using var stream = new Windows.Storage.Streams.InMemoryRandomAccessStream();
                    var encoder = await Windows.Graphics.Imaging.BitmapEncoder.CreateAsync(Windows.Graphics.Imaging.BitmapEncoder.PngEncoderId, stream);
                    encoder.SetPixelData(Windows.Graphics.Imaging.BitmapPixelFormat.Bgra8, Windows.Graphics.Imaging.BitmapAlphaMode.Premultiplied, (uint)bitmap.PixelWidth, (uint)bitmap.PixelHeight, 96, 96, bytes);
                    await encoder.FlushAsync();
                    using var fileReader = new Windows.Storage.Streams.DataReader(stream.GetInputStreamAt(0));
                    await fileReader.LoadAsync((uint)stream.Size);
                    var png = new byte[(int)stream.Size];
                    fileReader.ReadBytes(png);
                    await File.WriteAllBytesAsync(Path.Combine(directory, $"{name.Replace(' ', '-')}-{theme}.png"), png);
                }
            }
        }
        finally { AppWindow.Hide(); }
    }

    private Preferences ReadPreferences() => snapshot["settings"]!.Deserialize<Preferences>(EngineBridge.Json)!;
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
        Footer.Visibility = page == "About Doze" ? Visibility.Collapsed : Visibility.Visible;
        SaveHint.Text = dirty ? "You have unsaved changes." : "Changes apply when you save.";
        switch (page)
        {
            case "Overview": Overview(); break;
            case "General": General(); break;
            case "Session defaults": SessionDefaults(); break;
            case "After playback": AfterPlayback(); break;
            case "Notifications": Notifications(); break;
            case "Advanced": Advanced(); break;
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
        Card("Appearance", "Native Windows controls follow your Windows light, dark, and accessibility settings. Settings and About share the same Mica backdrop; countdown panels use Acrylic.", "\uE790");
        Card("Session safety", "Sessions are cleared after restart or suspend/resume. Closing Settings keeps Doze and existing sessions running.", "\uE72E");
    }

    private void SessionDefaults()
    {
        PageDescription.Text = "Defaults for sessions started from the tray. Existing timers keep their deadlines.";
        Toggle("Allow display sleep", "Let Windows turn off the screen while Doze keeps the computer awake. Applies immediately after saving.", "\uE7F4", draft.AllowDisplaySleep, value => draft.AllowDisplaySleep = value);
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
        Card("Your activity takes priority", "Resumed audio or user activity cancels a pending playback action. Manual Keep Awake blocks it. An explicit Power Timer takes priority.", "\uE72E");
        if (!Capability("audioSupported")) Card("Audio monitoring unavailable", "After Playback is not available on this platform.", "\uE7BA");
    }

    private void Notifications()
    {
        PageDescription.Text = "A final warning before Doze performs a power action.";
        Toggle("Countdown notifications", "Show a native Windows notification when a countdown starts. The warning window remains available even when notifications are off.", "\uEA8F", draft.Notifications, value => draft.Notifications = value);
        Number("Final countdown", "Seconds to Cancel or Snooze. Applies to both timers and After Playback.", "\uE823", draft.CountdownSeconds, 15, 1800, value => draft.CountdownSeconds = value);
        Card("Try the warning", "A preview never triggers a power action. Cancel closes it; Snooze extends the demonstration.", "\uE768", ActionButton("Preview", async () => await bridge.SendAsync("preview")));
        Card("Cancel and Snooze", "Closing the real warning window or pressing Escape cancels the action. Snooze adds 15 minutes. A real countdown always takes priority over a preview.", "\uE946");
    }

    private void Advanced()
    {
        PageDescription.Text = "Diagnostics and local data. No account, recording, or cloud service.";
        Toggle("Local diagnostic logs", "Record errors for troubleshooting. Off by default; stored locally and bounded in size.", "\uE9D9", draft.Logging, value => draft.Logging = value);
        Card("Data folder", snapshot["settingsPath"]!.GetValue<string>(), "\uE8B7", ActionButton("Open folder", OpenData));
        Card("Available power actions", string.Join(" · ", Actions.Select(action => ActionNames.GetValueOrDefault(action, action))), "\uE7E8");
        Card("Audio monitoring", Capability("audioSupported") ? "Available. Output levels are observed; audio is never recorded." : "Unavailable on this platform.", "\uE995");
        Card("Reset preferences", "Reset fills in defaults for review. Existing preferences change only after Save; sessions are not restored from disk.", "\uE777", ActionButton("Reset defaults", () => { ResetDraft(); return Task.CompletedTask; }));
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

    private void Changed() { dirty = true; SaveHint.Text = "You have unsaved changes."; Notice.IsOpen = false; }

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
        number.ValueChanged += (_, args) => { apply(double.IsFinite(args.NewValue) ? (int)args.NewValue : 0); Changed(); };
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

    private async void Save(object sender, RoutedEventArgs args)
    {
        if (saving) return;
        saving = true;
        SetBusy(true);
        try { await bridge.SendAsync("save", draft); }
        catch (Exception error) { saving = false; SetBusy(false); Notify("Couldn't save settings", error.Message, InfoBarSeverity.Error); }
    }

    private void SetBusy(bool busy) { SaveButton.IsEnabled = ResetButton.IsEnabled = DiscardButton.IsEnabled = !busy; Navigation.IsEnabled = !busy; }
    private void ResetDraft() { draft = new Preferences(); if (!Actions.Contains("sleep")) draft.DefaultAction = draft.PlaybackAction = Actions.First(); dirty = true; ShowPage(); }
    private void Reset(object sender, RoutedEventArgs args) => ResetDraft();
    private void Discard(object sender, RoutedEventArgs args) { draft = saved with { }; dirty = false; ShowPage(); }
    private void Notify(string title, string message, InfoBarSeverity severity) { Notice.Title = title; Notice.Message = message; Notice.Severity = severity; Notice.IsOpen = true; }
    private void SearchChanged(AutoSuggestBox sender, AutoSuggestBoxTextChangedEventArgs args) { if (args.Reason == AutoSuggestionBoxTextChangeReason.UserInput) sender.ItemsSource = Pages.Where(name => name.Contains(sender.Text, StringComparison.OrdinalIgnoreCase)).ToArray(); }
    private void SearchChosen(AutoSuggestBox sender, AutoSuggestBoxSuggestionChosenEventArgs args) => SelectPage((string)args.SelectedItem);
    private void SearchSubmitted(AutoSuggestBox sender, AutoSuggestBoxQuerySubmittedEventArgs args) { var match = Pages.FirstOrDefault(name => name.Contains(args.QueryText, StringComparison.OrdinalIgnoreCase)); if (match is not null) SelectPage(match); }
}
