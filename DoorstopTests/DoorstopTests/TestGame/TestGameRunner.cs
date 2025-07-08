using System.Diagnostics;
using AssetRipper.Primitives;
using DoorstopTests.Utilities;

namespace DoorstopTests.TestGame;

public sealed class TestGameRunner(TestGameBuild build)
{
    public TestGameBuild Build { get; } = build;

    public UnityVersion UnityVersion => Build.UnityVersion;
    public Platform Platform => Build.Target.Platform;
    public PlatformArchitecture Architecture { get; init; } = build.Target.PlatformArchitecture;
    public ScriptingImplementation ScriptingImplementation => Build.Target.ScriptingImplementation;

    public override string ToString()
    {
        var target = Build.Target;
        return $"{Build.UnityVersion}-{target.Platform.Serialize()}-{target.PlatformArchitecture.ToString().ToLowerInvariant()}-{target.ScriptingImplementation.ToString().ToLowerInvariant()}";
    }

    public sealed class TestGameLaunchOptions
    {
        public DoorstopInjectionMethod? InjectionMethod { get; init; }
        public required int ExpectedExitCode { get; init; }
        public Action<Dictionary<string, string?>>? ConfigureEnvironment { get; init; }
    }

    public enum DoorstopInjectionMethod
    {
        Launcher,
        DllProxy,
        Player,
    }

