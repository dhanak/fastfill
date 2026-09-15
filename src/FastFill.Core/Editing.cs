using System.Numerics;

namespace FastFill.Core;

public sealed class UndoBuffer<T>
{
    private readonly Func<T, T> _clone;
    private readonly int _capacity;
    private readonly Stack<T> _undo = [];
    private readonly Stack<T> _redo = [];

    public UndoBuffer(Func<T, T> clone, int capacity = 200)
    {
        ArgumentNullException.ThrowIfNull(clone);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(capacity);
        _clone = clone;
        _capacity = capacity;
    }

    public bool CanUndo => _undo.Count > 0;
    public bool CanRedo => _redo.Count > 0;

    public void Checkpoint(T current)
    {
        _undo.Push(_clone(current));
        _redo.Clear();
        TrimOldest(_undo);
    }

    public bool TryUndo(T current, out T result)
    {
        if (!_undo.TryPop(out var previous))
        {
            result = current;
            return false;
        }

        _redo.Push(_clone(current));
        result = previous;
        return true;
    }

    public bool TryRedo(T current, out T result)
    {
        if (!_redo.TryPop(out var next))
        {
            result = current;
            return false;
        }

        _undo.Push(_clone(current));
        result = next;
        return true;
    }

    public void Clear()
    {
        _undo.Clear();
        _redo.Clear();
    }

    private void TrimOldest(Stack<T> stack)
    {
        if (stack.Count <= _capacity)
        {
            return;
        }

        var values = stack.Take(_capacity).Reverse().ToArray();
        stack.Clear();
        foreach (var value in values)
        {
            stack.Push(value);
        }
    }
}

public static class ViewportNavigation
{
    public static Vector2 PinchPan(
        Vector2 viewportSize,
        Vector2 startCenter,
        Vector2 currentCenter,
        Vector2 startPan,
        float startZoom,
        float currentZoom)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(startZoom);
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(currentZoom);
        var viewportCenter = viewportSize / 2;
        var focus = (startCenter - viewportCenter - startPan) / startZoom;
        return currentCenter - viewportCenter - focus * currentZoom;
    }

    public static Vector2 ClampPan(
        Vector2 pan,
        Vector2 viewportSize,
        Vector2 fullViewSize,
        float zoom)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(zoom);
        if (zoom <= 1)
        {
            return Vector2.Zero;
        }

        var maximum = Vector2.Max(
            Vector2.Zero,
            (fullViewSize * zoom - viewportSize) / 2);
        return Vector2.Clamp(pan, -maximum, maximum);
    }
}

public static class AnnotationHitTester
{
    public static Annotation? HitTest(
        IReadOnlyList<Annotation> annotations,
        NormalizedPoint pagePoint,
        float tolerance = 0.015f)
    {
        for (var index = annotations.Count - 1; index >= 0; index--)
        {
            var annotation = annotations[index];
            if (!Matrix3x2.Invert(annotation.Transform.Matrix, out var inverse))
            {
                continue;
            }

            var local = NormalizedPoint.From(
                Vector2.Transform(pagePoint.Vector, inverse));
            if (Contains(annotation, local, tolerance))
            {
                return annotation;
            }
        }

        return null;
    }

    public static bool BoundsContain(
        Annotation annotation,
        NormalizedPoint pagePoint,
        float tolerance = 0)
    {
        if (!Matrix3x2.Invert(annotation.Transform.Matrix, out var inverse))
        {
            return false;
        }

        var local = NormalizedPoint.From(
            Vector2.Transform(pagePoint.Vector, inverse));
        return AnnotationGeometry.Bounds(annotation).Contains(
            local,
            tolerance);
    }

    private static bool Contains(
        Annotation annotation,
        NormalizedPoint point,
        float tolerance) => annotation switch
    {
        FreehandAnnotation { Filled: true } ink =>
            PolygonContains(ink.Points, point)
            || PolylineDistance(ink.Points, point, close: true) <= tolerance,
        FreehandAnnotation ink =>
            PolylineDistance(ink.Points, point) <= tolerance,
        TextAnnotation text => text.Bounds.Contains(point, tolerance),
        ShapeAnnotation shape => Contains(shape, point, tolerance),
        _ => false,
    };

