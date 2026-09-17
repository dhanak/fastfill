using FastFill.Core;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Http;

namespace FastFill.Checks;

internal static class FrameLab
{
    private const int MaximumSourceFrames = 300;
    private const int MaximumReplayFrames = 300;

    public static void MapRoutes(WebApplication app)
    {
        var session = new FrameLabSession();
        app.MapGet(
            "/frame-lab",
            () => Results.Text(
                ReadPage(),
                "text/html; charset=utf-8"));
        app.MapPost(
            "/api/frame-lab/load",
            (HttpRequest request, CancellationToken token) =>
                LoadAsync(session, request, token));
        app.MapGet(
            "/api/frame-lab/frame/{index:int}",
            (int index) => FrameResult(session, index));
    }

    private static async Task<IResult> LoadAsync(
        FrameLabSession session,
        HttpRequest request,
        CancellationToken cancellationToken)
    {
        try
        {
            if (!request.HasFormContentType)
            {
                return Error("Choose one or more JPEG or PNG frames.");
            }

            var form = await request.ReadFormAsync(cancellationToken);
            var files = form.Files.GetFiles("files");
            if (files.Count is < 1 or > MaximumSourceFrames)
            {
                return Error(
                    $"Choose between 1 and {MaximumSourceFrames} "
                    + "source frames.");
            }

            if (!int.TryParse(form["intervalMs"], out var intervalMs)
                || intervalMs is < 25 or > 5000)
            {
                return Error("Frame interval must be 25–5000 ms.");
            }

            if (!int.TryParse(form["repetitions"], out var repetitions)
                || repetitions is < 1 or > MaximumReplayFrames
                || files.Count * repetitions > MaximumReplayFrames)
            {
                return Error("Replay output must contain 1–120 frames.");
            }

            var inputs = new List<FrameInput>(files.Count);
            foreach (var file in files)
            {
                var label = Path.GetFileName(file.FileName);
                var extension = Path.GetExtension(label)
                    .ToLowerInvariant();
                if (extension is not (".jpg" or ".jpeg" or ".png"))
                {
                    return Error("Frames must be JPEG or PNG images.");
                }

                var bytes = await SnapLab.ReadImageBytesAsync(
                    file,
                    cancellationToken);
                var details = SnapLab.InspectImage(bytes);
                inputs.Add(new(
                    label,
                    bytes,
                    details.ContentType,
                    details.Width,
                    details.Height));
            }

            var frames = Analyze(inputs, repetitions, intervalMs);
            session.Replace(frames);
            return Results.Json(new
            {
                frames = frames.Select(FrameMetadata),
                intervalMs,
                repetitions,
                countdownMs =
                    AutoCaptureGate.CountdownDuration.TotalMilliseconds,
            });
        }
        catch (OperationCanceledException)
        {
            throw;
        }
        catch (Exception exception)
        {
            return Error(exception.Message);
        }
    }

    private static List<FrameLabFrame> Analyze(
        IReadOnlyList<FrameInput> inputs,
        int repetitions,
        int intervalMs)
    {
        var frames = new List<FrameLabFrame>(
            inputs.Count * repetitions);
        var stabilizer = new DocumentDetectionStabilizer();
        var gate = new AutoCaptureGate();
        var started = DateTimeOffset.UnixEpoch;
        for (var pass = 0; pass < repetitions; pass++)
        {
            foreach (var input in inputs)
            {
                var hint = stabilizer.Hint;
                var raw = DocumentDetector.DetectEncoded(
                    input.Bytes,
                    hint);
                var stabilized = stabilizer.Update(raw);
                var timestamp = started.AddMilliseconds(
                    frames.Count * (long)intervalMs);
                var state = gate.Evaluate(stabilized, timestamp);
                frames.Add(new(
                    input,
                    pass,
                    hint,
                    raw,
                    stabilized,
                    state,
                    gate.Progress));
            }
        }

        return frames;
    }

    private static IResult FrameResult(
        FrameLabSession session,
        int index)
    {
        var frame = session.Get(index);
        return frame is null
            ? Results.NotFound()
            : Results.Bytes(
                frame.Input.Bytes,
                frame.Input.ContentType);
    }

    private static object FrameMetadata(
        FrameLabFrame frame,
        int index)
    {
        var reviewCorners = frame.Raw.Corners
            ?? (frame.Raw.Confidence >= 0.45
                ? frame.Raw.CandidateCorners
                : null);
        return new
        {
            index,
            label = frame.Input.Label,
            pass = frame.Pass + 1,
            width = frame.Input.Width,
            height = frame.Input.Height,
            hint = QuadResult(frame.Hint),
            raw = DetectionResult(frame.Raw),
            stabilized = DetectionResult(frame.Stabilized),
            reviewCorners = QuadResult(reviewCorners),
            autoCapture = new
            {
                state = frame.AutoCaptureState.ToString(),
                progress = frame.AutoCaptureProgress,
            },
        };
    }

    private static object DetectionResult(
        DocumentDetection detection) => new
        {
            detection.Found,
            detection.Confidence,
            detection.AreaRatio,
            detection.Sharpness,
            corners = QuadResult(detection.Corners),
            candidateCorners = QuadResult(detection.CandidateCorners),
            overlayCorners = QuadResult(detection.OverlayCorners),
        };

    private static object? QuadResult(CropQuad? quad) => quad is null
        ? null
        : new
        {
            points = quad.Points.Select(point => new
            {
                x = point.X,
                y = point.Y,
            }),
        };

    private static IResult Error(string message) =>
        Results.BadRequest(new { error = message });

    private static string ReadPage()
    {
        using var stream = typeof(FrameLab).Assembly
            .GetManifestResourceStream("FastFill.Checks.FrameLab.html")
            ?? throw new InvalidOperationException(
                "Frame Lab page is missing.");
        using var reader = new StreamReader(stream);
        return reader.ReadToEnd();
    }

    private sealed class FrameLabSession
    {
        private readonly object _gate = new();
        private IReadOnlyList<FrameLabFrame> _frames = [];

        public void Replace(IReadOnlyList<FrameLabFrame> frames)
        {
            lock (_gate)
            {
                _frames = frames;
            }
        }

        public FrameLabFrame? Get(int index)
        {
            lock (_gate)
            {
                return index >= 0 && index < _frames.Count
                    ? _frames[index]
                    : null;
            }
        }
    }

    private sealed record FrameInput(
        string Label,
        byte[] Bytes,
        string ContentType,
        int Width,
        int Height);

    private sealed record FrameLabFrame(
        FrameInput Input,
        int Pass,
        CropQuad? Hint,
        DocumentDetection Raw,
        DocumentDetection Stabilized,
        AutoCaptureState AutoCaptureState,
        double AutoCaptureProgress);
}
