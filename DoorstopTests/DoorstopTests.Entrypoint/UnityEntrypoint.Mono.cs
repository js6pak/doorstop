#if MONO
using System;
using System.IO;
using System.Linq;
using System.Reflection;
using MonoMod.Cil;
using MonoMod.RuntimeDetour;
using MonoMod.Utils;

namespace DoorstopTests.Entrypoint;

internal partial class UnityEntrypoint
{
    private static partial void RuntimeSetup()
    {
        if (AppDomain.CurrentDomain.GetAssemblies().SingleOrDefault(a => a.GetName().Name is "UnityEngine.CoreModule" or "UnityEngine") is { } assembly)
        {
            HookUnityEngine(assembly);
        }
        else
        {
            AppDomain.CurrentDomain.AssemblyLoad += OnAssemblyLoad;
        }

        foreach (var name in new[] { nameof(Console.SetOut), nameof(Console.SetError) })
        {
            var hook = new Hook(
                typeof(Console).GetMethod(name, BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Static)!,
                void (TextWriter _) =>
                {
                    Console.WriteLine($"Preventing {nameof(Console)}.{name}");
                }
            );
            GC.SuppressFinalize(hook);
        }
    }

    private static void OnAssemblyLoad(object? sender, AssemblyLoadEventArgs args)
    {
        var assembly = args.LoadedAssembly;
        var assemblyName = assembly.GetName().Name;

        Console.WriteLine($"OnAssemblyLoad: {assemblyName}");

        if (assemblyName is "UnityEngine.CoreModule" or "UnityEngine")
        {
            AppDomain.CurrentDomain.AssemblyLoad -= OnAssemblyLoad;

            HookUnityEngine(assembly);
        }
    }

    private static void HookUnityEngine(Assembly assembly)
    {
        try
        {
            bool TryHook(string typeName, string methodName, bool tail = false)
            {
                var type = assembly.GetType(typeName, false);
                var method = type?.GetMethod(methodName, BindingFlags.NonPublic | BindingFlags.Public | BindingFlags.Static);
                if (method != null)
                {
                    Console.WriteLine("Hooking " + method);
                    s_entrypointHook = new ILHook(method, il =>
                    {
                        var cursor = new ILCursor(il);

                        if (tail)
                        {
                            cursor.Goto(-1);
                        }

                        cursor.EmitDelegate(OnUnityReady);
                    });

                    return true;
                }

                return false;
            }

            if (TryHook("UnityEngine.SceneManagement.SceneManager", "Internal_ActiveSceneChanged"))
            {
                return;
            }

            var isNoGraphics = Environment.GetCommandLineArgs().Contains("-nographics");
            if (PlatformDetection.OS.Is(OSKind.Windows) && !isNoGraphics)
            {
                if (TryHook("UnityEngine.Display", "RecreateDisplayList", tail: true))
                {
                    return;
                }
            }

            if (TryHook("UnityEngine.Font", "InvokeTextureRebuilt_Internal"))
            {
                return;
            }

            Console.WriteLine("Couldn't find any entrypoint");
            Utilities.Terminate(1);
        }
        catch (Exception e)
        {
            Console.WriteLine("Failed to hook entrypoint");
            Console.WriteLine(e);
            Utilities.Terminate(1);
        }
    }
}
#endif