    private static bool Contains(
        ShapeAnnotation shape,
        NormalizedPoint point,
        float tolerance)
    {
        var bounds = NormalizedRect.FromPoints(shape.Start, shape.End);
        return shape.Shape switch
        {
            ShapeKind.Line or ShapeKind.Arrow =>
                SegmentDistance(point, shape.Start, shape.End) <= tolerance,
            ShapeKind.Rectangle when shape.Filled =>
                bounds.Contains(point, tolerance),
            ShapeKind.Rectangle => RectangleEdgeDistance(bounds, point)
                <= tolerance,
            ShapeKind.Ellipse => EllipseContains(
                bounds,
                point,
                tolerance,
                shape.Filled),
            ShapeKind.Checkmark => CheckmarkDistance(bounds, point)
                <= tolerance,
            ShapeKind.Cross => CrossDistance(bounds, point) <= tolerance,
            _ => false,
        };
    }

    private static float PolylineDistance(
        List<NormalizedPoint> points,
        NormalizedPoint point,
        bool close = false)
    {
        if (points.Count == 0)
        {
            return float.PositiveInfinity;
        }

        if (points.Count == 1)
        {
            return points[0].DistanceTo(point);
        }

        var distance = float.PositiveInfinity;
        for (var index = 1; index < points.Count; index++)
        {
            distance = Math.Min(
                distance,
                SegmentDistance(point, points[index - 1], points[index]));
        }

        if (close)
        {
            distance = Math.Min(
                distance,
                SegmentDistance(point, points[^1], points[0]));
        }

        return distance;
    }

    private static bool PolygonContains(
        List<NormalizedPoint> points,
        NormalizedPoint point)
    {
        if (points.Count < 3)
        {
            return false;
        }

        var inside = false;
        for (var index = 0; index < points.Count; index++)
        {
            var current = points[index];
            var previous = points[(index + points.Count - 1) % points.Count];
            if ((current.Y > point.Y) == (previous.Y > point.Y))
            {
                continue;
            }

            var crossing = (previous.X - current.X)
                * (point.Y - current.Y)
                / (previous.Y - current.Y)
                + current.X;
            if (point.X < crossing)
            {
                inside = !inside;
            }
        }

        return inside;
    }

    private static float SegmentDistance(
        NormalizedPoint point,
        NormalizedPoint start,
        NormalizedPoint end)
    {
        var segment = end.Vector - start.Vector;
        var lengthSquared = segment.LengthSquared();
        if (lengthSquared == 0)
        {
            return point.DistanceTo(start);
        }

        var offset = point.Vector - start.Vector;
        var fraction = Math.Clamp(
            Vector2.Dot(offset, segment) / lengthSquared,
            0,
            1);
        return Vector2.Distance(
            point.Vector,
            start.Vector + segment * fraction);
    }

    private static float RectangleEdgeDistance(
        NormalizedRect bounds,
        NormalizedPoint point)
    {
        if (!bounds.Contains(point))
        {
            return float.PositiveInfinity;
        }

        return new[]
        {
            Math.Abs(point.X - bounds.X),
            Math.Abs(point.X - bounds.Right),
            Math.Abs(point.Y - bounds.Y),
            Math.Abs(point.Y - bounds.Bottom),
        }.Min();
    }

    private static bool EllipseContains(
        NormalizedRect bounds,
        NormalizedPoint point,
        float tolerance,
        bool filled)
    {
        var radiusX = Math.Max(bounds.Width / 2, float.Epsilon);
        var radiusY = Math.Max(bounds.Height / 2, float.Epsilon);
        var centerX = bounds.X + radiusX;
        var centerY = bounds.Y + radiusY;
        var normalizedX = (point.X - centerX) / radiusX;
        var normalizedY = (point.Y - centerY) / radiusY;
        var radius = MathF.Sqrt(
            normalizedX * normalizedX + normalizedY * normalizedY);
        if (filled)
        {
            return radius <= 1 + (tolerance / Math.Min(radiusX, radiusY));
        }

        var normalizedTolerance = tolerance / Math.Min(radiusX, radiusY);
        return Math.Abs(radius - 1) <= normalizedTolerance;
    }

    private static float CheckmarkDistance(
        NormalizedRect bounds,
        NormalizedPoint point)
    {
        var start = new NormalizedPoint(
            bounds.X,
            bounds.Y + bounds.Height * 0.55f);
        var middle = new NormalizedPoint(
            bounds.X + bounds.Width * 0.38f,
            bounds.Bottom);
        var end = new NormalizedPoint(bounds.Right, bounds.Y);
        return Math.Min(
            SegmentDistance(point, start, middle),
            SegmentDistance(point, middle, end));
    }

