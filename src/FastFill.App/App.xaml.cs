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
        if (window.AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.Maximize();
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
