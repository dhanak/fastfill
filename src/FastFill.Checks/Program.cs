using System.Formats.Tar;
using System.IO.Compression;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
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
    private static readonly string Samples = Path.GetFullPath(
        Path.Combine(AppContext.BaseDirectory, "../../../../../tests/samples"));

    private static async Task<int> Main(string[] args)
    {
        try
        {
            if (args.FirstOrDefault() == "--lab")
            {
                var url = args.ElementAtOrDefault(1)
                    ?? "http://0.0.0.0:5077";
                await SnapLab.RunAsync(url);
                return 0;
            }

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
        CheckDetectionStabilizer(detection);
        CheckRecordedDetectionSequence();
        CheckImageSnapFeatures();
        CheckSampleFormSnapFeatures();

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
        CheckProjectSnapshot(project);

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

    private static void CheckProjectSnapshot(FastFillProject project)
    {
        var mutable = ProjectJson.Clone(project);
        var snapshot = ProjectJson.Clone(mutable);
        var annotationCount = snapshot.Pages[0].Annotations.Count;
        mutable.Pages[0].Annotations.Add(new TextAnnotation
        {
            Text = "Newer edit",
        });
        Assert(
            snapshot.Pages[0].Annotations.Count == annotationCount,
            "Project snapshot changed with live annotations.");
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

        // A preview status banner must not consume every Hough line slot.
        Cv2.Rectangle(
            image,
            new Rect(55, 570, 1050, 36),
            new Scalar(65, 70, 75),
            -1);
        Cv2.PutText(
            image,
            "Camera preview status",
            new Point(230, 597),
            HersheyFonts.HersheySimplex,
            0.8,
            new Scalar(230, 230, 230),
            2);

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

    private static void CheckImageSnapFeatures()
    {
        using var image = new Mat(
            600,
            800,
            MatType.CV_8UC3,
            new Scalar(245, 245, 245));
        Cv2.Rectangle(
            image,
            new Rect(120, 100, 44, 44),
            new Scalar(20, 20, 20),
            4);
        Cv2.Line(
            image,
            new Point(240, 360),
            new Point(640, 360),
            new Scalar(20, 20, 20),
            3);
        for (var x = 100; x < 380; x += 16)
        {
            Cv2.Line(
                image,
                new Point(x, 450),
                new Point(Math.Min(x + 11, 380), 450),
                new Scalar(20, 20, 20),
                2);
        }

        Cv2.PutText(
            image,
            "NAME",
            new Point(400, 456),
            HersheyFonts.HersheySimplex,
            0.55,
            new Scalar(20, 20, 20),
            2);
        Cv2.Line(
            image,
            new Point(520, 450),
            new Point(700, 450),
            new Scalar(20, 20, 20),
            2);
        Cv2.PutText(
            image,
            "Datum:",
            new Point(60, 520),
            HersheyFonts.HersheySimplex,
            0.6,
            new Scalar(20, 20, 20),
            2);
        for (var x = 130; x < 430; x += 6)
        {
            Cv2.Circle(
                image,
                new Point(x, 520),
                1,
                new Scalar(20, 20, 20),
                -1);
        }
        for (var x = 560; x < 602; x += 6)
        {
            Cv2.Circle(
                image,
                new Point(x, 560),
                1,
                new Scalar(20, 20, 20),
                -1);
        }
        Assert(
            Cv2.ImEncode(".png", image, out var encoded),
            "Snap feature image encoding failed.");

        var features = ImageSnapFeatures.Analyze(encoded);
        var box = features.FindBox(
            new NormalizedPoint(142f / 799, 122f / 599),
            0.04f);
        Assert(
            box is NormalizedRect detected
                && detected.Width * 800 is > 35 and < 55,
            "Checkbox snap feature was not detected.");
        var corner = features.FindCorner(
            new NormalizedPoint(122f / 799, 102f / 599),
            0.04f);
        Assert(corner is not null, "Nearby image corner was not detected.");
        var line = features.FindHorizontalLine(
            new NormalizedPoint(440f / 799, 355f / 599),
            0.04f);
        Assert(
            line is not null
                && line.Start.X < 0.4f
                && line.End.X > 0.7f,
            "Text line snap feature was not detected.");
        var dashedLine = features.FindHorizontalLine(
            new NormalizedPoint(250f / 799, 450f / 599),
            0.04f);
        Assert(
            dashedLine is not null
                && dashedLine.Start.X < 0.16f
                && dashedLine.End.X is > 0.44f and < 0.51f,
            "Dashed text line snap feature was not detected separately.");
        var separateLine = features.FindHorizontalLine(
            new NormalizedPoint(620f / 799, 450f / 599),
            0.04f);
        Assert(
            separateLine is not null
                && separateLine.Start.X > 0.62f
                && separateLine.End.X > 0.85f,
            "Separate same-row text line snap feature was merged.");
        var ordinaryText = features.FindHorizontalLine(
            new NormalizedPoint(430f / 799, 456f / 599),
            0.01f);
        Assert(
            ordinaryText is null,
            "Ordinary text was detected as a snap line.");
        var dottedLine = features.FindHorizontalLine(
            new NormalizedPoint(280f / 799, 520f / 599),
            0.04f);
        Assert(
            dottedLine is not null
                && dottedLine.Start.X is > 0.16f and < 0.2f
                && dottedLine.End.X is > 0.5f and < 0.56f,
            $"Dotted text line snap feature included its label: "
                + $"{dottedLine}.");
        var dottedLineFromLabel = features.FindHorizontalLine(
            new NormalizedPoint(108f / 799, 520f / 599),
            0.04f);
        Assert(
            dottedLineFromLabel is not null
                && dottedLineFromLabel.Start.X > 0.16f,
            "Dotted text line snap selected the adjacent label.");
        var shortDottedLine = features.FindHorizontalLine(
            new NormalizedPoint(580f / 799, 560f / 599),
            0.02f);
        Assert(
            shortDottedLine is not null
                && shortDottedLine.Start.X is > 0.69f and < 0.72f
                && shortDottedLine.End.X is > 0.73f and < 0.77f,
            "Short dotted text line snap feature was not detected.");
    }

    private static void CheckSampleFormSnapFeatures()
    {
        var bytes = File.ReadAllBytes(
            Path.Combine(Samples, "sample-form.png"));
        var features = ImageSnapFeatures.Analyze(bytes);
        var box = features.FindBox(
            new NormalizedPoint(113f / 1240, 654f / 1753),
            0.015f);
        Assert(
            box is NormalizedRect detected
                && detected.Width * 1241 is > 20 and < 40
                && detected.Height * 1754 is > 20 and < 40,
            "Checkbox inside a bordered form was not detected.");

        var dottedLine = features.FindHorizontalLine(
            new NormalizedPoint(980f / 1240, 718f / 1753),
            0.02f);
        Assert(
            dottedLine is not null
                && dottedLine.Start.X is > 0.64f and < 0.7f
                && dottedLine.End.X > 0.88f,
            $"Dotted line inside a bordered form was not detected: "
                + $"{dottedLine}.");
    }

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
                    IsBold = true,
                    IsItalic = true,
                    IsUnderlined = true,
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

        var outside = rectangle with
        {
            Transform = AffineTransform.FromMatrix(
                Matrix3x2.CreateTranslation(2, -2)),
        };
        var broughtInside = AnnotationGeometry.BringIntoPage(outside);
        var broughtBounds = AnnotationGeometry.TransformedBounds(
            broughtInside);
        Assert(
            Math.Abs(broughtBounds.X - 0.95f) < 0.001f
                && Math.Abs(broughtBounds.Bottom - 0.05f) < 0.001f,
            "Off-page annotation was not moved to an accessible edge.");
        var partial = rectangle with
        {
            Transform = AffineTransform.FromMatrix(
                Matrix3x2.CreateTranslation(-0.2f, 0)),
        };
        Assert(
            AnnotationGeometry.BringIntoPage(partial).Transform
                == partial.Transform,
            "Partly visible annotation was moved unnecessarily.");

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
        Assert(
            !AnnotationGeometry.SupportsRotation(line)
                && AnnotationGeometry.SupportsRotation(rectangle),
            "Rotation support does not match annotation geometry.");
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
        var rotationTransform = AffineTransform.FromMatrix(
            AnnotationGeometry.PageRotation(
                MathF.PI / 3,
                center,
                600,
                800));
        Assert(
            Math.Abs(AnnotationGeometry.PageRotationRadians(
                rotationTransform,
                600,
                800) - MathF.PI / 3) < 0.001,
            "Page rotation angle could not be recovered.");

        List<Annotation> annotations = [rectangle, line, text];
        var selectedIds = new HashSet<Guid> { line.Id };
        Assert(
            AnnotationOrdering.SendToBack(annotations, selectedIds)
                && annotations[0].Id == line.Id,
            "Annotation was not sent to the back.");
        Assert(
            AnnotationOrdering.BringToFront(annotations, selectedIds)
                && annotations[^1].Id == line.Id,
            "Annotation was not brought to the front.");
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
            gate.Evaluate(stable, start.AddMilliseconds(1000))
                == AutoCaptureState.HoldSteady,
            "Auto-capture countdown ended early.");
        Assert(
            Math.Abs(gate.Progress - 0.5) < 0.01,
            "Auto-capture countdown progress is incorrect.");
        Assert(
            gate.Evaluate(stable, start.AddMilliseconds(2100))
                == AutoCaptureState.Ready,
            "Auto-capture did not become ready.");
        Assert(
            gate.Evaluate(stable, start.AddMilliseconds(2200))
                == AutoCaptureState.Captured,
            "Auto-capture cooldown failed.");
        Assert(
            gate.Evaluate(
                stable with { Sharpness = 10 },
                start.AddSeconds(3)) == AutoCaptureState.NoDocument,
            "Blur rejection failed.");
        Assert(gate.Progress == 0, "Auto-capture countdown did not reset.");

        var driftGate = new AutoCaptureGate();
        driftGate.Evaluate(stable, start);
        driftGate.Evaluate(
            stable with { Corners = Shift(stable.Corners!, 0.025f, 0) },
            start.AddSeconds(1));
        Assert(driftGate.Progress > 0, "Small camera jitter reset countdown.");
        driftGate.Evaluate(
            stable with { Corners = Shift(stable.Corners!, 0.05f, 0) },
            start.AddSeconds(2));
        Assert(
            driftGate.Progress == 0,
            "Slow document drift did not reset countdown.");
    }

    private static void CheckRecordedDetectionSequence()
    {
        var path = Path.Combine(Samples, "folded-paper-frames.tar");
        Assert(File.Exists(path), "Recorded detection sequence is missing.");
        using var stream = File.OpenRead(path);
        using var reader = new TarReader(stream);
        var stabilizer = new DocumentDetectionStabilizer();
        var gate = new AutoCaptureGate();
        var expected = new CropQuad(
            new(0.28f, 0.09f),
            new(0.86f, 0.15f),
            new(0.86f, 0.89f),
            new(0.26f, 0.89f));
        var timestamp = DateTimeOffset.UnixEpoch;
        var frames = 0;
        var correctRaw = 0;
        var wrongRaw = 0;
        var correctStable = 0;
        var wrongStable = 0;
        var captured = false;
        TarEntry? entry;
        while ((entry = reader.GetNextEntry()) is not null)
        {
            if (entry.EntryType != TarEntryType.RegularFile
                || entry.DataStream is null
                || !entry.Name.EndsWith(
                    ".jpg",
                    StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }

            using var encoded = new MemoryStream();
            entry.DataStream.CopyTo(encoded);
            var raw = DocumentDetector.DetectEncoded(
                encoded.ToArray(),
                stabilizer.Hint);
            var stable = stabilizer.Update(raw);
            CountDetection(raw, ref correctRaw, ref wrongRaw);
            CountDetection(stable, ref correctStable, ref wrongStable);
            var state = gate.Evaluate(stable, timestamp);
            captured |= state is AutoCaptureState.Ready
                or AutoCaptureState.Captured;
            frames++;
            timestamp = timestamp.AddMilliseconds(33);
        }

        Assert(frames == 224, "Recorded sequence frame count changed.");
        Assert(correctRaw >= 160, "Recorded raw detection regressed.");
        Assert(wrongRaw == 0, "Recorded raw detection selected a wrong quad.");
        Assert(
            correctStable >= 180,
            "Recorded stabilized detection regressed.");
        Assert(
            wrongStable == 0,
            "Recorded stabilization selected a wrong quad.");
        Assert(captured, "Recorded sequence never reached auto-capture.");

        void CountDetection(
            DocumentDetection detection,
            ref int correct,
            ref int wrong)
        {
            if (!detection.Found)
            {
                return;
            }

            if (detection.Corners!.MaximumCornerDistance(expected) < 0.1f)
            {
                correct++;
            }
            else
            {
                wrong++;
            }
        }
    }

    private static void CheckDetectionStabilizer(
        DocumentDetection detection)
    {
        var stabilizer = new DocumentDetectionStabilizer();
        var baseline = detection with
        {
            Confidence = 0.85,
            AreaRatio = 0.3,
            Sharpness = 60,
        };
        var nearby = baseline with
        {
            Corners = Shift(baseline.Corners!, 0.01f, 0),
        };
        Assert(
            !stabilizer.Update(baseline).Found,
            "Detection stabilizer did not delay its first result.");
        var stable = stabilizer.Update(nearby);
        Assert(stable.Found, "Detection consensus was not accepted.");

        var distant = baseline with
        {
            Corners = Shift(baseline.Corners!, -0.12f, 0),
        };
        stabilizer.Update(distant);
        var held = stabilizer.Update(distant);
        Assert(
            held.Corners!.MaximumCornerDistance(stable.Corners!) < 0.03f,
            "Minority detections displaced the stable framing.");
        var switched = stabilizer.Update(distant);
        Assert(
            switched.Corners!.MaximumCornerDistance(distant.Corners!)
                < 0.01f,
            "Detection framing ignored a new majority.");
    }

    private static CropQuad Shift(CropQuad quad, float x, float y) => new(
        Shift(quad.TopLeft, x, y),
        Shift(quad.TopRight, x, y),
        Shift(quad.BottomRight, x, y),
        Shift(quad.BottomLeft, x, y));

    private static NormalizedPoint Shift(
        NormalizedPoint point,
        float x,
        float y) => new(point.X + x, point.Y + y);

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
        Assert(
            loaded.Project.Pages[0].Annotations.OfType<TextAnnotation>()
                .Single() is
            {
                IsBold: true,
                IsItalic: true,
                IsUnderlined: true,
            },
            "Text styles were not preserved.");
        await CheckArchiveSaveLimitsAsync(project, workspace);
        await CheckArchiveImageValidationAsync(project, source, root);
        await CheckProjectValidationAsync(project, source, root);
    }

    private static async Task CheckArchiveSaveLimitsAsync(
        FastFillProject project,
        string workspace)
    {
        var oversized = ProjectJson.Clone(project);
        var text = new string('x', ProjectValidator.MaximumTextLength);
        oversized.Pages[0] = oversized.Pages[0] with
        {
            Annotations = Enumerable.Range(0, 600)
                .Select(_ => (Annotation)new TextAnnotation
                {
                    Text = text,
                    Bounds = new(0.1f, 0.1f, 0.4f, 0.1f),
                })
                .ToList(),
        };
        var destination = Path.Combine(
            Artifacts,
            "preview",
            "oversized-save.docscan");
        var original = Encoding.UTF8.GetBytes("existing project");
        await File.WriteAllBytesAsync(destination, original);
        var rejected = false;
        try
        {
            await ProjectArchive.SaveAsync(
                oversized,
                workspace,
                destination);
        }
        catch (InvalidDataException)
        {
            rejected = true;
        }

        Assert(rejected, "Oversized manifest was saved.");
        Assert(
            (await File.ReadAllBytesAsync(destination))
                .SequenceEqual(original),
            "Rejected save replaced the existing archive.");
        var pattern = $".{Path.GetFileName(destination)}.*.tmp";
        Assert(
            !Directory.EnumerateFiles(
                Path.GetDirectoryName(destination)!,
                pattern)
                .Any(),
            "Rejected save left a temporary archive.");
    }

    private static async Task CheckProjectValidationAsync(
        FastFillProject project,
        byte[] source,
        string root)
    {
        var invalidFilter = ProjectJson.Clone(project);
        invalidFilter.Pages[0] = invalidFilter.Pages[0] with
        {
            Filter = invalidFilter.Pages[0].Filter with
            {
                Mode = (PageFilterMode)999,
            },
        };
        await AssertArchiveRejectedAsync(
            invalidFilter,
            source,
            root,
            "invalid-filter",
            "Undefined page filter mode was accepted.");

        var invalidShape = ProjectJson.Clone(project);
        var shapeIndex = invalidShape.Pages[0].Annotations.FindIndex(
            annotation => annotation is ShapeAnnotation);
        var shape = (ShapeAnnotation)invalidShape.Pages[0]
            .Annotations[shapeIndex];
        invalidShape.Pages[0].Annotations[shapeIndex] = shape with
        {
            Shape = (ShapeKind)999,
        };
        await AssertArchiveRejectedAsync(
            invalidShape,
            source,
            root,
            "invalid-shape",
            "Undefined shape kind was accepted.");

        var invalidCrop = ProjectJson.Clone(project);
        invalidCrop.Pages[0] = invalidCrop.Pages[0] with
        {
            Crop = invalidCrop.Pages[0].Crop with
            {
                TopLeft = new(-1, 0),
            },
        };
        await AssertArchiveRejectedAsync(
            invalidCrop,
            source,
            root,
            "invalid-crop",
            "Out-of-page crop geometry was accepted.");

        var invalidTransform = ProjectJson.Clone(project);
        invalidTransform.Pages[0].Annotations[0] =
            invalidTransform.Pages[0].Annotations[0] with
            {
                Transform = new AffineTransform(
                    1,
                    0,
                    0,
                    1,
                    100,
                    0),
            };
        await AssertArchiveRejectedAsync(
            invalidTransform,
            source,
            root,
            "invalid-transform",
            "Fully off-page annotation was accepted.");

        var renderOverflow = ProjectJson.Clone(project);
        var compensatedText = new TextAnnotation
        {
            Text = "Visible after translation",
            Bounds = new(1e37f, 0.2f, 0.1f, 0.1f),
            Transform = new AffineTransform(
                1,
                0,
                0,
                1,
                -1e37f,
                0),
        };
        ProjectValidator.ValidateAnnotation(compensatedText);
        renderOverflow.Pages[0].Annotations[0] = compensatedText;
        AssertProjectRejected(
            renderOverflow,
            "Renderer-overflowing annotation was accepted.");

        var tooManyAnnotations = ProjectJson.Clone(project);
        tooManyAnnotations.Pages[0] = tooManyAnnotations.Pages[0] with
        {
            Annotations = Enumerable.Range(
                0,
                ProjectValidator.MaximumAnnotationsPerPage + 1)
                .Select(_ => (Annotation)new ShapeAnnotation
                {
                    Shape = ShapeKind.Line,
                    Start = new(0.4f, 0.5f),
                    End = new(0.6f, 0.5f),
                })
                .ToList(),
        };
        AssertProjectRejected(
            tooManyAnnotations,
            "Excess page annotations were accepted.");

        var tooManyInkPoints = ProjectJson.Clone(project);
        tooManyInkPoints.Pages[0].Annotations[0] =
            new FreehandAnnotation
            {
                Points = Enumerable.Repeat(
                    new NormalizedPoint(0.5f, 0.5f),
                    ProjectValidator.MaximumInkPoints + 1)
                    .ToList(),
            };
        AssertProjectRejected(
            tooManyInkPoints,
            "Excess ink points were accepted.");

        await CheckTallPageRotationAsync(root);

        var nonFinite = ProjectJson.Clone(project);
        nonFinite.Pages[0].Annotations[0] =
            nonFinite.Pages[0].Annotations[0] with
            {
                StrokeWidth = float.NaN,
            };
        AssertProjectRejected(
            nonFinite,
            "Non-finite annotation geometry was accepted.");
        CheckAnnotationValidation();
    }

    private static void CheckAnnotationValidation()
    {
        var page = new DocumentPage
        {
            SourcePixelWidth = 1_000,
            SourcePixelHeight = 1_400,
        };
        var multiline = PageRenderer.FitTextBounds(
            new TextAnnotation
            {
                Text = string.Join(
                    "\n",
                    Enumerable.Repeat("x", 1_000)),
                Bounds = new(0.1f, 0.1f, 0.4f, 0.1f),
                FontSize = 14,
                MaximumWidth = 0.4f,
            },
            612,
            792);
        Assert(
            multiline.Bounds.Height > 16,
            "Multiline regression text no longer exceeds project bounds.");
        ProjectValidator.ValidateAnnotation(multiline, page);

        var partial = new ShapeAnnotation
        {
            Shape = ShapeKind.Rectangle,
            Start = new(-2, 0.25f),
            End = new(0.5f, 0.75f),
        };
        ProjectValidator.ValidateAnnotation(partial, page);

        Annotation grown = new ShapeAnnotation
        {
            Shape = ShapeKind.Rectangle,
            Start = new(0.4f, 0.4f),
            End = new(0.6f, 0.6f),
        };
        for (var index = 0; index < 50; index++)
        {
            grown = AnnotationGeometry.ResizeByFactor(grown, 1.1f);
        }

        Assert(
            AnnotationGeometry.Bounds(grown).Width > 16,
            "Resize regression no longer exceeds the former limit.");
        ProjectValidator.ValidateAnnotation(grown, page);

        AssertAnnotationRejected(
            new TextAnnotation
            {
                Text = new string(
                    'x',
                    ProjectValidator.MaximumTextLength + 1),
            },
            "Overlength text was accepted.");
        AssertAnnotationRejected(
            new ShapeAnnotation
            {
                Shape = ShapeKind.Rectangle,
                Start = new(-2, -2),
                End = new(-1, -1),
            },
            "Fully off-page annotation was accepted.");
    }

    private static void AssertAnnotationRejected(
        Annotation annotation,
        string message)
    {
        try
        {
            ProjectValidator.ValidateAnnotation(annotation);
        }
        catch (InvalidDataException)
        {
            return;
        }

        throw new InvalidOperationException(message);
    }

    private static async Task CheckTallPageRotationAsync(string root)
    {
        const int sourceWidth = 21;
        const int sourceHeight = 401;
        const float processedWidth = sourceWidth - 1;
        const float processedHeight = sourceHeight - 1;
        var rotation = AnnotationGeometry.PageRotation(
            MathF.PI / 2,
            new Vector2(0.5f, 0.5f),
            processedWidth,
            processedHeight);
        Assert(
            Math.Abs(rotation.M21) > 16,
            "Tall-page regression transform no longer exceeds raw cap.");
        var project = new FastFillProject
        {
            Title = "Tall page rotation",
            Pages =
            [
                new DocumentPage
                {
                    SourceAsset = "tall.png",
                    SourcePixelWidth = sourceWidth,
                    SourcePixelHeight = sourceHeight,
                    Annotations =
                    [
                        new ShapeAnnotation
                        {
                            Shape = ShapeKind.Rectangle,
                            Start = new(0.4f, 0.4f),
                            End = new(0.6f, 0.6f),
                            Transform = AffineTransform.FromMatrix(rotation),
                        },
                    ],
                },
            ],
        };
        using var image = new Mat(
            sourceHeight,
            sourceWidth,
            MatType.CV_8UC3,
            new Scalar(255, 255, 255));
        Assert(
            Cv2.ImEncode(".png", image, out var asset),
            "Tall-page test image encoding failed.");

        var workspace = WorkspaceStore.Create(root, project.Id);
        await File.WriteAllBytesAsync(
            Path.Combine(workspace, "assets", "tall.png"),
            asset);
        var archivePath = Path.Combine(
            Artifacts,
            "preview",
            "tall-page-rotation.docscan");
        await ProjectArchive.SaveAsync(project, workspace, archivePath);
        var loaded = await ProjectArchive.LoadAsync(
            archivePath,
            Path.Combine(root, "tall-page-loaded"));
        Assert(
            loaded.Project.Pages[0].Annotations[0].Transform
                == AffineTransform.FromMatrix(rotation),
            "Tall-page rotation changed during archive round trip.");
    }

    private static void AssertProjectRejected(
        FastFillProject project,
        string message)
    {
        try
        {
            ProjectValidator.Validate(project);
        }
        catch (InvalidDataException)
        {
            return;
        }

        throw new InvalidOperationException(message);
    }

    private static async Task CheckArchiveImageValidationAsync(
        FastFillProject project,
        byte[] source,
        string root)
    {
        ProjectValidator.ValidateEncodedImage(source);
        await CheckBoundedImageReadAsync(source);
        AssertInvalidImage(
            SetJpegDimensions(source, 8_000, 7_000),
            "Oversized encoded dimensions were accepted.");
        AssertInvalidImage(
            [0xff, 0xd8, 0xff, 0xd9],
            "Corrupt image content was accepted.");

        var falseDimensions = ProjectJson.Clone(project);
        falseDimensions.Pages[0] = falseDimensions.Pages[0] with
        {
            SourcePixelWidth = 1,
            SourcePixelHeight = 1,
        };
        await AssertArchiveRejectedAsync(
            falseDimensions,
            source,
            root,
            "false-dimensions",
            "Archive image dimensions were trusted.");

        var falseExtension = ProjectJson.Clone(project);
        falseExtension.Pages[0] = falseExtension.Pages[0] with
        {
            SourceAsset = "page-1.png",
        };
        await AssertArchiveRejectedAsync(
            falseExtension,
            source,
            root,
            "false-extension",
            "Archive image extension was trusted.");
    }

    private static async Task CheckBoundedImageReadAsync(byte[] source)
    {
        await using var valid = new MemoryStream(source, writable: false);
        var read = await ProjectValidator.ReadEncodedImageAsync(
            valid,
            source.LongLength);
        Assert(read.SequenceEqual(source), "Bounded image read changed data.");

        await using var changed = new MemoryStream(source, writable: false);
        await AssertImageReadRejectedAsync(
            changed,
            source.LongLength - 1,
            "Changed image size was accepted.");
        await AssertImageReadRejectedAsync(
            Stream.Null,
            ProjectValidator.MaximumImageBytes + 1,
            "Oversized image stream was accepted.");
    }

    private static async Task AssertImageReadRejectedAsync(
        Stream stream,
        long length,
        string message)
    {
        try
        {
            await ProjectValidator.ReadEncodedImageAsync(stream, length);
        }
        catch (InvalidDataException)
        {
            return;
        }

        throw new InvalidOperationException(message);
    }

    private static void AssertInvalidImage(byte[] bytes, string message)
    {
        try
        {
            ProjectValidator.ValidateEncodedImage(bytes);
        }
        catch (InvalidDataException)
        {
            return;
        }

        throw new InvalidOperationException(message);
    }

    private static byte[] SetJpegDimensions(
        byte[] source,
        ushort width,
        ushort height)
    {
        var changed = source.ToArray();
        for (var index = 0; index + 8 < changed.Length; index++)
        {
            if (changed[index] != 0xff
                || !IsJpegStartOfFrame(changed[index + 1]))
            {
                continue;
            }

            changed[index + 5] = (byte)(height >> 8);
            changed[index + 6] = (byte)height;
            changed[index + 7] = (byte)(width >> 8);
            changed[index + 8] = (byte)width;
            return changed;
        }

        throw new InvalidOperationException("JPEG frame header not found.");
    }

    private static bool IsJpegStartOfFrame(byte marker) => marker is
        0xc0 or 0xc1 or 0xc2 or 0xc3
        or 0xc5 or 0xc6 or 0xc7
        or 0xc9 or 0xca or 0xcb
        or 0xcd or 0xce or 0xcf;

    private static async Task AssertArchiveRejectedAsync(
        FastFillProject project,
        byte[] asset,
        string root,
        string name,
        string message)
    {
        var archivePath = Path.Combine(
            Artifacts,
            "preview",
            $"{name}.docscan");
        await WriteUncheckedArchiveAsync(archivePath, project, asset);
        var rejected = false;
        try
        {
            await ProjectArchive.LoadAsync(
                archivePath,
                Path.Combine(root, name));
        }
        catch (InvalidDataException)
        {
            rejected = true;
        }

        Assert(rejected, message);
    }

    private static async Task WriteUncheckedArchiveAsync(
        string path,
        FastFillProject project,
        byte[] asset)
    {
        await using var file = File.Create(path);
        using var archive = new ZipArchive(
            file,
            ZipArchiveMode.Create,
            leaveOpen: true);
        var manifest = archive.CreateEntry("project.json");
        await using (var output = manifest.Open())
        {
            await JsonSerializer.SerializeAsync(
                output,
                project,
                ProjectJson.Options);
        }

        var image = archive.CreateEntry(
            $"assets/{project.Pages[0].SourceAsset}");
        await using var imageOutput = image.Open();
        await imageOutput.WriteAsync(asset);
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