    private static float CrossDistance(
        NormalizedRect bounds,
        NormalizedPoint point) => Math.Min(
        SegmentDistance(
            point,
            new NormalizedPoint(bounds.X, bounds.Y),
            new NormalizedPoint(bounds.Right, bounds.Bottom)),
        SegmentDistance(
            point,
            new NormalizedPoint(bounds.Right, bounds.Y),
            new NormalizedPoint(bounds.X, bounds.Bottom)));
}

public static class AnnotationGeometry
{
    public static bool SupportsRotation(Annotation annotation) =>
        annotation is not ShapeAnnotation
        {
            Shape: ShapeKind.Line or ShapeKind.Arrow,
        };

    public static NormalizedRect Bounds(Annotation annotation) =>
        annotation switch
        {
            FreehandAnnotation ink when ink.Points.Count > 0 => new(
                ink.Points.Min(point => point.X),
                ink.Points.Min(point => point.Y),
                ink.Points.Max(point => point.X)
                    - ink.Points.Min(point => point.X),
                ink.Points.Max(point => point.Y)
                    - ink.Points.Min(point => point.Y)),
            TextAnnotation text => text.Bounds,
            ShapeAnnotation shape => NormalizedRect.FromPoints(
                shape.Start,
                shape.End),
            _ => new(0, 0, 0, 0),
        };

    public static NormalizedPoint[] BoundsCorners(Annotation annotation) =>
        BoundsCorners(Bounds(annotation));

    public static NormalizedPoint[] ResizeHandles(Annotation annotation) =>
        annotation switch
        {
            TextAnnotation => [],
            ShapeAnnotation
            {
                Shape: ShapeKind.Line or ShapeKind.Arrow,
            } line => [line.Start, line.End],
            _ => BoundsCorners(annotation),
        };

    public static NormalizedPoint[] BoundsCorners(NormalizedRect bounds) =>
    [
        new(bounds.X, bounds.Y),
        new(bounds.Right, bounds.Y),
        new(bounds.Right, bounds.Bottom),
        new(bounds.X, bounds.Bottom),
    ];

    public static NormalizedRect TransformedBounds(Annotation annotation)
    {
        var corners = BoundsCorners(annotation)
            .Select(annotation.Transform.Apply)
            .ToArray();
        return new NormalizedRect(
            corners.Min(point => point.X),
            corners.Min(point => point.Y),
            corners.Max(point => point.X) - corners.Min(point => point.X),
            corners.Max(point => point.Y) - corners.Min(point => point.Y));
    }

    public static bool Intersects(
        Annotation annotation,
        NormalizedRect rectangle)
    {
        var bounds = TransformedBounds(annotation);
        return bounds.X <= rectangle.Right
            && bounds.Right >= rectangle.X
            && bounds.Y <= rectangle.Bottom
            && bounds.Bottom >= rectangle.Y;
    }

    public static Annotation ResizeFromCorner(
        Annotation annotation,
        int cornerIndex,
        NormalizedPoint newCorner,
        float pageWidth,
        float pageHeight)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(pageWidth, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(pageHeight, 1);
        var handles = ResizeHandles(annotation);
        ArgumentOutOfRangeException.ThrowIfNegative(cornerIndex);
        ArgumentOutOfRangeException.ThrowIfGreaterThanOrEqual(
            cornerIndex,
            handles.Length);

        if (annotation is ShapeAnnotation
            {
                Shape: ShapeKind.Line or ShapeKind.Arrow,
            } line)
        {
            return ResizeLine(line, handles[cornerIndex], newCorner);
        }

        var corners = BoundsCorners(annotation);
        var opposite = corners[(cornerIndex + 2) % corners.Length];
        if (annotation is ShapeAnnotation
            {
                Shape: ShapeKind.Checkmark or ShapeKind.Cross,
            })
        {
            newCorner = SquareCorner(
                opposite,
                newCorner,
                pageWidth,
                pageHeight);
        }

        return ResizeToBounds(
            annotation,
            NormalizedRect.FromPoints(opposite, newCorner));
    }

    public static Annotation ResizeByFactor(
        Annotation annotation,
        float factor)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(factor);
        if (annotation is TextAnnotation)
        {
            return annotation;
        }

