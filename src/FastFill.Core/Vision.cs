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

public sealed record SnapLineFeature(
    NormalizedPoint Start,
    NormalizedPoint End);

public sealed class ImageSnapFeatures
{
    private const int MaximumAnalysisEdge = 1600;

    private readonly int _width;
    private readonly int _height;
    private readonly NormalizedPoint[] _corners;
    private readonly NormalizedRect[] _boxes;
    private readonly SnapLineFeature[] _horizontalLines;

    private ImageSnapFeatures(
        int width,
        int height,
        NormalizedPoint[] corners,
        NormalizedRect[] boxes,
        SnapLineFeature[] horizontalLines)
    {
        _width = width;
        _height = height;
        _corners = corners;
        _boxes = boxes;
        _horizontalLines = horizontalLines;
    }

    public IReadOnlyList<SnapLineFeature> HorizontalLines =>
        _horizontalLines;

    public IReadOnlyList<NormalizedRect> Boxes => _boxes;

    public static ImageSnapFeatures Analyze(byte[] encodedImage)
    {
        ArgumentNullException.ThrowIfNull(encodedImage);
        using var source = Cv2.ImDecode(encodedImage, ImreadModes.Grayscale);
        if (source.Empty())
        {
            throw new ArgumentException("Image could not be decoded.");
        }

        using var image = Resize(source);
        if (image.Width < 16 || image.Height < 16)
        {
            return new(image.Width, image.Height, [], [], []);
        }

        var cornerPoints = Cv2.GoodFeaturesToTrack(
            image,
            300,
            0.02,
            8,
            null!,
            5,
            false,
            0.04);
        var corners = cornerPoints
            .Select(point => Normalize(point, image.Size()))
            .ToArray();

        using var binary = new Mat();
        Cv2.AdaptiveThreshold(
            image,
            binary,
            255,
            AdaptiveThresholdTypes.GaussianC,
            ThresholdTypes.BinaryInv,
            31,
            10);
        Cv2.FindContours(
            binary,
            out var contours,
            out _,
            RetrievalModes.External,
            ContourApproximationModes.ApproxSimple);
        var minimumSide = Math.Max(
            8,
            Math.Min(image.Width, image.Height) * 0.006);
        var maximumSide = Math.Min(image.Width, image.Height) * 0.12;
        var boxes = contours
            .Select(contour => new
            {
                Contour = contour,
                Perimeter = Cv2.ArcLength(contour, true),
            })
            .Select(value => new
            {
                Polygon = Cv2.ApproxPolyDP(
                    value.Contour,
                    value.Perimeter * 0.03,
                    true),
                Bounds = Cv2.BoundingRect(value.Contour),
            })
            .Where(value => value.Polygon.Length == 4
                && Cv2.IsContourConvex(value.Polygon)
                && value.Bounds.Width >= minimumSide
                && value.Bounds.Height >= minimumSide
                && value.Bounds.Width <= maximumSide
                && value.Bounds.Height <= maximumSide
                && value.Bounds.Width / (double)value.Bounds.Height
                    is >= 0.65 and <= 1.55)
            .Select(value => Normalize(value.Bounds, image.Size()))
            .OrderByDescending(value => value.Width * value.Height)
            .ToArray();

        var bridgeWidth = Math.Clamp(
            (int)Math.Round(image.Width * 0.006),
            3,
            10);
        var minimumLineWidth = Math.Max(
            20,
            (int)Math.Round(image.Width * 0.025));
        var maximumLineHeight = Math.Max(
            5,
            (int)Math.Round(
                Math.Min(image.Width, image.Height) * 0.012));
        using var lineMask = new Mat();
        using var lineKernel = Cv2.GetStructuringElement(
            MorphShapes.Rect,
            new Size(minimumLineWidth, 1));
        Cv2.MorphologyEx(
            binary,
            lineMask,
            MorphTypes.Open,
            lineKernel);
        Cv2.FindContours(
            lineMask,
            out var lineContours,
            out _,
            RetrievalModes.External,
            ContourApproximationModes.ApproxSimple);
        var horizontalLines = lineContours
            .Select(Cv2.BoundingRect)
            .Where(bounds => bounds.Width >= minimumLineWidth
                && bounds.Height <= maximumLineHeight
                && bounds.Width >= bounds.Height * 5)
            .Select(bounds => new SnapLineFeature(
                Normalize(
                    new Point(
                        bounds.X,
                        bounds.Y + bounds.Height / 2),
                    image.Size()),
                Normalize(
                    new Point(
                        bounds.Right - 1,
                        bounds.Y + bounds.Height / 2),
                    image.Size())))
            .Concat(FindBrokenHorizontalLines(
                contours,
                image.Size(),
                minimumLineWidth,
                maximumLineHeight,
                bridgeWidth))
            .OrderByDescending(line => line.End.X - line.Start.X)
            .ToArray();
        return new(
            image.Width,
            image.Height,
            corners,
            boxes,
            horizontalLines);
    }

