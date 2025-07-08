using AssetRipper.Primitives;
using DoorstopTests.Utilities;
using GitHub;
using GitHub.Octokit.Client;
using GitHub.Octokit.Client.Authentication;
using Microsoft.Kiota.Abstractions.Authentication;
using Microsoft.Kiota.Bundle;
using Microsoft.Kiota.Http.HttpClientLibrary;

namespace DoorstopTests.TestGame;

public static class TestGameManager
{
    public static IEnumerable<TestGameRunner> Builds { get; private set; } = null!;

    [Before(TestDiscovery)]
    public static async Task Setup()
    {
        IAuthenticationProvider authenticationProvider = Environment.GetEnvironmentVariable("GITHUB_TOKEN") is { } githubToken
            ? new TokenAuthProvider(new TokenProvider(githubToken))
            : new AnonymousAuthenticationProvider();

        using var httpClient = KiotaClientFactory.Create(ClientFactory.CreateDefaultHandlers().Append(new DiskHttpCacheHandler()).ToList());
        using var adapter = new DefaultRequestAdapter(authenticationProvider, httpClient: httpClient);
        var client = new GitHubClient(adapter);

        var release = await client.Repos["js6pak"]["TestGame"].Releases.Latest.GetAsync();
        ArgumentNullException.ThrowIfNull(release);
        ArgumentNullException.ThrowIfNull(release.Assets);

        var builds = new List<TestGameRunner>();

        foreach (var asset in release.Assets)
        {
            if (asset.Name?.StripSuffix(".zip") is not { } assetName) continue;

            var digest = (string) asset.AdditionalData["digest"];

            var (versionRaw, targetRaw) = assetName.SplitOnce('-')
                                          ?? throw new ArgumentException($"Invalid asset name: {assetName}");

            var version = UnityVersion.Parse(versionRaw);
            var target = TestGameTarget.Parse(targetRaw);

            var build = new TestGameBuild
            {
                Id = assetName,
                UnityVersion = version,
                Target = target,
                DownloadUrl = asset.BrowserDownloadUrl ?? throw new InvalidOperationException($"Asset {assetName} has no download URL"),
                DownloadHash = digest,
            };

            if (!build.Target.CanRun())
            {
                continue;
            }

            // Versions before 4.6 were net20, and that's too much pain
            if (!build.UnityVersion.GreaterThanOrEquals(4, 6))
            {
                continue;
            }

            if (target.PlatformArchitecture == PlatformArchitecture.X64Arm64)
            {
                if (!OperatingSystem.IsMacOS()) throw new NotImplementedException();
                builds.Add(new TestGameRunner(build) { Architecture = PlatformArchitecture.X64 });
                builds.Add(new TestGameRunner(build) { Architecture = PlatformArchitecture.Arm64 });
                continue;
            }

            if (target.PlatformArchitecture == PlatformArchitecture.X64X86)
            {
                if (!OperatingSystem.IsMacOS()) throw new NotImplementedException();
                builds.Add(new TestGameRunner(build) { Architecture = PlatformArchitecture.X64 });
                continue;
            }

            builds.Add(new TestGameRunner(build));
        }

        Builds = builds;
    }
}
