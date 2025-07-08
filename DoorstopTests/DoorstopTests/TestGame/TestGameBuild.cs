using System.IO.Compression;
using AssetRipper.Primitives;

namespace DoorstopTests.TestGame;

public sealed class TestGameBuild : DependencyArtifact
{
    public static string CommonDirectory { get; } = Path.Combine(Constants.BaseOutputPath, "TestGame");

    public override string InstallPath => Path.Combine(CommonDirectory, Id);
    public override string Hash => DownloadHash;

    public string ExecutablePath => Path.Combine(InstallPath, Target.Platform != Platform.MacOS
        ? "TestGame" + Target.Platform.ExecutableExtension
        : "TestGame.app/Contents/MacOS/TestGame");

    public required string Id { get; init; }
    public required UnityVersion UnityVersion { get; init; }
    public required TestGameTarget Target { get; init; }
    public required string DownloadUrl { get; init; }
    public required string DownloadHash { get; init; }

    protected override async Task DownloadAsync()
    {
        var archivePath = Path.Combine(InstallPath, Path.GetFileName(DownloadUrl));

        await DownloadFileAsync(DownloadUrl, archivePath);

        await ZipFile.ExtractToDirectoryAsync(archivePath, InstallPath, overwriteFiles: true);
        File.Delete(archivePath);

        try
        {
            // Save on some disk space
            Directory.Delete(Path.Combine(InstallPath, "TestGame_BackUpThisFolder_ButDontShipItWithYourGame"), true);
        }
        catch (IOException)
        {
        }

        if (!OperatingSystem.IsWindows() && Target.Platform != Platform.Windows)
        {
            File.SetUnixFileMode(ExecutablePath, File.GetUnixFileMode(ExecutablePath) | UnixFileMode.UserExecute | UnixFileMode.UserRead | UnixFileMode.UserWrite);
        }
    }
}
