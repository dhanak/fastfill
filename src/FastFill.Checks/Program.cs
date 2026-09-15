using System.IO.Compression;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Text;
using FastFill.Core;
using OpenCvSharp;
using SkiaSharp;

namespace FastFill.Checks;

internal static class Program
{
    private static readonly string Artifacts = Path.GetFullPath(
        Path.Combine(AppContext.BaseDirectory, "../../../../../artifacts"));
    private static readonly string Goldens = Path.GetFullPath(
        Path.Combine(AppContext.BaseDirectory, "../../../../../tests/goldens"));

    private static async Task<int> Main(string[] args)
    {
        try
        {
            if (args is ["--make-assets", var assetPath])
            {
                MakeAssets(assetPath);
                return 0;
            }

            if (args.FirstOrDefault() == "--detect")
            {
                if (args.Length < 2)
                {
                    throw new ArgumentException(
                        "--detect requires at least one image path.");
                }

                foreach (var path in args.Skip(1))
                {
                    PrintDetection(path);
                }

                return 0;
            }

            var updateGoldens = args.Contains(
                "--update-goldens",
                StringComparer.Ordinal);
            Directory.CreateDirectory(Path.Combine(Artifacts, "preview"));
            await RunAsync(updateGoldens);
            Console.WriteLine("FastFill checks passed.");
            return 0;
        }
        catch (Exception exception)
        {
            Console.Error.WriteLine(exception);
            return 1;
        }
    }

    private static async Task RunAsync(bool updateGoldens)
    {
        var source = CreateSyntheticDocument();
        var detection = DocumentDetector.DetectEncoded(source);
        Assert(detection.Found, "Synthetic document was not detected.");
        Assert(detection.Confidence >= 0.8, "Detection confidence is low.");
        Assert(detection.AreaRatio >= 0.45, "Detected document is too small.");
        Assert(
            detection.Corners!.MaximumCornerDistance(ExpectedCrop()) < 0.04,
            "Detected corners are outside tolerance.");

        var distracted = DocumentDetector.DetectEncoded(
            CreateDistractedDocument());
        Assert(
            distracted.Corners is not null
                && distracted.Corners.MaximumCornerDistance(
                    ExpectedDistractedCrop()) < 0.02,
            "Background lines displaced the detected document corners.");
        Assert(
            !DocumentDetector.DetectEncoded(
                CreateFormWithoutVisibleBoundary()).Found,
            "Printed form lines were mistaken for a document boundary.");

        var processed = ImageProcessor.Process(
            source,
            ExpectedCrop(),
            0,
            new PageFilterSettings
            {
                Mode = PageFilterMode.EnhancedColor,
                Contrast = 10,
            });
        Assert(processed.Width > 600, "Perspective output is too narrow.");
        Assert(processed.Height > 400, "Perspective output is too short.");
        CheckAutoCapture(detection);

        var project = CreateProject();
        var page = project.Pages[0];
        Assert(
            AnnotationHitTester.HitTest(
                page.Annotations,
                new NormalizedPoint(0.3f, 0.3f)) is not null,
            "Annotation hit test failed.");
        Assert(
            AnnotationHitTester.HitTest(
                page.Annotations,
                new NormalizedPoint(0.68f, 0.86f))
                is FreehandAnnotation { Filled: true },
            "Filled freehand hit test failed.");
        Assert(
            AnnotationHitTester.HitTest(
                page.Annotations,
                new NormalizedPoint(0.77f, 0.73f))
                is ShapeAnnotation { Shape: ShapeKind.Cross },
            "X mark hit test failed.");
        CheckUndo(project);
        CheckViewportNavigation();
        CheckAnnotationGeometry();

        var preview = PageRenderer.RenderPng(processed, page);
        var previewPath = Path.Combine(
            Artifacts,
            "preview",
            "annotated-page.png");
        await File.WriteAllBytesAsync(previewPath, preview);
        CheckGolden(previewPath, "annotated-page.png", updateGoldens);
        await CheckStorageAndPdfAsync(project, source);
        await CheckUnsafeArchiveAsync();
        await CheckReplayAsync(source);
    }

    private static void PrintDetection(string path)
    {
        var detection = DocumentDetector.DetectEncoded(
            File.ReadAllBytes(path));
        Console.WriteLine(
            $"{Path.GetFileName(path)}: found={detection.Found}, "
            + $"confidence={detection.Confidence:F3}, "
            + $"area={detection.AreaRatio:F3}, "
            + $"sharpness={detection.Sharpness:F1}");
        if (detection.Corners is not null)
        {
            Console.WriteLine(string.Join(
                ", ",
                detection.Corners.Points.Select(
                    point => $"({point.X:F3}, {point.Y:F3})")));
        }
    }

