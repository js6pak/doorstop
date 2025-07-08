using System.Formats.Tar;
using SharpCompress.Compressors.Xz;

namespace DoorstopTests.TestGame;

internal sealed class SteamLinuxRuntime : DependencyArtifact
{
    public static SteamLinuxRuntime Instance { get; } = new();

    public override string InstallPath => Path.Combine(Constants.BaseOutputPath, "SteamLinuxRuntime");
    public override string Hash => string.Empty;

    public string RunScriptPath => Path.Combine(InstallPath, "SteamLinuxRuntime_sniper", "run");

    private string DownloadUrl => "https://repo.steampowered.com/steamrt3/images/latest-public-stable/SteamLinuxRuntime_sniper.tar.xz";

    protected override async Task DownloadAsync()
    {
        var archivePath = Path.Combine(InstallPath, Path.GetFileName(DownloadUrl));

        await DownloadFileAsync(DownloadUrl, archivePath);

        await using var stream = new XZStream(File.OpenRead(archivePath));
        await TarFile.ExtractToDirectoryAsync(stream, InstallPath, overwriteFiles: true);

        File.Delete(archivePath);
    }
}