    private static IEnumerable<SnapLineFeature>
        FindBrokenHorizontalLines(
            Point[][] contours,
            Size imageSize,
            int minimumLineWidth,
            int maximumLineHeight,
            int maximumGap)
    {
        var alignmentTolerance = Math.Max(2, maximumLineHeight / 3);
        var maximumMarkHeight = Math.Max(3, maximumLineHeight / 2);
        var marks = contours
            .Select(Cv2.BoundingRect)
            .Where(bounds => bounds.Height <= maximumLineHeight
                && bounds.Width <= minimumLineWidth * 2
                && (bounds.Height <= maximumMarkHeight
                    || bounds.Width >= bounds.Height * 2))
            .OrderBy(bounds => bounds.Bottom)
            .ToArray();
        var rows = new List<List<Rect>>();
        foreach (var mark in marks)
        {
            var row = rows
                .Where(candidate => Math.Abs(
                    candidate.Average(bounds => bounds.Bottom)
                        - mark.Bottom) <= alignmentTolerance)
                .MinBy(candidate => Math.Abs(
                    candidate.Average(bounds => bounds.Bottom)
                        - mark.Bottom));
            if (row is null)
            {
                row = [];
                rows.Add(row);
            }

            row.Add(mark);
        }

        foreach (var row in rows)
        {
            var group = new List<Rect>();
            foreach (var mark in row.OrderBy(bounds => bounds.X))
            {
                if (group.Count > 0
                    && mark.X - group[^1].Right > maximumGap)
                {
                    if (CreateBrokenLine(
                        group,
                        imageSize,
                        minimumLineWidth) is { } line)
                    {
                        yield return line;
                    }

                    group.Clear();
                }

                group.Add(mark);
            }

            if (CreateBrokenLine(
                group,
                imageSize,
                minimumLineWidth) is { } finalLine)
            {
                yield return finalLine;
            }
        }
    }

    private static SnapLineFeature? CreateBrokenLine(
        List<Rect> marks,
        Size imageSize,
        int minimumLineWidth)
    {
        const int minimumMarkCount = 6;
        if (marks.Count < minimumMarkCount)
        {
            return null;
        }

        var gaps = marks
            .Zip(
                marks.Skip(1),
                (left, right) => Math.Max(0, right.X - left.Right))
            .Order()
            .ToArray();
        var typicalGap = gaps[gaps.Length / 2];
        var maximumEdgeGap = typicalGap + 2;
        var first = 0;
        var last = marks.Count - 1;
        while (last - first + 1 > minimumMarkCount
            && marks[first + 1].X - marks[first].Right
                > maximumEdgeGap)
        {
            first++;
        }

        while (last - first + 1 > minimumMarkCount
            && marks[last].X - marks[last - 1].Right
                > maximumEdgeGap)
        {
            last--;
        }

        var lineMarks = marks
            .Skip(first)
            .Take(last - first + 1)
            .ToArray();
        var left = lineMarks[0].X;
        var right = lineMarks[^1].Right;
        var width = right - left;
        var inkWidth = lineMarks.Sum(mark => mark.Width);
        var maximumDotSize = Math.Max(4, minimumLineWidth / 4);
        var isDotted = lineMarks.All(mark =>
            mark.Width <= maximumDotSize
            && mark.Height <= maximumDotSize);
        var requiredWidth = isDotted
            ? minimumLineWidth
            : minimumLineWidth * 2;
        if (width < requiredWidth
            || inkWidth < width * 0.2
            || inkWidth > width * 0.9)
        {
            return null;
        }

        var centerY = (int)Math.Round(
            lineMarks.Average(mark => mark.Y + mark.Height / 2d));
        return new(
            Normalize(new Point(left, centerY), imageSize),
            Normalize(new Point(right - 1, centerY), imageSize));
    }