    private static byte[] CreateSyntheticDocument()
    {
        using var image = new Mat(
            700,
            1000,
            MatType.CV_8UC3,
            new Scalar(25, 35, 45));
        Point[] corners =
        [
            new(145, 80),
            new(870, 105),
            new(820, 625),
            new(110, 590),
        ];
        Cv2.FillConvexPoly(image, corners, new Scalar(245, 245, 240));
        Cv2.Line(
            image,
            new Point(250, 220),
            new Point(730, 235),
            new Scalar(60, 60, 60),
            8);
        Cv2.Line(
            image,
            new Point(220, 310),
            new Point(760, 330),
            new Scalar(90, 90, 90),
            5);
        Cv2.PutText(
            image,
            "FASTFILL",
            new Point(280, 460),
            HersheyFonts.HersheySimplex,
            2,
            new Scalar(30, 30, 30),
            4);
        Assert(
            Cv2.ImEncode(".jpg", image, out var encoded),
            "Synthetic image encoding failed.");
        return encoded;
    }

    private static CropQuad ExpectedCrop() => new(
        new(0.145f, 0.114f),
        new(0.871f, 0.15f),
        new(0.821f, 0.894f),
        new(0.11f, 0.844f));

    private static byte[] CreateDistractedDocument()
    {
        using var image = new Mat(
            720,
            1280,
            MatType.CV_8UC3,
            new Scalar(35, 45, 55));
        Point[] corners =
        [
            new(303, 59),
            new(923, 59),
            new(923, 678),
            new(303, 669),
        ];
        Cv2.FillConvexPoly(image, corners, new Scalar(240, 240, 235));
        Cv2.Polylines(
            image,
            [corners],
            true,
            new Scalar(210, 210, 210),
            5);
        Cv2.PutText(
            image,
            "DOCUMENT",
            new Point(430, 340),
            HersheyFonts.HersheySimplex,
            2,
            new Scalar(40, 40, 40),
            5);

        // These lines can form a larger false quad with three document edges.
        Cv2.Line(
            image,
            corners[1],
            new Point(1041, 59),
            new Scalar(225, 225, 225),
            6);
        Cv2.Line(
            image,
            new Point(1041, 59),
            new Point(1220, 681),
            new Scalar(225, 225, 225),
            6);
        Cv2.Line(
            image,
            corners[2],
            new Point(1220, 681),
            new Scalar(225, 225, 225),
            6);

        Assert(
            Cv2.ImEncode(".jpg", image, out var encoded),
            "Distracted image encoding failed.");
        return encoded;
    }

    private static CropQuad ExpectedDistractedCrop() => new(
        new(303f / 1279, 59f / 719),
        new(923f / 1279, 59f / 719),
        new(923f / 1279, 678f / 719),
        new(303f / 1279, 669f / 719));

    private static byte[] CreateFormWithoutVisibleBoundary()
    {
        using var image = new Mat(
            720,
            1280,
            MatType.CV_8UC3,
            new Scalar(235, 235, 230));
        var ink = new Scalar(55, 55, 55);
        Cv2.Rectangle(image, new Rect(45, 210, 1190, 330), ink, 4);
        Cv2.Line(image, new Point(640, 210), new Point(640, 540), ink, 4);
        foreach (var y in new[] { 275, 340, 405, 470 })
        {
            Cv2.Line(image, new Point(45, y), new Point(1235, y), ink, 3);
        }

        Cv2.PutText(
            image,
            "FORM CONTENT",
            new Point(410, 120),
            HersheyFonts.HersheySimplex,
            1.5,
            ink,
            4);
        Assert(
            Cv2.ImEncode(".jpg", image, out var encoded),
            "Boundary-free form encoding failed.");
        return encoded;
    }