        var bounds = Bounds(annotation);
        var centerX = bounds.X + bounds.Width / 2;
        var centerY = bounds.Y + bounds.Height / 2;
        return ResizeToBounds(
            annotation,
            new NormalizedRect(
                centerX - bounds.Width * factor / 2,
                centerY - bounds.Height * factor / 2,
                bounds.Width * factor,
                bounds.Height * factor));
    }

    public static Matrix3x2 PageRotation(
        float radians,
        Vector2 normalizedCenter,
        float pageWidth,
        float pageHeight)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(pageWidth, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(pageHeight, 1);
        var pageCenter = new Vector2(
            normalizedCenter.X * pageWidth,
            normalizedCenter.Y * pageHeight);
        return Matrix3x2.CreateScale(pageWidth, pageHeight)
            * Matrix3x2.CreateTranslation(-pageCenter)
            * Matrix3x2.CreateRotation(radians)
            * Matrix3x2.CreateTranslation(pageCenter)
            * Matrix3x2.CreateScale(1 / pageWidth, 1 / pageHeight);
    }

    public static float PageRotationRadians(
        AffineTransform transform,
        float pageWidth,
        float pageHeight)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(pageWidth, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(pageHeight, 1);
        var origin = Vector2.Transform(
            Vector2.Zero,
            transform.Matrix);
        var horizontal = Vector2.Transform(
            new Vector2(1 / pageWidth, 0),
            transform.Matrix);
        return MathF.Atan2(
            (horizontal.Y - origin.Y) * pageHeight,
            (horizontal.X - origin.X) * pageWidth);
    }

    private static Annotation ResizeToBounds(
        Annotation annotation,
        NormalizedRect target)
    {
        var source = Bounds(annotation);
        return annotation switch
        {
            FreehandAnnotation ink => ink with
            {
                Points = ink.Points
                    .Select(point => Remap(point, source, target))
                    .ToList(),
            },
            TextAnnotation text => text with { Bounds = target },
            ShapeAnnotation shape => shape with
            {
                Start = Remap(shape.Start, source, target),
                End = Remap(shape.End, source, target),
            },
            _ => annotation,
        };
    }

    private static ShapeAnnotation ResizeLine(
        ShapeAnnotation line,
        NormalizedPoint draggedCorner,
        NormalizedPoint newCorner)
    {
        if (draggedCorner.DistanceTo(line.Start)
            <= draggedCorner.DistanceTo(line.End))
        {
            return line with { Start = newCorner };
        }

        return line with { End = newCorner };
    }

    private static NormalizedPoint Remap(
        NormalizedPoint point,
        NormalizedRect source,
        NormalizedRect target) => new(
        Remap(point.X, source.X, source.Width, target.X, target.Width),
        Remap(point.Y, source.Y, source.Height, target.Y, target.Height));

    private static float Remap(
        float value,
        float sourceStart,
        float sourceLength,
        float targetStart,
        float targetLength) => sourceLength <= float.Epsilon
        ? targetStart + targetLength / 2
        : targetStart
            + (value - sourceStart) / sourceLength * targetLength;

    private static NormalizedPoint SquareCorner(
        NormalizedPoint anchor,
        NormalizedPoint corner,
        float pageWidth,
        float pageHeight)
    {
        var deltaX = (corner.X - anchor.X) * pageWidth;
        var deltaY = (corner.Y - anchor.Y) * pageHeight;
        var size = Math.Max(Math.Abs(deltaX), Math.Abs(deltaY));
        var directionX = deltaX < 0 ? -1 : 1;
        var directionY = deltaY < 0 ? -1 : 1;
        return new NormalizedPoint(
            anchor.X + directionX * size / pageWidth,
            anchor.Y + directionY * size / pageHeight);
    }
}

public static class AnnotationOrdering
{
    public static bool SendToBack(
        List<Annotation> annotations,
        IReadOnlySet<Guid> selectedIds) =>
        MoveToEdge(annotations, selectedIds, toFront: false);

    public static bool BringToFront(
        List<Annotation> annotations,
        IReadOnlySet<Guid> selectedIds) =>
        MoveToEdge(annotations, selectedIds, toFront: true);

    private static bool MoveToEdge(
        List<Annotation> annotations,
        IReadOnlySet<Guid> selectedIds,
        bool toFront)
    {
        ArgumentNullException.ThrowIfNull(annotations);
        ArgumentNullException.ThrowIfNull(selectedIds);
        var selected = annotations
            .Where(annotation => selectedIds.Contains(annotation.Id))
            .ToList();
        if (selected.Count == 0)
        {
            return false;
        }

        var unselected = annotations
            .Where(annotation => !selectedIds.Contains(annotation.Id));
        var reordered = (toFront
                ? unselected.Concat(selected)
                : selected.Concat(unselected))
            .ToList();
        if (reordered.Select(annotation => annotation.Id).SequenceEqual(
            annotations.Select(annotation => annotation.Id)))
        {
            return false;
        }

        annotations.Clear();
        annotations.AddRange(reordered);
        return true;
    }
}