    public async Task LaunchAsync(TestGameLaunchOptions options, CancellationToken cancellationToken)
    {
        var injectionMethod = options.InjectionMethod
                              ?? (Platform == Platform.Windows ? DoorstopInjectionMethod.DllProxy : DoorstopInjectionMethod.Launcher);

        await Build.EnsureDownloadedAsync();

        var workingDirectory = Build.InstallPath;
        var arguments = new List<string> { Build.ExecutablePath };

        var environment = new Dictionary<string, string?>
        {
            ["RUST_BACKTRACE"] = "full",
            ["DOORSTOP_LOG_LEVEL"] = "trace",
            ["DOORSTOP_TARGET_ASSEMBLY"] = Constants.EntrypointPaths[ScriptingImplementation],
            ["DOORSTOPTESTS_ASSERT_RUNTIME"] = ScriptingImplementation == ScriptingImplementation.IL2CPP ? "CoreCLR" : "Mono",
            // ["DOORSTOPTESTS_SCENARIO"] = "throw",
        };

        var rustTarget =
            Architecture switch
            {
                PlatformArchitecture.X64 => "x86_64",
                PlatformArchitecture.X86 => "i686",
                PlatformArchitecture.Arm64 => "aarch64",
                PlatformArchitecture.Arm => "arm",
                PlatformArchitecture.X64X86 or PlatformArchitecture.X64Arm64 or PlatformArchitecture.Universal
                    => throw new InvalidOperationException("Universal architectures are invalid here, pick one"),
                _ => throw new UnreachableException(),
            } + "-" +
            Platform switch
            {
                Platform.Windows => "pc-windows-msvc",
                Platform.MacOS => "apple-darwin",
                Platform.Linux => "unknown-linux-gnu",
                Platform.Android => throw new PlatformNotSupportedException(),
                _ => throw new UnreachableException(),
            };

        var rustOutputPath = Path.Combine(Constants.DoorstopPath, "target", rustTarget, "release");

        if (injectionMethod == DoorstopInjectionMethod.Launcher)
        {
            var doorstopLauncherPath = Path.Combine(rustOutputPath, "doorstop_launcher" + Platform.ExecutableExtension);
            if (!File.Exists(doorstopLauncherPath))
            {
                throw new InvalidOperationException($"doorstop_launcher for {rustTarget} was not found (expected it at {doorstopLauncherPath})");
            }

            arguments.Insert(0, doorstopLauncherPath);
        }
        else if (injectionMethod == DoorstopInjectionMethod.Player)
        {
            var doorstopPlayerPath = Path.Combine(rustOutputPath, "doorstop_player" + Platform.ExecutableExtension);
            if (!File.Exists(doorstopPlayerPath))
            {
                throw new InvalidOperationException($"doorstop_player for {rustTarget} was not found (expected it at {doorstopPlayerPath})");
            }

            arguments[0] = doorstopPlayerPath;
        }
        else if (injectionMethod == DoorstopInjectionMethod.DllProxy)
        {
            if (Platform != Platform.Windows) throw new PlatformNotSupportedException();

            var doorstopDllProxyPath = Path.Combine(rustOutputPath, "doorstop.dll");
            if (!File.Exists(doorstopDllProxyPath))
            {
                throw new InvalidOperationException($"doorstop.dll for {rustTarget} was not found (expected it at {doorstopDllProxyPath})");
            }

            var dllProxyClone = Path.Combine(Build.InstallPath, "dll-proxy");

            if (!Directory.Exists(dllProxyClone))
            {
                var files = Directory.GetFileSystemEntries(Build.InstallPath);

                Directory.CreateDirectory(dllProxyClone);

                foreach (var file in files)
                {
                    var fileName = Path.GetFileName(file);
                    if (fileName.StartsWith('.') || fileName.EndsWith(".log")) continue;
                    File.CreateSymbolicLink(Path.Combine(dllProxyClone, fileName), file);
                }
            }

            arguments[0] = Path.Combine(dllProxyClone, Path.GetFileName(Build.ExecutablePath));
            workingDirectory = dllProxyClone;

            try
            {
                File.Copy(doorstopDllProxyPath, Path.Combine(dllProxyClone, "winhttp.dll"), overwrite: true);
            }
            catch (IOException e) when ((uint) e.HResult == /* ERROR_SHARING_VIOLATION */ 0x80070020)
            {
                Console.WriteLine("Failed to copy dll proxy because it was locked: " + e);
            }
        }

        if (Platform == Platform.Windows && !OperatingSystem.IsWindows())
        {
            var winePrefix = Path.Combine(Constants.BaseOutputPath, ".wine");

            environment.Add("WINEPREFIX", winePrefix);
            environment.Add("WINEDEBUG", "fixme-all");
            environment.Add("DXVK_LOG_LEVEL", "error");
            if (injectionMethod == DoorstopInjectionMethod.DllProxy)
            {
                environment.Add("WINEDLLOVERRIDES", "winhttp=n,b");
            }

            var userRegistryPath = Path.Combine(winePrefix, "user.reg");
            if (!File.Exists(userRegistryPath) || !(await File.ReadAllTextAsync(userRegistryPath, cancellationToken)).Contains("\"ShowCrashDialog\"=dword:00000000"))
            {
                await Process.Start(new ProcessStartInfo("wine", "reg add \"HKEY_CURRENT_USER\\SOFTWARE\\Wine\\WineDbg\" /v ShowCrashDialog /t REG_DWORD /d 0 /f")
                {
                    Environment =
                    {
                        ["WINEPREFIX"] = winePrefix,
                    },
                })!.WaitForExitAsync(cancellationToken);
            }

            arguments.Insert(0, "wine");
        }
        else if (Platform == Platform.Linux)
        {
            if (UnityVersion.LessThanOrEquals(5))
            {
                environment["PRESSURE_VESSEL_GENERATE_LOCALES"] = "0";

                await SteamLinuxRuntime.Instance.EnsureDownloadedAsync();
                arguments.InsertRange(0, SteamLinuxRuntime.Instance.RunScriptPath, "--");
            }

            // NixOS-specific hack
            if (Directory.Exists("/etc/nixos"))
            {
                arguments.InsertRange(0, "steam-run");
            }
        }

        arguments.AddRange("-logFile", "-");

        // Unity 4 and below: "-batchmode" command line argument is only available when publishing using Unity Pro.
        var noGraphics = UnityVersion.Major > 4;

        if (noGraphics)
        {
            arguments.AddRange("-batchmode", "-nographics");
            environment.Add("SDL_VIDEODRIVER", "dummy");
        }

        arguments.AddRange("-hidewindow", "1");

        if (OperatingSystem.IsLinux())
        {
            // X11 shouldn't be needed with -nographics most of the time, but some unity versions crash without it regardless, and it's needed for wine.
            environment["DISPLAY"] = ":" + Xvfb.DisplayId;
        }

        if (ScriptingImplementation == ScriptingImplementation.IL2CPP)
        {
            var dotNetRuntime = new DotNetRuntime(Platform, Architecture);
            await dotNetRuntime.EnsureDownloadedAsync();
            environment["DOORSTOP_CLR_CORLIB_DIR"] = dotNetRuntime.RuntimePath;
        }

        options.ConfigureEnvironment?.Invoke(environment);

        var processStartInfo = new ProcessStartInfo
        {
            WorkingDirectory = workingDirectory,
            FileName = arguments[0],

            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };

        foreach (var item in environment)
        {
            processStartInfo.Environment.Add(item);
        }

        foreach (var argument in arguments.Skip(1))
        {
            processStartInfo.ArgumentList.Add(argument);
        }

        Console.WriteLine($"Starting `{string.Join(' ', environment.Select(v => $"{v.Key}=\"{v.Value}\""))} {string.Join(' ', arguments)}` in '{processStartInfo.WorkingDirectory}'");

        using var process = new Process();
        process.StartInfo = processStartInfo;

        int? exitCode = null;
        string? retry = null;

        using var cancellationTokenSource = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);

        void ProcessOutput(string line)
        {
            if (line.Contains("err:sync:RtlpWaitForCriticalSection") && line.Contains("wait timed out"))
            {
                retry = "wine deadlock";
                cancellationTokenSource.Cancel();
            }
            else if (line.StartsWith("X Error of failed request:") ||
                     line.Contains("err:winediag:nodrv_CreateWindow") ||
                     line.Contains("/libX11.so.6(XCreateIC"))
            {
                retry = "X11 transient error";
                cancellationTokenSource.Cancel();
            }
        }

        process.OutputDataReceived += (_, e) =>
        {
            if (e.Data == null) return;
            Console.WriteLine(e.Data);

            if (e.Data.StripPrefix("Terminating with exit code ") is { } exitCodeRaw)
            {
                exitCode = int.Parse(exitCodeRaw);
                cancellationTokenSource.Cancel();
            }
            else
            {
                ProcessOutput(e.Data);
            }
        };
        process.ErrorDataReceived += (_, e) =>
        {
            if (e.Data == null) return;
            Console.WriteLine("stderr: " + e.Data);

            ProcessOutput(e.Data);
        };

        process.Start();

        process.BeginOutputReadLine();
        process.BeginErrorReadLine();

        cancellationTokenSource.CancelAfter(TimeSpan.FromMinutes(1));
        try
        {
            await process.WaitForExitAsync(cancellationTokenSource.Token);
        }
        catch (OperationCanceledException)
        {
            process.Kill(true);

            if (exitCode == null && retry == null)
            {
                Assert.Fail("Timed out");
                return;
            }
        }

        if (retry != null)
        {
            throw new RetryException(retry);
        }

        await Assert.That(exitCode ?? process.ExitCode).IsEqualTo(options.ExpectedExitCode);
    }
}
