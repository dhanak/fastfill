namespace FastFill.Core;

public enum CameraPosition
{
    Unknown,
    Front,
    Back,
}

public sealed record FramePacket(
    DateTimeOffset Timestamp,
    byte[] Bgra32,
    int Width,
    int Height,
    int Stride,
    int RotationDegrees,
    bool Mirrored,
    CameraPosition Camera)
{
    public void Validate()
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(Width);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(Height);
        ArgumentOutOfRangeException.ThrowIfLessThan(Stride, Width * 4);
        if (Bgra32.Length < Stride * Height)
        {
            throw new ArgumentException(
                "Frame buffer is shorter than its declared dimensions.");
        }

        if (RotationDegrees is not (0 or 90 or 180 or 270))
        {
            throw new ArgumentException("Frame rotation is invalid.");
        }
    }
}

public interface IFrameSource : IAsyncDisposable
{
    event Action<FramePacket>? FrameArrived;

    CameraPosition Position { get; }

    Task StartAsync(CancellationToken cancellationToken = default);

    Task StopAsync(CancellationToken cancellationToken = default);

    Task<FramePacket> CaptureAsync(
        CancellationToken cancellationToken = default);
}

public sealed class ReplayFrameSource : IFrameSource
{
    private readonly IReadOnlyList<FramePacket> _frames;
    private readonly TimeSpan _interval;
    private CancellationTokenSource? _stopSource;
    private Task? _replayTask;
    private int _index;

    public ReplayFrameSource(
        IReadOnlyList<FramePacket> frames,
        TimeSpan? interval = null)
    {
        if (frames.Count == 0)
        {
            throw new ArgumentException("At least one frame is required.");
        }

        foreach (var frame in frames)
        {
            frame.Validate();
        }

        _frames = frames;
        _interval = interval ?? TimeSpan.FromMilliseconds(100);
    }

    public event Action<FramePacket>? FrameArrived;

    public CameraPosition Position => _frames[0].Camera;

    public Task StartAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (_replayTask is not null)
        {
            return Task.CompletedTask;
        }

        _stopSource = new CancellationTokenSource();
        _replayTask = ReplayAsync(_stopSource.Token);
        return Task.CompletedTask;
    }

    public async Task StopAsync(CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        if (_replayTask is null || _stopSource is null)
        {
            return;
        }

        await _stopSource.CancelAsync();
        try
        {
            await _replayTask;
        }
        catch (OperationCanceledException)
        {
            // Cancellation is the normal replay shutdown path.
        }

        _replayTask = null;
        _stopSource.Dispose();
        _stopSource = null;
    }

    public Task<FramePacket> CaptureAsync(
        CancellationToken cancellationToken = default)
    {
        cancellationToken.ThrowIfCancellationRequested();
        return Task.FromResult(_frames[Volatile.Read(ref _index)]);
    }

    public async ValueTask DisposeAsync()
    {
        await StopAsync();
        GC.SuppressFinalize(this);
    }

    private async Task ReplayAsync(CancellationToken cancellationToken)
    {
        using var timer = new PeriodicTimer(_interval);
        while (!cancellationToken.IsCancellationRequested)
        {
            var index = (Volatile.Read(ref _index) + 1) % _frames.Count;
            Interlocked.Exchange(ref _index, index);
            FrameArrived?.Invoke(_frames[index] with
            {
                Timestamp = DateTimeOffset.UtcNow,
            });
            await timer.WaitForNextTickAsync(cancellationToken);
        }
    }
}
