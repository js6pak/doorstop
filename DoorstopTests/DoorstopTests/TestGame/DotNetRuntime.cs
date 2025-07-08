using System.Diagnostics;
using System.Formats.Tar;
using System.IO.Compression;

namespace DoorstopTests.TestGame;

internal sealed class DotNetRuntime(Platform Platform, PlatformArchitecture PlatformArchitecture) : DependencyArtifact
{
    public const string Version = "9.0.9";

    public static string CommonDirectory { get; } = Path.Combine(Constants.BaseOutputPath, "dotnet");

    public string Rid => Platform switch
    {
        Platform.Windows => "win",
        Platform.MacOS => "osx",
        Platform.Linux => "linux",
        Platform.Android => "android",
        _ => throw new UnreachableException(),
    } + "-" + PlatformArchitecture switch
    {
        PlatformArchitecture.X64 => "x64",
        PlatformArchitecture.X86 => "x86",
        PlatformArchitecture.Arm => "arm",
        PlatformArchitecture.Arm64 => "arm64",
        PlatformArchitecture.X64X86 or PlatformArchitecture.X64Arm64 or PlatformArchitecture.Universal
            => throw new InvalidOperationException("Universal architectures are invalid here, pick one"),
        _ => throw new UnreachableException(),
    };

    public override string InstallPath => Path.Combine(CommonDirectory, Rid);
    public override string Hash => Version;

    public string RuntimePath => Path.Combine(InstallPath, "shared", "Microsoft.NETCore.App", Version);

    private string DownloadUrl => $"https://builds.dotnet.microsoft.com/dotnet/Runtime/{Version}/dotnet-runtime-{Version}-{Rid}." + Platform switch
    {
        Platform.Windows => "zip",
        Platform.Linux or Platform.MacOS => "tar.gz",
        Platform.Android => throw new NotSupportedException(),
        _ => throw new UnreachableException(),
    };

    protected override async Task DownloadAsync()
    {
        var archivePath = Path.Combine(InstallPath, Path.GetFileName(DownloadUrl));

        await DownloadFileAsync(DownloadUrl, archivePath);

        if (Platform == Platform.Windows)
        {
            await ZipFile.ExtractToDirectoryAsync(archivePath, InstallPath, overwriteFiles: true);
        }
        else
        {
            await using var stream = new GZipStream(File.OpenRead(archivePath), CompressionMode.Decompress);
            await TarFile.ExtractToDirectoryAsync(stream, InstallPath, overwriteFiles: true);
        }

        File.Delete(archivePath);
    }
}
