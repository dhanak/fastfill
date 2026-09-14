using OpenCvSharp;

namespace FastFill.Core;

public sealed record DocumentDetection(
    CropQuad? Corners,
    double Confidence,
    double AreaRatio,
    double Sharpness)
{
    public bool Found => Corners is not null;
}

public static class DocumentDetector
{
    private const int MaximumDetectionEdge = 1280;

    public static DocumentDetection Detect(FramePacket frame)
    {
        frame.Validate();
        using var source = Mat.FromPixelData(
            frame.Height,
            frame.Width,
            MatType.CV_8UC4,
            frame.Bgra32,
            frame.Stride);
        return Detect(source);
    }

    public static DocumentDetection DetectEncoded(byte[] encodedImage)
    {
        ArgumentNullException.ThrowIfNull(encodedImage);
        using var source = Cv2.ImDecode(encodedImage, ImreadModes.Color);
        if (source.Empty())
        {
            throw new ArgumentException("Image could not be decoded.");
        }

        return Detect(source);
    }

    private static DocumentDetection Detect(Mat source)
    {
        using var scaled = ResizeForDetection(source);
        using var gray = new Mat();
        if (scaled.Channels() == 4)
        {
            Cv2.CvtColor(scaled, gray, ColorConversionCodes.BGRA2GRAY);
        }
        else
        {
            Cv2.CvtColor(scaled, gray, ColorConversionCodes.BGR2GRAY);
        }

        using var blurred = new Mat();
        Cv2.GaussianBlur(gray, blurred, new Size(5, 5), 0);
        using var edges = new Mat();
        Cv2.Canny(blurred, edges, 60, 180);
        using var kernel = Cv2.GetStructuringElement(
            MorphShapes.Rect,
            new Size(5, 5));
        Cv2.MorphologyEx(
            edges,
            edges,
            MorphTypes.Close,
            kernel,
            iterations: 2);

        Cv2.FindContours(
            edges,
            out var contours,
            out _,
            RetrievalModes.List,
            ContourApproximationModes.ApproxSimple);

        Point[]? best = null;
        var bestScore = 0d;
        var bestAreaRatio = 0d;
        var imageArea = scaled.Width * (double)scaled.Height;
        foreach (var contour in contours)
        {
            var perimeter = Cv2.ArcLength(contour, true);
            var polygon = Cv2.ApproxPolyDP(contour, perimeter * 0.02, true);
            if (polygon.Length != 4 || !Cv2.IsContourConvex(polygon))
            {
                continue;
            }

            var area = Math.Abs(Cv2.ContourArea(polygon));
            var areaRatio = area / imageArea;
            if (areaRatio < 0.05)
            {
                continue;
            }

            var bounds = Cv2.BoundingRect(polygon);
            var rectangularity = area / (bounds.Width * (double)bounds.Height);
            var areaScore = Math.Min(1, areaRatio / 0.65);
            var score = (areaScore * 0.65) + (rectangularity * 0.35);
            if (score <= bestScore)
            {
                continue;
            }

            best = polygon;
            bestScore = score;
            bestAreaRatio = areaRatio;
        }

        using var laplacian = new Mat();
        Cv2.Laplacian(gray, laplacian, MatType.CV_64F);
        Cv2.MeanStdDev(laplacian, out _, out var deviation);
        var sharpness = deviation.Val0 * deviation.Val0;
        if (best is null)
        {
            return new(null, 0, 0, sharpness);
        }

        var ordered = OrderCorners(best);
        var quad = new CropQuad(
            Normalize(ordered[0], scaled.Size()),
            Normalize(ordered[1], scaled.Size()),
            Normalize(ordered[2], scaled.Size()),
            Normalize(ordered[3], scaled.Size()));
        return new(
            quad,
            Math.Clamp(bestScore, 0, 1),
            bestAreaRatio,
            sharpness);
    }

    private static Mat ResizeForDetection(Mat source)
    {
        var longest = Math.Max(source.Width, source.Height);
        if (longest <= MaximumDetectionEdge)
        {
            return source.Clone();
        }

        var scale = MaximumDetectionEdge / (double)longest;
        var result = new Mat();
        Cv2.Resize(
            source,
            result,
            new Size(),
            scale,
            scale,
            InterpolationFlags.Area);
        return result;
    }

