using System.Diagnostics.CodeAnalysis;
using DoorstopTests.TestGame;
using DoorstopTests.Utilities;
using TUnit.Core.Helpers;

namespace DoorstopTests;

[SuppressMessage("Usage", "TUnit0046:Return a `Func<T>` rather than a `<T>`", Justification = "We *do* want TestGameRunner's to be reused, so this is a false positive")]
[ParallelLimiter<ProcessorCountParallelLimit>]
[RetryException.Attribute(3)]
[Timeout(2 * 60 * 1000)]
public sealed partial class TestGameTests
{
    [SuppressMessage("Design", "CA1024:Use properties where appropriate", Justification = "TUnit needs a method")]
    public static IEnumerable<TestGameRunner> GetTestGameBuilds()
    {
        return TestGameManager.Builds;
    }

    public static IEnumerable<TestGameRunner> GetLauncherTestGameBuilds()
    {
        // dll-syringe doesn't support win-arm64 yet
        return TestGameManager.Builds.Where(b => b is not { Platform: Platform.Windows, Architecture: PlatformArchitecture.Arm64 });
    }

    [Test]
    [MethodDataSource(nameof(GetLauncherTestGameBuilds))]
    public async Task Launcher(TestGameRunner build, CancellationToken cancellationToken = default)
    {
        await build.LaunchAsync(
            new TestGameRunner.TestGameLaunchOptions
            {
                InjectionMethod = TestGameRunner.DoorstopInjectionMethod.Launcher,
                ExpectedExitCode = 0xAA,
            },
            cancellationToken
        );
    }

    public static IEnumerable<TestGameRunner> GetWindowsTestGameBuilds()
    {
        return TestGameManager.Builds.Where(b => b.Platform == Platform.Windows);
    }

    [Test]
    [MethodDataSource(nameof(GetWindowsTestGameBuilds))]
    public async Task DllProxy(TestGameRunner build, CancellationToken cancellationToken = default)
    {
        await build.LaunchAsync(
            new TestGameRunner.TestGameLaunchOptions
            {
                InjectionMethod = TestGameRunner.DoorstopInjectionMethod.DllProxy,
                ExpectedExitCode = 0xAA,
            },
            cancellationToken
        );
    }

    public static IEnumerable<TestGameRunner> GetPlayerTestGameBuilds()
    {
        return TestGameManager.Builds.Where(b => b.Platform switch
        {
            Platform.Windows => b.UnityVersion.GreaterThanOrEquals(2017, 2),
            // Platform.MacOS => b.UnityVersion.GreaterThanOrEquals(2018, 3),
            Platform.MacOS => false, // seems like properly implementing doorstop_player on macos will be impossible because of SIP
            Platform.Linux => b.UnityVersion.GreaterThanOrEquals(2019, 3),
            Platform.Android => false,
            _ => throw new ArgumentOutOfRangeException(),
        });
    }

    [Test]
    [MethodDataSource(nameof(GetPlayerTestGameBuilds))]
    public async Task Player(TestGameRunner build, CancellationToken cancellationToken = default)
    {
        await build.LaunchAsync(
            new TestGameRunner.TestGameLaunchOptions
            {
                InjectionMethod = TestGameRunner.DoorstopInjectionMethod.Player,
                ExpectedExitCode = 0xAA,
            },
            cancellationToken
        );
    }
}
