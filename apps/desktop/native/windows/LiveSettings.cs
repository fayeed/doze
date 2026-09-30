namespace Doze.SettingsUi;

// One request is in flight at a time. Acknowledging it must preserve newer edits.
internal sealed class LiveSettings(Preferences initial)
{
    public Preferences Draft { get; private set; } = initial with { };
    public Preferences Confirmed { get; private set; } = initial with { };
    private Preferences? pending;

    public bool IsApplying => pending is not null;
    public bool HasChanges => Draft != Confirmed;

    public Preferences? BeginApply()
    {
        if (IsApplying || !HasChanges) return null;
        pending = Draft with { };
        return pending with { };
    }

    public void Confirm(Preferences saved)
    {
        if (Draft == pending) Draft = saved with { };
        Confirmed = saved with { };
        pending = null;
    }

    public void Reject()
    {
        if (Draft == pending) Draft = Confirmed with { };
        pending = null;
    }

    public void Refresh(Preferences saved)
    {
        if (!HasChanges && !IsApplying) Draft = saved with { };
        Confirmed = saved with { };
    }

    public void Reset(Preferences defaults) => Draft = defaults with { };

    public static void Verify()
    {
        var settings = new LiveSettings(new Preferences());
        settings.Draft.Notifications = false;
        var first = settings.BeginApply() ?? throw new InvalidOperationException("Edits were not applied.");
        settings.Draft.DefaultAwakeMinutes = 42;
        if (settings.BeginApply() is not null) throw new InvalidOperationException("Concurrent settings writes.");
        settings.Confirm(first);
        var second = settings.BeginApply() ?? throw new InvalidOperationException("Newer edits were lost.");
        if (second.DefaultAwakeMinutes != 42 || second.Notifications)
            throw new InvalidOperationException("Rapid edits did not preserve all changes.");
        settings.Reject();
        if (settings.HasChanges) throw new InvalidOperationException("Rejected settings were not restored.");
        settings.Reset(new Preferences());
        if (settings.BeginApply() is null) throw new InvalidOperationException("Reset did not apply defaults.");
    }
}
