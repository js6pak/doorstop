#if IL2CPP
using System;
using System.Diagnostics;
using System.Diagnostics.CodeAnalysis;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using MonoMod.Core;
using MonoMod.RuntimeDetour;
using MonoMod.Utils;

[assembly: DefaultDllImportSearchPaths(DllImportSearchPath.UserDirectories)]

namespace DoorstopTests.Entrypoint;

internal static unsafe partial class UnityEntrypoint
{
    private static partial void RuntimeSetup()
    {
        if (PlatformDetection.Architecture == ArchitectureKind.Arm64)
        {
            DetourFactory.SetCurrentFactory(current => new DobbyDetourFactory(current));
        }

        IntPtr gameAssemblyHandle;

        if (PlatformDetection.OS == OSKind.OSX)
        {
            gameAssemblyHandle = NativeLibrary.Load("@executable_path/../Frameworks/GameAssembly.dylib");
        }
        else
        {
            var gameAssemblyModule = Process.GetCurrentProcess().Modules
                .OfType<ProcessModule>()
                .Single(m => Path.GetFileNameWithoutExtension(m.ModuleName) == "GameAssembly");

            gameAssemblyHandle = NativeLibrary.Load(gameAssemblyModule.FileName);
        }

        NativeLibrary.SetDllImportResolver(
            typeof(LightApplication).Assembly,
            (libraryName, _, _) => libraryName == "GameAssembly" ? gameAssemblyHandle : IntPtr.Zero
        );

        var runtimeInvoke = NativeLibrary.GetExport(gameAssemblyHandle, "il2cpp_runtime_invoke");
        var methodGetName = (delegate* unmanaged<Il2CppMethod*, byte*>) NativeLibrary.GetExport(gameAssemblyHandle, "il2cpp_method_get_name");

        s_entrypointHook = new NativeHook(
            runtimeInvoke,
            Il2CppObject* (RuntimeInvokeDelegate orig, Il2CppMethod* method, void* obj, void** @params, Il2CppException** exc) =>
            {
                var methodName = Marshal.PtrToStringUTF8((IntPtr) methodGetName(method));
                if (methodName == "Internal_ActiveSceneChanged")
                {
                    OnUnityReady();
                }

                return orig(method, obj, @params, exc);
            }
        );

        Console.WriteLine("Unity version: " + LightApplication.unityVersion);
    }

    private struct Il2CppObject;

    private struct Il2CppException;

    private struct Il2CppMethod;

    [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
    private delegate Il2CppObject* RuntimeInvokeDelegate(Il2CppMethod* method, void* obj, void** @params, Il2CppException** exc);
}

[SuppressMessage("Style", "IDE1006:Naming Styles", Justification = "Mimics Unity's api")]
internal static partial class LightApplication
{
    public static unsafe string? unityVersion
    {
        get
        {
            var getUnityVersion = (delegate* unmanaged<Il2CppString*>) il2cpp_resolve_icall("UnityEngine.Application::get_unityVersion()");
            if (getUnityVersion != null)
            {
                return Il2CppString.GetString(getUnityVersion());
            }

            var getUnityVersionInjected = (delegate* unmanaged<ManagedSpanWrapper*, void>) il2cpp_resolve_icall("UnityEngine.Application::get_unityVersion_Injected(UnityEngine.Bindings.ManagedSpanWrapper&)");
            if (getUnityVersionInjected != null)
            {
                ManagedSpanWrapper wrapper = default;
                getUnityVersionInjected(&wrapper);
                return wrapper.GetStringAndDispose();
            }

            throw new NotSupportedException();
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    private readonly unsafe ref struct ManagedSpanWrapper
    {
        public readonly void* begin;
        public readonly int length;

        public string? GetStringAndDispose()
        {
            if (length == 0)
            {
                return begin == null ? null : string.Empty;
            }

            var outString = new string((char*) begin, 0, length);
            Free(begin);
            return outString;
        }

        private static void Free(void* ptr)
        {
            var free = (delegate* unmanaged<void*, void>) il2cpp_resolve_icall("UnityEngine.Bindings.BindingsAllocator::Free(System.Void*)");
            ArgumentNullException.ThrowIfNull(free);
            free(ptr);
        }
    }

    [LibraryImport("GameAssembly")]
    private static unsafe partial void* il2cpp_resolve_icall([MarshalAs(UnmanagedType.LPUTF8Str)] string? name);

    [LibraryImport("GameAssembly")]
    private static unsafe partial int il2cpp_string_length(Il2CppString* str);

    [LibraryImport("GameAssembly")]
    private static unsafe partial char* il2cpp_string_chars(Il2CppString* str);

    private struct Il2CppString
    {
        public static unsafe string? GetString(Il2CppString* value)
        {
            if (value == null) return null;
            var length = il2cpp_string_length(value);
            if (length == 0) return string.Empty;
            var chars = il2cpp_string_chars(value);
            return new string(chars, 0, length);
        }
    }
}
#endif
