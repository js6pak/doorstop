namespace DoorstopTests.Utilities;

internal static class StringExtensions
{
    public static string? StripPrefix(this string text, string prefix)
    {
        return text.StartsWith(prefix) ? text[prefix.Length..] : null;
    }

    public static string? StripSuffix(this string text, string suffix)
    {
        return text.EndsWith(suffix) ? text[..^suffix.Length] : null;
    }

    public static (string, string)? SplitOnce(this string text, char delimiter)
    {
        var indexOf = text.IndexOf(delimiter);
        return indexOf == -1 ? null : (text[..indexOf], text[(indexOf + 1)..]);
    }
}
