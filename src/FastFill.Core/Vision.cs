using OpenCvSharp;

namespace FastFill.Core;

public sealed record DocumentDetection(
    CropQuad? Corners,
    double Confidence,
    double AreaRatio,
    double Sharpness)
{
    public bool Found => Corners is not null;
    public CropQuad? CandidateCorners { get; init; }
    public CropQuad? OverlayCorners => Corners ?? CandidateCorners;
}

public static class DocumentDetector
{
    private const int MaximumDetectionEdge = 1280;
    private const double MinimumDetectionConfidence = 0.7;

    public static DocumentDetection Detect(
        FramePacket frame,
        CropQuad? hint = null)
    {
        frame.Validate();
        ValidateHint(hint);
        using var source = Mat.FromPixelData(
            frame.Height,
            frame.Width,
            MatType.CV_8UC4,
            frame.Bgra32,
            frame.Stride);
        return Detect(source, hint);
    }

    public static DocumentDetection DetectEncoded(
        byte[] encodedImage,
        CropQuad? hint = null)
    {
        ArgumentNullException.ThrowIfNull(encodedImage);
        ValidateHint(hint);
        using var source = Cv2.ImDecode(encodedImage, ImreadModes.Color);
        if (source.Empty())
        {
            throw new ArgumentException("Image could not be decoded.");
        }

        return Detect(source, hint);
    }

    private static DocumentDetection Detect(Mat source, CropQuad? hint)
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

        using var color = new Mat();
        if (scaled.Channels() == 4)
        {
            Cv2.CvtColor(scaled, color, ColorConversionCodes.BGRA2BGR);
        }
        else
        {
            scaled.CopyTo(color);
        }

        using var hsv = new Mat();
        using var paperMask = new Mat();
        Cv2.CvtColor(color, hsv, ColorConversionCodes.BGR2HSV);
        Cv2.InRange(
            hsv,
            new Scalar(0, 0, 115),
            new Scalar(180, 105, 255),
            paperMask);
        using var paperKernel = Cv2.GetStructuringElement(
            MorphShapes.Ellipse,
            new Size(9, 9));
        using var paperOpenKernel = Cv2.GetStructuringElement(
            MorphShapes.Ellipse,
            new Size(25, 25));
        Cv2.MorphologyEx(
            paperMask,
            paperMask,
            MorphTypes.Close,
            paperKernel,
            iterations: 2);
        Cv2.MorphologyEx(
            paperMask,
            paperMask,
            MorphTypes.Open,
            paperOpenKernel);
        Cv2.FindContours(
            paperMask,
            out var paperContours,
            out _,
            RetrievalModes.External,
            ContourApproximationModes.ApproxSimple);
        contours = contours.Concat(paperContours).ToArray();

        CropQuad? best = null;
        var bestScore = 0d;
        var bestRank = 0d;
        var bestAreaRatio = 0d;
        var imageArea = scaled.Width * (double)scaled.Height;
        var candidates = new List<Point[]>();
        foreach (var contour in contours)
        {
            var hull = Cv2.ConvexHull(contour);
            foreach (var outline in new[] { contour, hull })
            {
                var perimeter = Cv2.ArcLength(outline, true);
                foreach (var epsilon in new[]
                {
                    0.01,
                    0.015,
                    0.02,
                    0.03,
                    0.04,
                })
                {
                    var polygon = Cv2.ApproxPolyDP(
                        outline,
                        perimeter * epsilon,
                        true);
                    if (polygon.Length != 4
                        || !Cv2.IsContourConvex(polygon))
                    {
                        continue;
                    }

                    candidates.Add(polygon);
                }
            }
        }

