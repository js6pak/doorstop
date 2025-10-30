using System.Net;
using System.Runtime.InteropServices;
using BepInEx.GameTestFramework.Models;
using BepInEx.GameTestFramework.Unity;
using BepInEx.GameTestFramework.Unity.TestGame;
using Mono.Debugging.Client;
using Mono.Debugging.Soft;

namespace DoorstopTests;

public sealed partial class TestGameTests
{
    // Setting breakpoints on macos-arm64 crashes before 2021.2 due to W^X
    public static IEnumerable<UnityGameRunner> MonoDebuggingTestGameBuilds
        => TestGameManager.Runners.Where(build =>
            build.RuntimeType == DotNetRuntimeType.Mono
            && (build.Platform != Platform.MacOS || build.Architecture != Architecture.Arm64 || build.UnityVersion.GreaterThanOrEquals(2021, 2)));

    [Test]
    [MethodDataSource(nameof(MonoDebuggingTestGameBuilds))]
    [Retry(3)] // sdb itself is quite flaky
    public async Task Debugging(UnityGameRunner runner, CancellationToken cancellationToken = default)
    {
        var breakpoints = new BreakpointStore
        {
            { "/_/DoorstopTests/DoorstopTests.Entrypoint/DebuggingTest.cs", 12 },
        };

        using var session = new MySoftDebuggerSession();
        session.Breakpoints = breakpoints;
        session.ExceptionHandler = ex =>
        {
            Console.WriteLine(ex);
            return false;
        };
        session.LogWriter = (isStdErr, text) =>
        {
            Console.WriteLine(text);
        };
        session.OutputWriter = (isStdErr, text) =>
        {
            Console.Write(text);
        };
        session.TargetEvent += (_, args) =>
        {
            Console.WriteLine(args.Type);
        };
        session.TargetHitBreakpoint += (_, args) =>
        {
            var frame = args.Backtrace.GetFrame(0);
            var localVariables = frame.GetLocalVariables();
            var local = localVariables.Single(v => v.Name == "local");
            local.WaitHandle.WaitOne();
            Console.WriteLine("local = " + (bool) local.GetRawValue());
            local.SetRawValue(false);
            Console.WriteLine("local = " + (bool) local.GetRawValue());

            session.Continue();
        };

        await session.DebugSymbolsManager.SetSymbolServerUrl([]);
        session.Run(
            new SoftDebuggerStartInfo(new SoftDebuggerListenArgs(string.Empty, IPAddress.Loopback, 0)
            {
                MaxConnectionAttempts = -1,
                TimeBetweenConnectionAttempts = 250,
            }),
            new DebuggerSessionOptions
            {
                EvaluationOptions = EvaluationOptions.DefaultOptions,
                ProjectAssembliesOnly = true,
                AutomaticSourceLinkDownload = AutomaticSourceDownload.Never,
            }
        );

        var port = await session.AssignedDebugPort;

        var result = await runner.LaunchAsync(
            new UnityGameRunner.LaunchOptions
            {
                TargetAssembly = Constants.EntrypointPaths[runner.RuntimeType],
            },
            info =>
            {
                var env = info.Environment;
                env["DOORSTOPTESTS_SCENARIO"] = "debugging";
                env["DOORSTOP_MONO_DEBUG_ENABLED"] = "true";
                env["DOORSTOP_MONO_DEBUG_CONNECT"] = "true";
                env["DOORSTOP_MONO_DEBUG_SUSPEND"] = "true";
                env["DOORSTOP_MONO_DEBUG_ADDRESS"] = "127.0.0.1:" + port;
            },
            cancellationToken
        );

        await Assert.That(result.ExitCode).IsEqualTo(0xAB);
    }
}

internal sealed class MySoftDebuggerSession : SoftDebuggerSession
{
    private readonly TaskCompletionSource<int> _assignedDebugPortSource = new();
    public Task<int> AssignedDebugPort => _assignedDebugPortSource.Task;

    protected override void OnRun(DebuggerStartInfo startInfo)
    {
        if (HasExited)
            throw new InvalidOperationException("Already exited");

        var dsi = (SoftDebuggerStartInfo) startInfo;
        if (dsi.StartArgs is SoftDebuggerListenArgs)
        {
            StartListening(dsi, out var assignedDebugPort);
            _assignedDebugPortSource.SetResult(assignedDebugPort);
        }
        else
        {
            base.OnRun(startInfo);
        }
    }
}