    private static FastFillProject CreateProject()
    {
        var page = new DocumentPage
        {
            SourceAsset = "page-1.jpg",
            SourcePixelWidth = 1000,
            SourcePixelHeight = 700,
            Crop = ExpectedCrop(),
            Filter = new PageFilterSettings
            {
                Mode = PageFilterMode.EnhancedColor,
                Contrast = 10,
            },
            Annotations =
            [
                new ShapeAnnotation
                {
                    Shape = ShapeKind.Rectangle,
                    Start = new(0.12f, 0.16f),
                    End = new(0.48f, 0.42f),
                    ColorArgb = 0xff0067c0,
                    StrokeWidth = 4,
                },
                new ShapeAnnotation
                {
                    Shape = ShapeKind.Arrow,
                    Start = new(0.18f, 0.72f),
                    End = new(0.72f, 0.52f),
                    ColorArgb = 0xffd13438,
                    StrokeWidth = 5,
                },
                new ShapeAnnotation
                {
                    Shape = ShapeKind.Checkmark,
                    Start = new(0.62f, 0.15f),
                    End = new(0.82f, 0.34f),
                    ColorArgb = 0xff008272,
                    StrokeWidth = 7,
                },
                new ShapeAnnotation
                {
                    Shape = ShapeKind.Cross,
                    Start = new(0.72f, 0.68f),
                    End = new(0.82f, 0.78f),
                    ColorArgb = 0xffd13438,
                    StrokeWidth = 6,
                },
                new FreehandAnnotation
                {
                    Filled = true,
                    ColorArgb = 0xff16a34a,
                    StrokeWidth = 3,
                    Points =
                    [
                        new(0.65f, 0.8f),
                        new(0.75f, 0.88f),
                        new(0.6f, 0.9f),
                    ],
                },
                new FreehandAnnotation
                {
                    IsHighlighter = true,
                    ColorArgb = 0xffffd800,
                    StrokeWidth = 16,
                    Points =
                    [
                        new(0.15f, 0.83f),
                        new(0.35f, 0.81f),
                        new(0.58f, 0.84f),
                    ],
                },
                new TextAnnotation
                {
                    Text = "Ready to send",
                    Bounds = new(0.22f, 0.25f, 0.48f, 0.12f),
                    ColorArgb = 0xff111827,
                    FontSize = 18,
                },
            ],
        };
        return new FastFillProject
        {
            Title = "FastFill check",
            Pages = [page],
        };
    }

    private static void CheckAnnotationGeometry()
    {
        var rectangle = new ShapeAnnotation
        {
            Shape = ShapeKind.Rectangle,
            Start = new(0.1f, 0.1f),
            End = new(0.3f, 0.2f),
            StrokeWidth = 4,
        };
        var resized = (ShapeAnnotation)AnnotationGeometry.ResizeFromCorner(
            rectangle,
            2,
            new NormalizedPoint(0.6f, 0.3f),
            600,
            800);
        var resizedBounds = AnnotationGeometry.Bounds(resized);
        Assert(
            Math.Abs(resizedBounds.Width - 0.5f) < 0.001
                && Math.Abs(resizedBounds.Height - 0.2f) < 0.001,
            "Annotation resize still preserves its old aspect ratio.");
        Assert(
            Math.Abs(resized.StrokeWidth - rectangle.StrokeWidth) < 0.001,
            "Annotation resize changed line thickness.");

        var line = new ShapeAnnotation
        {
            Shape = ShapeKind.Arrow,
            Start = new(0.2f, 0.3f),
            End = new(0.6f, 0.5f),
        };
        var lineHandles = AnnotationGeometry.ResizeHandles(line);
        Assert(
            lineHandles.SequenceEqual([line.Start, line.End]),
            "Line resize handles are not its endpoints.");
        var resizedLine = (ShapeAnnotation)
            AnnotationGeometry.ResizeFromCorner(
                line,
                0,
                new NormalizedPoint(0.1f, 0.2f),
                600,
                800);
        Assert(
            resizedLine.Start == new NormalizedPoint(0.1f, 0.2f)
                && resizedLine.End == line.End,
            "Line endpoint resize moved the wrong point.");

        var text = new TextAnnotation
        {
            Text = "Resize this complete text around several words",
            Bounds = new(0.2f, 0.2f, 0.2f, 0.05f),
            FontSize = 18,
            MaximumWidth = 0.15f,
        };
        var resizedText = (TextAnnotation)AnnotationGeometry.ResizeByFactor(
            text,
            1.5f);
        Assert(
            resizedText.Bounds == text.Bounds,
            "Text box should not expose manual resize.");
        var fittedText = PageRenderer.FitTextBounds(text, 600, 800);
        Assert(
            fittedText.Bounds.Width <= text.MaximumWidth + 0.001f,
            $"Text width exceeded its limit: {fittedText.Bounds.Width}.");
        Assert(
            fittedText.Bounds.Height > text.Bounds.Height,
            $"Text bounds did not grow: {fittedText.Bounds.Height}.");

        var center = new Vector2(0.5f, 0.5f);
        var rotation = AnnotationGeometry.PageRotation(
            MathF.PI / 2,
            center,
            600,
            800);
        var rotated = Vector2.Transform(new Vector2(0.6f, 0.5f), rotation);
        Assert(
            Vector2.Distance(rotated, new Vector2(0.5f, 0.575f)) < 0.001,
            "Page rotation did not preserve physical aspect ratio.");
    }