        candidates.AddRange(FindLineQuadrilaterals(edges));
        foreach (var polygon in candidates)
        {
            var scored = ScoreCandidate(
                polygon,
                scaled.Size(),
                imageArea,
                paperMask);
            if (scored is null)
            {
                continue;
            }

            var ordered = OrderCorners(polygon);
            var quad = new CropQuad(
                Normalize(ordered[0], scaled.Size()),
                Normalize(ordered[1], scaled.Size()),
                Normalize(ordered[2], scaled.Size()),
                Normalize(ordered[3], scaled.Size()));
            var rank = scored.Value.Score + HintBonus(quad, hint);
            if (rank <= bestRank)
            {
                continue;
            }

            best = quad;
            bestScore = scored.Value.Score;
            bestRank = rank;
            bestAreaRatio = scored.Value.AreaRatio;
        }

        using var laplacian = new Mat();
        Cv2.Laplacian(gray, laplacian, MatType.CV_64F);
        Cv2.MeanStdDev(laplacian, out _, out var deviation);
        var sharpness = deviation.Val0 * deviation.Val0;
        if (best is null)
        {
            return new(null, 0, 0, sharpness);
        }

        if (bestScore < MinimumDetectionConfidence)
        {
            return new(
                null,
                Math.Clamp(bestScore, 0, 1),
                bestAreaRatio,
                sharpness)
            {
                CandidateCorners = best,
            };
        }