    private static Point[] OrderCorners(Point[] points)
    {
        var topLeft = points.MinBy(point => point.X + point.Y);
        var bottomRight = points.MaxBy(point => point.X + point.Y);
        var topRight = points.MaxBy(point => point.X - point.Y);
        var bottomLeft = points.MinBy(point => point.X - point.Y);
        return [topLeft, topRight, bottomRight, bottomLeft];
    }

    private static NormalizedPoint Normalize(Point point, Size size) =>
        new(
            point.X / (float)Math.Max(1, size.Width - 1),
            point.Y / (float)Math.Max(1, size.Height - 1));
}

public sealed record ProcessedPage(byte[] EncodedPng, int Width, int Height);

public static class ImageProcessor
{
    public static ProcessedPage Process(
        byte[] encodedImage,
        CropQuad crop,
        int rotationQuarterTurns,
        PageFilterSettings filter)
    {
        ArgumentNullException.ThrowIfNull(encodedImage);
        ArgumentNullException.ThrowIfNull(crop);
        ArgumentNullException.ThrowIfNull(filter);
        if (!crop.IsConvex())
        {
            throw new ArgumentException("Crop corners must be convex.");
        }

        if (filter.Contrast is < -100 or > 100)
        {
            throw new ArgumentOutOfRangeException(
                nameof(filter),
                "Contrast must be between -100 and 100.");
        }

        using var source = Cv2.ImDecode(encodedImage, ImreadModes.Color);
        if (source.Empty())
        {
            throw new ArgumentException("Image could not be decoded.");
        }

        using var corrected = CorrectPerspective(source, crop.Clamp());
        using var rotated = Rotate(corrected, rotationQuarterTurns);
        using var filtered = ApplyFilter(rotated, filter);
        if (!Cv2.ImEncode(".png", filtered, out var bytes))
        {
            throw new InvalidOperationException(
                "Processed image encoding failed.");
        }

        return new(bytes, filtered.Width, filtered.Height);
    }

    private static Mat CorrectPerspective(Mat source, CropQuad crop)
    {
        var points = crop.Points
            .Select(point => new Point2f(
                point.X * (source.Width - 1),
                point.Y * (source.Height - 1)))
            .ToArray();
        var width = Math.Max(
            Distance(points[0], points[1]),
            Distance(points[3], points[2]));
        var height = Math.Max(
            Distance(points[0], points[3]),
            Distance(points[1], points[2]));
        var outputWidth = Math.Max(1, (int)Math.Round(width));
        var outputHeight = Math.Max(1, (int)Math.Round(height));
        Point2f[] destination =
        [
            new(0, 0),
            new(outputWidth - 1, 0),
            new(outputWidth - 1, outputHeight - 1),
            new(0, outputHeight - 1),
        ];
        using var transform = Cv2.GetPerspectiveTransform(points, destination);
        var result = new Mat();
        Cv2.WarpPerspective(
            source,
            result,
            transform,
            new Size(outputWidth, outputHeight),
            InterpolationFlags.Cubic,
            BorderTypes.Replicate);
        return result;
    }

    private static Mat Rotate(Mat source, int quarterTurns)
    {
        var normalized = ((quarterTurns % 4) + 4) % 4;
        if (normalized == 0)
        {
            return source.Clone();
        }

        var result = new Mat();
        var flag = normalized switch
        {
            1 => RotateFlags.Rotate90Clockwise,
            2 => RotateFlags.Rotate180,
            _ => RotateFlags.Rotate90Counterclockwise,
        };
        Cv2.Rotate(source, result, flag);
        return result;
    }

    private static Mat ApplyFilter(Mat source, PageFilterSettings settings)
    {
        var result = source.Clone();
        switch (settings.Mode)
        {
            case PageFilterMode.EnhancedColor:
                EnhanceColor(result);
                break;
            case PageFilterMode.Grayscale:
                ConvertToGrayscale(result);
                break;
            case PageFilterMode.BlackAndWhite:
                ConvertToBlackAndWhite(result);
                break;
            case PageFilterMode.Original:
                break;
            default:
                throw new ArgumentOutOfRangeException(nameof(settings));
        }

        if (settings.Contrast != 0)
        {
            var alpha = Math.Max(0, 1 + (settings.Contrast / 100d));
            result.ConvertTo(result, -1, alpha, 128 * (1 - alpha));
        }

        return result;
    }

