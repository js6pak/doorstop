using System.Diagnostics;

namespace DoorstopTests.TestGame;

public enum Platform
{
    Windows,
    MacOS,
    Linux,
    Android,
}

internal static class PlatformExtensions
{
    extension(Platform @this)
    {
        public static Platform Deserialize(ReadOnlySpan<char> value)
        {
            return value switch
            {
                "win" => Platform.Windows,
                "osx" => Platform.MacOS,
                "linux" => Platform.Linux,
                "android" => Platform.Android,
                _ => throw new ArgumentException($"Unknown platform: {value}"),
            };
        }

        public string Serialize()
        {
            return @this switch
            {
                Platform.Windows => "win",
                Platform.MacOS => "osx",
                Platform.Linux => "linux",
                Platform.Android => "android",
                _ => throw new UnreachableException(),
            };
        }

        public string ExecutableExtension => @this switch
        {
            Platform.Windows => ".exe",
            _ => string.Empty,
        };
    }
}
