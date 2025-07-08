using System;
using System.Diagnostics.CodeAnalysis;
using MonoMod.Utils;

namespace DoorstopTests.Entrypoint;

internal static partial class UnityEntrypoint
{
    [SuppressMessage("Performance", "CA1859:Use concrete types when possible for improved performance", Justification = "Different types based on platform")]
    private static IDisposable? s_entrypointHook;

    public static void Setup()
    {
        // MonoMod doesn't support these platforms
        if ((PlatformDetection.OS, PlatformDetection.Architecture)
            is (OSKind.Linux, ArchitectureKind.x86)
            or (OSKind.Windows, ArchitectureKind.Arm64))
        {
            FinalPoint.ExitSuccess();
            return;
        }

        RuntimeSetup();
    }

    private static partial void RuntimeSetup();

    private static void OnUnityReady()
    {
        Console.WriteLine("OnUnityReady");

        if (s_entrypointHook == null) throw new InvalidOperationException();
        s_entrypointHook.Dispose();
        s_entrypointHook = null;

        FinalPoint.Run();
    }
}
