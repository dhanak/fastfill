using OpenCvSharp;

namespace FastFill.Core;

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
        using var transform = Cv2.GetPerspectiveTransform(
            points,
            destination);
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

    private static Mat ApplyFilter(
        Mat source,
        PageFilterSettings settings)
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