        return new(
            best,
            Math.Clamp(bestScore, 0, 1),
            bestAreaRatio,
            sharpness);
    }

    private static double HintBonus(CropQuad candidate, CropQuad? hint)
    {
        if (hint is null)
        {
            return 0;
        }

        const float maximumHintDistance = 0.12f;
        var distance = candidate.MaximumCornerDistance(hint);
        return distance >= maximumHintDistance
            ? 0
            : 0.12 * (1 - (distance / maximumHintDistance));
    }

    private static void ValidateHint(CropQuad? hint)
    {
        if (hint is not null && !hint.IsConvex())
        {
            throw new ArgumentException("Detection hint must be convex.");
        }
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

    private static (double Score, double AreaRatio)? ScoreCandidate(
        Point[] polygon,
        Size imageSize,
        double imageArea,
        Mat paperMask)
    {
        var area = Math.Abs(Cv2.ContourArea(polygon));
        var areaRatio = area / imageArea;
        if (areaRatio < 0.05 || !Cv2.IsContourConvex(polygon))
        {
            return null;
        }

        var rectangle = Cv2.MinAreaRect(polygon);
        var rectangleArea = rectangle.Size.Width
            * (double)rectangle.Size.Height;
        var shortestSide = Math.Min(
            rectangle.Size.Width,
            rectangle.Size.Height);
        var longestSide = Math.Max(
            rectangle.Size.Width,
            rectangle.Size.Height);
        if (rectangleArea <= 0
            || shortestSide <= 0
            || longestSide / shortestSide > 5)
        {
            return null;
        }

        var rectangularity = Math.Min(1, area / rectangleArea);
        // More area stops helping once a document occupies 40% of the frame.
        // This prevents unrelated lines from inflating a good document quad.
        var areaScore = Math.Min(1, areaRatio / 0.4);
        var border = Math.Min(imageSize.Width, imageSize.Height) * 0.02;
        var borderCorners = polygon.Count(point =>
            point.X <= border
            || point.Y <= border
            || point.X >= imageSize.Width - border
            || point.Y >= imageSize.Height - border);
        var borderPenalty = borderCorners * 0.12;
        var geometryScore = (areaScore * 0.45)
            + (rectangularity * 0.55);
        var score = (geometryScore * 0.6) - borderPenalty;
        if (score + 0.4 < MinimumDetectionConfidence)
        {
            return (score, areaRatio);
        }

        score += PaperBoundaryScore(polygon, paperMask) * 0.4;
        return (score, areaRatio);
    }

    private static double PaperBoundaryScore(
        Point[] polygon,
        Mat paperMask)
    {
        const int samplesPerSide = 20;
        var corners = OrderCorners(polygon);
        var offset = Math.Max(
            2,
            Math.Min(paperMask.Width, paperMask.Height) * 0.0125);
        var insidePaper = 0;
        var outsidePaper = 0;
        var sampleCount = 0;
        for (var side = 0; side < corners.Length; side++)
        {
            var first = corners[side];
            var second = corners[(side + 1) % corners.Length];
            var dx = second.X - first.X;
            var dy = second.Y - first.Y;
            var length = Math.Sqrt((dx * dx) + (dy * dy));
            if (length < 1)
            {
                continue;
            }

            // Ordered image coordinates are clockwise; right normal is inward.
            var normalX = -dy / length;
            var normalY = dx / length;
            for (var sample = 0; sample < samplesPerSide; sample++)
            {
                var position = (sample + 0.5) / samplesPerSide;
                var x = first.X + (dx * position);
                var y = first.Y + (dy * position);
                var insideX = (int)Math.Round(x + (normalX * offset));
                var insideY = (int)Math.Round(y + (normalY * offset));
                var outsideX = (int)Math.Round(x - (normalX * offset));
                var outsideY = (int)Math.Round(y - (normalY * offset));
                if (!Contains(paperMask.Size(), insideX, insideY)
                    || !Contains(paperMask.Size(), outsideX, outsideY))
                {
                    continue;
                }

                insidePaper += paperMask.At<byte>(insideY, insideX) > 0
                    ? 1
                    : 0;
                outsidePaper += paperMask.At<byte>(outsideY, outsideX) > 0
                    ? 1
                    : 0;
                sampleCount++;
            }
        }

        if (sampleCount == 0)
        {
            return 0;
        }

        var contrast = (insidePaper - outsidePaper)
            / (double)sampleCount;
        return Math.Clamp(contrast / 0.6, 0, 1);
    }

    private static bool Contains(Size size, int x, int y) =>
        x >= 0 && x < size.Width && y >= 0 && y < size.Height;

    private static IEnumerable<Point[]> FindLineQuadrilaterals(Mat edges)
    {
        var minimumLength = Math.Min(edges.Width, edges.Height) * 0.18;
        var lines = Cv2.HoughLinesP(
            edges,
            1,
            Math.PI / 180,
            70,
            minimumLength,
            60);
        var horizontal = lines
            .Where(line => Math.Abs(line.P2.X - line.P1.X)
                >= Math.Abs(line.P2.Y - line.P1.Y) * 1.5)
            .OrderByDescending(LineLength)
            .Take(14)
            .ToArray();
        var vertical = lines
            .Where(line => Math.Abs(line.P2.Y - line.P1.Y)
                >= Math.Abs(line.P2.X - line.P1.X) * 1.5)
            .OrderByDescending(LineLength)
            .Take(14)
            .ToArray();
        for (var firstH = 0; firstH < horizontal.Length; firstH++)
        {
            for (var secondH = firstH + 1;
                secondH < horizontal.Length;
                secondH++)
            {
                var top = horizontal[firstH];
                var bottom = horizontal[secondH];
                if (LineMidpoint(top).Y > LineMidpoint(bottom).Y)
                {
                    (top, bottom) = (bottom, top);
                }

                if (LineMidpoint(bottom).Y - LineMidpoint(top).Y
                    < edges.Height * 0.12)
                {
                    continue;
                }

                foreach (var polygon in VerticalLinePairs(
                    top,
                    bottom,
                    vertical,
                    edges.Size()))
                {
                    yield return polygon;
                }
            }
        }
    }

    private static IEnumerable<Point[]> VerticalLinePairs(
        LineSegmentPoint top,
        LineSegmentPoint bottom,
        LineSegmentPoint[] vertical,
        Size imageSize)
    {
        for (var firstV = 0; firstV < vertical.Length; firstV++)
        {
            for (var secondV = firstV + 1;
                secondV < vertical.Length;
                secondV++)
            {
                var left = vertical[firstV];
                var right = vertical[secondV];
                if (LineMidpoint(left).X > LineMidpoint(right).X)
                {
                    (left, right) = (right, left);
                }

                if (LineMidpoint(right).X - LineMidpoint(left).X
                    < imageSize.Width * 0.12
                    || !TryIntersect(top, left, out var topLeft)
                    || !TryIntersect(top, right, out var topRight)
                    || !TryIntersect(bottom, right, out var bottomRight)
                    || !TryIntersect(bottom, left, out var bottomLeft))
                {
                    continue;
                }

                var points = new[]
                {
                    topLeft,
                    topRight,
                    bottomRight,
                    bottomLeft,
                };
                var margin = Math.Min(
                    imageSize.Width,
                    imageSize.Height) * 0.05;
                if (points.Any(point =>
                    point.X < -margin
                    || point.Y < -margin
                    || point.X > imageSize.Width + margin
                    || point.Y > imageSize.Height + margin))
                {
                    continue;
                }

                yield return points.Select(point => new Point(
                    Math.Clamp(
                        (int)Math.Round(point.X),
                        0,
                        imageSize.Width - 1),
                    Math.Clamp(
                        (int)Math.Round(point.Y),
                        0,
                        imageSize.Height - 1))).ToArray();
            }
        }
    }

    private static bool TryIntersect(
        LineSegmentPoint first,
        LineSegmentPoint second,
        out Point2d intersection)
    {
        var firstA = first.P2.Y - first.P1.Y;
        var firstB = first.P1.X - first.P2.X;
        var firstC = firstA * first.P1.X + firstB * first.P1.Y;
        var secondA = second.P2.Y - second.P1.Y;
        var secondB = second.P1.X - second.P2.X;
        var secondC = secondA * second.P1.X + secondB * second.P1.Y;
        var determinant = firstA * secondB - secondA * firstB;
        if (Math.Abs(determinant) < 0.001)
        {
            intersection = default;
            return false;
        }

        intersection = new Point2d(
            (secondB * firstC - firstB * secondC) / determinant,
            (firstA * secondC - secondA * firstC) / determinant);
        return true;
    }

    private static Point2d LineMidpoint(LineSegmentPoint line) => new(
        (line.P1.X + line.P2.X) / 2d,
        (line.P1.Y + line.P2.Y) / 2d);

    private static double LineLength(LineSegmentPoint line)
    {
        var x = line.P2.X - line.P1.X;
        var y = line.P2.Y - line.P1.Y;
        return Math.Sqrt(x * x + y * y);
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

public sealed class DocumentDetectionStabilizer
{
    private const int WindowSize = 5;
    private const int RequiredVotes = 3;
    private const float AgreementDistance = 0.08f;
    private const float SmoothingFactor = 0.35f;

    private readonly Queue<DocumentDetection> _history = new();
    private CropQuad? _corners;

    public CropQuad? Hint => _corners;

    public DocumentDetection Update(DocumentDetection detection)
    {
        ArgumentNullException.ThrowIfNull(detection);
        _history.Enqueue(detection);
        while (_history.Count > WindowSize)
        {
            _history.Dequeue();
        }

        var found = _history
            .Where(item => item.Corners is not null)
            .ToArray();
        var cluster = FindConsensus(found);
        if (cluster.Length < RequiredVotes)
        {
            if (_history.Count == WindowSize)
            {
                _corners = null;
            }

            return new(null, 0, 0, detection.Sharpness)
            {
                CandidateCorners = _corners,
            };
        }

        var target = MedianQuad(cluster.Select(item => item.Corners!));
        _corners = _corners is null
            || _corners.MaximumCornerDistance(target) > AgreementDistance
                ? target
                : Interpolate(_corners, target, SmoothingFactor);
        return new(
            _corners,
            Median(cluster.Select(item => item.Confidence)),
            Median(cluster.Select(item => item.AreaRatio)),
            detection.Sharpness);
    }

    public void Reset()
    {
        _history.Clear();
        _corners = null;
    }

    private DocumentDetection[] FindConsensus(
        DocumentDetection[] detections)
    {
        if (_corners is not null)
        {
            var current = Matching(detections, _corners);
            if (current.Length >= RequiredVotes)
            {
                return current;
            }
        }

        return detections
            .Select(seed => Matching(detections, seed.Corners!))
            .OrderByDescending(cluster => cluster.Length)
            .ThenByDescending(cluster =>
                cluster.Sum(item => item.Confidence))
            .FirstOrDefault() ?? [];
    }

    private static DocumentDetection[] Matching(
        IEnumerable<DocumentDetection> detections,
        CropQuad center) => detections
        .Where(item => item.Corners!
            .MaximumCornerDistance(center) <= AgreementDistance)
        .ToArray();

    private static CropQuad MedianQuad(IEnumerable<CropQuad> values)
    {
        var quads = values.ToArray();
        return new CropQuad(
            MedianPoint(quads.Select(value => value.TopLeft)),
            MedianPoint(quads.Select(value => value.TopRight)),
            MedianPoint(quads.Select(value => value.BottomRight)),
            MedianPoint(quads.Select(value => value.BottomLeft)));
    }

    private static NormalizedPoint MedianPoint(
        IEnumerable<NormalizedPoint> values)
    {
        var points = values.ToArray();
        return new NormalizedPoint(
            (float)Median(points.Select(point => (double)point.X)),
            (float)Median(points.Select(point => (double)point.Y)));
    }

    private static double Median(IEnumerable<double> values)
    {
        var ordered = values.Order().ToArray();
        if (ordered.Length == 0)
        {
            return 0;
        }

        var middle = ordered.Length / 2;
        return ordered.Length % 2 == 0
            ? (ordered[middle - 1] + ordered[middle]) / 2
            : ordered[middle];
    }

    private static CropQuad Interpolate(
        CropQuad current,
        CropQuad target,
        float amount) => new(
        Interpolate(current.TopLeft, target.TopLeft, amount),
        Interpolate(current.TopRight, target.TopRight, amount),
        Interpolate(current.BottomRight, target.BottomRight, amount),
        Interpolate(current.BottomLeft, target.BottomLeft, amount));

    private static NormalizedPoint Interpolate(
        NormalizedPoint current,
        NormalizedPoint target,
        float amount) => new(
        current.X + ((target.X - current.X) * amount),
        current.Y + ((target.Y - current.Y) * amount));
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
    public static TimeSpan CountdownDuration { get; } =
        TimeSpan.FromSeconds(3);

    private CropQuad? _lastCorners;
    private DateTimeOffset? _stableSince;
    private bool _captured;

    public double Progress { get; private set; }

    public AutoCaptureState Evaluate(
        DocumentDetection detection,
        DateTimeOffset timestamp,
        double minimumSharpness = 45)
    {
        if (!detection.Found
            || detection.Confidence < 0.7
            || detection.AreaRatio < 0.1
            || detection.Sharpness < minimumSharpness)
        {
            Reset();
            return AutoCaptureState.NoDocument;
        }

        if (_captured)
        {
            Progress = 1;
            return AutoCaptureState.Captured;
        }

        var corners = detection.Corners!;
        if (_lastCorners is null
            || corners.MaximumCornerDistance(_lastCorners) > 0.04f)
        {
            _stableSince = timestamp;
            _lastCorners = corners;
            Progress = 0;
            return AutoCaptureState.HoldSteady;
        }

        var stableSince = _stableSince ?? timestamp;
        var elapsed = timestamp - stableSince;
        Progress = Math.Clamp(
            elapsed / CountdownDuration,
            0,
            1);
        if (elapsed < CountdownDuration)
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
        Progress = 0;
    }
}
