using System.Runtime.InteropServices;
using BepInEx.GameTestFramework.Models;
using BepInEx.GameTestFramework.Unity;
using BepInEx.GameTestFramework.Unity.TestGame;
using BepInEx.GameTestFramework.Utilities;
using TUnit.Core.Helpers;

namespace DoorstopTests;

[ParallelLimiter<ProcessorCountParallelLimit>]
[RetryException(3)]
[Timeout(2 * 60 * 1000)]
public sealed partial class TestGameTests
{
    private async Task TestInjectionMethodAsync(UnityGameRunner runner, DoorstopInjectionMethod method, CancellationToken cancellationToken = default)
    {
        var result = await runner.LaunchAsync(
            new UnityGameRunner.LaunchOptions
            {
                TargetAssembly = Constants.EntrypointPaths[runner.RuntimeType],
                InjectionMethod = method,
            },
            info => info.Environment["DOORSTOPTESTS_ASSERT_RUNTIME"] = runner.RuntimeType == DotNetRuntimeType.IL2CPP ? "CoreCLR" : "Mono",
            cancellationToken
        );

        await Assert.That(result.ExitCode).IsEqualTo(0xAA);
    }

    public static IEnumerable<UnityGameRunner> LauncherTestGameBuilds
        // dll-syringe doesn't support win-arm64 yet
        => TestGameManager.Runners.Where(b => b is not { Platform: Platform.Windows, Architecture: Architecture.Arm64 });

    [Test]
    [MethodDataSource(nameof(LauncherTestGameBuilds))]
    public async Task Launcher(UnityGameRunner runner, CancellationToken cancellationToken = default)
    {
        await TestInjectionMethodAsync(runner, DoorstopInjectionMethod.Launcher, cancellationToken);
    }

    public static IEnumerable<UnityGameRunner> WindowsTestGameBuilds
        => TestGameManager.Runners.Where(b => b.Platform == Platform.Windows);

    [Test]
    [MethodDataSource(nameof(WindowsTestGameBuilds))]
    public async Task DllProxy(UnityGameRunner runner, CancellationToken cancellationToken = default)
    {
        await TestInjectionMethodAsync(runner, DoorstopInjectionMethod.DllProxy, cancellationToken);
    }

    public static IEnumerable<UnityGameRunner> PlayerTestGameBuilds
        => TestGameManager.Runners.Where(b => b.Platform switch
        {
            Platform.Windows => b.UnityVersion.GreaterThanOrEquals(2017, 2),
            // Platform.MacOS => b.UnityVersion.GreaterThanOrEquals(2018, 3),
            Platform.MacOS => false, // seems like properly implementing doorstop_player on macos will be impossible because of SIP
            Platform.Linux => b.UnityVersion.GreaterThanOrEquals(2019, 3),
            Platform.Android => false,
            _ => throw new ArgumentOutOfRangeException(),
        });

    [Test]
    [MethodDataSource(nameof(PlayerTestGameBuilds))]
    public async Task Player(UnityGameRunner runner, CancellationToken cancellationToken = default)
    {
        await TestInjectionMethodAsync(runner, DoorstopInjectionMethod.Player, cancellationToken);
    }
}