    private static void CheckAutoCapture(DocumentDetection detection)
    {
        var gate = new AutoCaptureGate();
        var start = DateTimeOffset.UtcNow;
        var stable = detection with
        {
            Confidence = 0.8,
            AreaRatio = 0.2,
            Sharpness = 50,
        };
        Assert(
            gate.Evaluate(stable, start) == AutoCaptureState.HoldSteady,
            "Auto-capture did not begin stability wait.");
        Assert(
            gate.Evaluate(stable, start.AddMilliseconds(1500))
                == AutoCaptureState.HoldSteady,
            "Auto-capture countdown ended early.");
        Assert(
            Math.Abs(gate.Progress - 0.5) < 0.01,
            "Auto-capture countdown progress is incorrect.");
        Assert(
            gate.Evaluate(stable, start.AddMilliseconds(3100))
                == AutoCaptureState.Ready,
            "Auto-capture did not become ready.");
        Assert(
            gate.Evaluate(stable, start.AddSeconds(2))
                == AutoCaptureState.Captured,
            "Auto-capture cooldown failed.");
        Assert(
            gate.Evaluate(
                stable with { Sharpness = 10 },
                start.AddSeconds(3)) == AutoCaptureState.NoDocument,
            "Blur rejection failed.");
        Assert(gate.Progress == 0, "Auto-capture countdown did not reset.");
    }

    private static void CheckUndo(FastFillProject project)
    {
        var history = new UndoBuffer<FastFillProject>(ProjectJson.Clone);
        history.Checkpoint(project);
        var changed = project with { Title = "Changed" };
        Assert(
            history.TryUndo(changed, out var undone)
            && undone.Title == project.Title,
            "Undo failed.");
        Assert(
            history.TryRedo(undone, out var redone)
            && redone.Title == "Changed",
            "Redo failed.");
    }

    private static void CheckViewportNavigation()
    {
        var pan = ViewportNavigation.PinchPan(
            new Vector2(1000, 800),
            new Vector2(500, 400),
            new Vector2(550, 430),
            Vector2.Zero,
            1,
            2);
        Assert(
            Vector2.Distance(pan, new Vector2(50, 30)) < 0.01f,
            "Pinch focal-point pan changed.");
        var clamped = ViewportNavigation.ClampPan(
            new Vector2(200, -500),
            new Vector2(1000, 800),
            new Vector2(600, 800),
            2);
        Assert(
            Vector2.Distance(clamped, new Vector2(100, -400)) < 0.01f,
            "Viewport pan bounds changed.");
    }

    private static async Task CheckStorageAndPdfAsync(
        FastFillProject project,
        byte[] source)
    {
        var root = Path.Combine(Artifacts, "check-workspace");
        ResetDirectory(root);
        var workspace = WorkspaceStore.Create(root, project.Id);
        await File.WriteAllBytesAsync(
            Path.Combine(workspace, "assets", "page-1.jpg"),
            source);
        await WorkspaceStore.SaveManifestAsync(project, workspace);

        var pdfPath = Path.Combine(Artifacts, "preview", "fastfill-check.pdf");
        await using (var pdf = File.Create(pdfPath))
        {
            await PdfExporter.ExportAsync(
                project,
                (page, token) => WorkspaceStore.ProcessPageAsync(
                    workspace,
                    page,
                    token),
                pdf);
        }

        var pdfBytes = await File.ReadAllBytesAsync(pdfPath);
        Assert(
            Encoding.ASCII.GetString(pdfBytes, 0, 4) == "%PDF",
            "PDF signature is invalid.");
        Assert(pdfBytes.Length > 10_000, "PDF output is unexpectedly small.");

        var archivePath = Path.Combine(Artifacts, "preview", "check.docscan");
        await ProjectArchive.SaveAsync(project, workspace, archivePath);
        var loadedPath = Path.Combine(root, "loaded");
        var loaded = await ProjectArchive.LoadAsync(archivePath, loadedPath);
        Assert(loaded.Project.Title == project.Title, "Project title changed.");
        Assert(loaded.Project.Pages.Count == 1, "Project page was lost.");
    }

