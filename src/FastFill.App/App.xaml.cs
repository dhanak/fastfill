using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;

namespace FastFill.App;

public partial class App : Application
{
    private Window? _window;

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        var window = new MainWindow();
        _window = window;
        window.Activate();
        try
        {
            if (window.AppWindow.Presenter is OverlappedPresenter presenter)
            {
                presenter.Maximize();
            }
        }
        catch (Exception exception)
        {
            // Presenter failure must not block launch.
            System.Diagnostics.Debug.WriteLine(exception);
        }

        var projectPath = Environment.GetCommandLineArgs()
            .Skip(1)
            .FirstOrDefault(path => string.Equals(
                Path.GetExtension(path),
                ".docscan",
                StringComparison.OrdinalIgnoreCase));
        if (projectPath is not null)
        {
            _ = window.OpenProjectFromPathAsync(projectPath);
        }
    }
}