    private static void EnhanceColor(Mat image)
    {
        var mean = Cv2.Mean(image);
        var target = (mean.Val0 + mean.Val1 + mean.Val2) / 3;
        var channels = Cv2.Split(image);
        try
        {
            var means = new[] { mean.Val0, mean.Val1, mean.Val2 };
            for (var index = 0; index < channels.Length; index++)
            {
                var scale = target / Math.Max(1, means[index]);
                channels[index].ConvertTo(channels[index], -1, scale);
            }

            Cv2.Merge(channels, image);
        }
        finally
        {
            foreach (var channel in channels)
            {
                channel.Dispose();
            }
        }

        using var lab = new Mat();
        Cv2.CvtColor(image, lab, ColorConversionCodes.BGR2Lab);
        var labChannels = Cv2.Split(lab);
        try
        {
            using var clahe = Cv2.CreateCLAHE(2, new Size(8, 8));
            clahe.Apply(labChannels[0], labChannels[0]);
            Cv2.Merge(labChannels, lab);
        }
        finally
        {
            foreach (var channel in labChannels)
            {
                channel.Dispose();
            }
        }

        Cv2.CvtColor(lab, image, ColorConversionCodes.Lab2BGR);
    }

    private static void ConvertToGrayscale(Mat image)
    {
        using var gray = new Mat();
        Cv2.CvtColor(image, gray, ColorConversionCodes.BGR2GRAY);
        Cv2.CvtColor(gray, image, ColorConversionCodes.GRAY2BGR);
    }

    private static void ConvertToBlackAndWhite(Mat image)
    {
        using var gray = new Mat();
        using var denoised = new Mat();
        using var binary = new Mat();
        Cv2.CvtColor(image, gray, ColorConversionCodes.BGR2GRAY);
        Cv2.MedianBlur(gray, denoised, 3);
        Cv2.AdaptiveThreshold(
            denoised,
            binary,
            255,
            AdaptiveThresholdTypes.GaussianC,
            ThresholdTypes.Binary,
            31,
            12);
        Cv2.CvtColor(binary, image, ColorConversionCodes.GRAY2BGR);
    }

    private static double Distance(Point2f first, Point2f second)
    {
        var x = first.X - second.X;
        var y = first.Y - second.Y;
        return Math.Sqrt((x * x) + (y * y));
    }
}

public enum AutoCaptureState
{
    NoDocument,
    HoldSteady,
    Ready,
    Captured,
}

public sealed class AutoCaptureGate
{
    private static readonly TimeSpan StableDuration =
        TimeSpan.FromMilliseconds(750);

    private CropQuad? _lastCorners;
    private DateTimeOffset? _stableSince;
    private bool _captured;

    public AutoCaptureState Evaluate(
        DocumentDetection detection,
        DateTimeOffset timestamp,
        double minimumSharpness = 80)
    {
        if (!detection.Found
            || detection.Confidence < 0.8
            || detection.AreaRatio < 0.25
            || detection.Sharpness < minimumSharpness)
        {
            Reset();
            return AutoCaptureState.NoDocument;
        }

        if (_captured)
        {
            return AutoCaptureState.Captured;
        }

        var corners = detection.Corners!;
        if (_lastCorners is null
            || corners.MaximumCornerDistance(_lastCorners) > 0.015f)
        {
            _stableSince = timestamp;
            _lastCorners = corners;
            return AutoCaptureState.HoldSteady;
        }

        _lastCorners = corners;
        if (timestamp - _stableSince < StableDuration)
        {
            return AutoCaptureState.HoldSteady;
        }

        _captured = true;
        return AutoCaptureState.Ready;
    }

    public void Reset()
    {
        _lastCorners = null;
        _stableSince = null;
        _captured = false;
    }
}