    private static async Task CheckUnsafeArchiveAsync()
    {
        var archivePath = Path.Combine(Artifacts, "unsafe.docscan");
        await using (var file = File.Create(archivePath))
        {
            using var archive = new ZipArchive(
                file,
                ZipArchiveMode.Create,
                leaveOpen: false);
            var entry = archive.CreateEntry("../outside.txt");
            await using var writer = new StreamWriter(entry.Open());
            await writer.WriteAsync("unsafe");
        }

        var rejected = false;
        try
        {
            await ProjectArchive.LoadAsync(
                archivePath,
                Path.Combine(Artifacts, "unsafe-workspace"));
        }
        catch (InvalidDataException)
        {
            rejected = true;
        }

        Assert(rejected, "Unsafe ZIP path was accepted.");
    }

    private static async Task CheckReplayAsync(byte[] source)
    {
        using var decoded = Cv2.ImDecode(source, ImreadModes.Color);
        using var bgra = new Mat();
        Cv2.CvtColor(decoded, bgra, ColorConversionCodes.BGR2BGRA);
        bgra.GetArray(out Vec4b[] pixels);
        var bytes = MemoryMarshal.AsBytes(pixels.AsSpan()).ToArray();
        var frame = new FramePacket(
            DateTimeOffset.UtcNow,
            bytes,
            bgra.Width,
            bgra.Height,
            bgra.Width * 4,
            0,
            false,
            CameraPosition.Back);
        await using var replay = new ReplayFrameSource(
            [frame],
            TimeSpan.FromMilliseconds(10));
        var arrived = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        replay.FrameArrived += _ => arrived.TrySetResult();
        await replay.StartAsync();
        await arrived.Task.WaitAsync(TimeSpan.FromSeconds(1));
        var captured = await replay.CaptureAsync();
        Assert(captured.Width == 1000, "Replay capture dimensions changed.");
        await replay.StopAsync();
    }

    private static void CheckGolden(
        string actualPath,
        string name,
        bool update)
    {
        if (!OperatingSystem.IsLinux())
        {
            return;
        }

        var expectedPath = Path.Combine(Goldens, name);
        if (update)
        {
            Directory.CreateDirectory(Goldens);
            File.Copy(actualPath, expectedPath, overwrite: true);
            return;
        }

        Assert(File.Exists(expectedPath), $"Golden is missing: {name}");
        using var actual = SKBitmap.Decode(actualPath);
        using var expected = SKBitmap.Decode(expectedPath);
        Assert(
            actual.Width == expected.Width && actual.Height == expected.Height,
            $"Golden dimensions differ: {name}");
        long difference = 0;
        for (var y = 0; y < actual.Height; y += 4)
        {
            for (var x = 0; x < actual.Width; x += 4)
            {
                var first = actual.GetPixel(x, y);
                var second = expected.GetPixel(x, y);
                difference += Math.Abs(first.Red - second.Red);
                difference += Math.Abs(first.Green - second.Green);
                difference += Math.Abs(first.Blue - second.Blue);
            }
        }

        Assert(difference == 0, $"Golden image differs: {name}");
    }

    private static void ResetDirectory(string path)
    {
        if (Directory.Exists(path))
        {
            Directory.Delete(path, recursive: true);
        }

        Directory.CreateDirectory(path);
    }

    private static void MakeAssets(string assetPath)
    {
        var sourcePath = Path.Combine(assetPath, "AppIcon.png");
        using var source = SKImage.FromEncodedData(sourcePath)
            ?? throw new InvalidDataException("App icon is invalid.");
        WriteIcon(source, assetPath, "Square44x44Logo.png", 44);
        WriteIcon(source, assetPath, "Square150x150Logo.png", 150);
        WriteIcon(source, assetPath, "StoreLogo.png", 50);
    }

    private static void WriteIcon(
        SKImage source,
        string assetPath,
        string name,
        int size)
    {
        var info = new SKImageInfo(size, size, SKColorType.Bgra8888);
        using var surface = SKSurface.Create(info)
            ?? throw new InvalidOperationException("Icon surface failed.");
        surface.Canvas.DrawImage(
            source,
            new SKRect(0, 0, size, size),
            new SKSamplingOptions(
                SKFilterMode.Linear,
                SKMipmapMode.Linear));
        using var image = surface.Snapshot();
        using var data = image.Encode(SKEncodedImageFormat.Png, 100);
        using var output = File.Create(Path.Combine(assetPath, name));
        data.SaveTo(output);
    }

    private static void Assert(bool condition, string message)
    {
        if (!condition)
        {
            throw new InvalidOperationException(message);
        }
    }
}