    public NormalizedPoint? FindCorner(
        NormalizedPoint point,
        float maximumDistance)
    {
        var match = _corners
            .Select(candidate => new
            {
                Point = candidate,
                Distance = Distance(point, candidate),
            })
            .Where(value => value.Distance <= maximumDistance)
            .MinBy(value => value.Distance);
        return match?.Point;
    }

    public NormalizedRect? FindBox(
        NormalizedPoint point,
        float maximumDistance)
    {
        var match = _boxes
            .Select(box => new
            {
                Box = box,
                Distance = DistanceToBox(point, box),
            })
            .Where(value => value.Distance <= maximumDistance)
            .OrderBy(value => value.Distance)
            .ThenBy(value => Distance(point, Center(value.Box)))
            .FirstOrDefault();
        return match?.Box;
    }

    public SnapLineFeature? FindHorizontalLine(
        NormalizedPoint point,
        float maximumDistance)
    {
        var match = _horizontalLines
            .Select(line => new
            {
                Line = line,
                Distance = DistanceToLine(point, line),
            })
            .Where(value => value.Distance <= maximumDistance)
            .MinBy(value => value.Distance);
        return match?.Line;
    }

    private double DistanceToBox(
        NormalizedPoint point,
        NormalizedRect box)
    {
        var nearest = new NormalizedPoint(
            Math.Clamp(point.X, box.X, box.Right),
            Math.Clamp(point.Y, box.Y, box.Bottom));
        return Distance(point, nearest);
    }

    private double DistanceToLine(
        NormalizedPoint point,
        SnapLineFeature line)
    {
        var pointX = point.X * _width;
        var pointY = point.Y * _height;
        var startX = line.Start.X * _width;
        var startY = line.Start.Y * _height;
        var deltaX = (line.End.X - line.Start.X) * _width;
        var deltaY = (line.End.Y - line.Start.Y) * _height;
        var lengthSquared = (deltaX * deltaX) + (deltaY * deltaY);
        var amount = lengthSquared <= double.Epsilon
            ? 0
            : Math.Clamp(
                (((pointX - startX) * deltaX)
                    + ((pointY - startY) * deltaY))
                    / lengthSquared,
                0,
                1);
        var nearest = new NormalizedPoint(
            (float)((startX + (deltaX * amount)) / _width),
            (float)((startY + (deltaY * amount)) / _height));
        return Distance(point, nearest);
    }

    private double Distance(
        NormalizedPoint first,
        NormalizedPoint second)
    {
        var x = (first.X - second.X) * _width;
        var y = (first.Y - second.Y) * _height;
        return Math.Sqrt((x * x) + (y * y))
            / Math.Min(_width, _height);
    }

    private static NormalizedPoint Center(NormalizedRect rectangle) => new(
        rectangle.X + (rectangle.Width / 2),
        rectangle.Y + (rectangle.Height / 2));

    private static Mat Resize(Mat source)
    {
        var longest = Math.Max(source.Width, source.Height);
        if (longest <= MaximumAnalysisEdge)
        {
            return source.Clone();
        }

        var scale = MaximumAnalysisEdge / (double)longest;
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

    private static NormalizedPoint Normalize(Point point, Size size) =>
        new(
            point.X / (float)Math.Max(1, size.Width - 1),
            point.Y / (float)Math.Max(1, size.Height - 1));

    private static NormalizedPoint Normalize(Point2f point, Size size) =>
        new(
            point.X / Math.Max(1, size.Width - 1),
            point.Y / Math.Max(1, size.Height - 1));

    private static NormalizedRect Normalize(Rect rectangle, Size size) =>
        new(
            rectangle.X / (float)Math.Max(1, size.Width),
            rectangle.Y / (float)Math.Max(1, size.Height),
            rectangle.Width / (float)Math.Max(1, size.Width),
            rectangle.Height / (float)Math.Max(1, size.Height));
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
    private const int WindowSize = 4;
    private const int RequiredVotes = 2;
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
        TimeSpan.FromSeconds(2);

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
