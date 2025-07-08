#if IL2CPP
using System;
using System.Runtime.InteropServices;
using MonoMod.Core;
using MonoMod.Utils;

namespace DoorstopTests.Entrypoint;

internal sealed class DobbyDetourFactory(IDetourFactory fallback) : IDetourFactory
{
    public ICoreDetour CreateDetour(CreateDetourRequest request)
    {
        return fallback.CreateDetour(request);
    }

    public ICoreNativeDetour CreateNativeDetour(CreateNativeDetourRequest request)
    {
        var detour = new DobbyDetour(request.Source, request.Target);
        if (request.ApplyByDefault)
        {
            detour.Apply();
        }

        return detour;
    }

    public bool SupportsNativeDetourOrigEntrypoint => true;

    private sealed unsafe class DobbyDetour(IntPtr source, IntPtr target) : ICoreNativeDetour
    {
        public bool IsApplied { get; private set; }

        public IntPtr Source { get; } = source;
        public IntPtr Target { get; } = target;
        public IntPtr OrigEntrypoint { get; private set; }
        public bool HasOrigEntrypoint => OrigEntrypoint != IntPtr.Zero;

        public void Apply()
        {
            if (IsApplied)
                throw new InvalidOperationException("Cannot apply a detour which is already applied");

            void* orig;
            if (DobbyHook((void*) Source, (void*) Target, &orig) != 0)
            {
                throw new Exception();
            }

            OrigEntrypoint = (IntPtr) orig;

            IsApplied = true;
        }

        public void Undo()
        {
            if (IsApplied)
                return;

            if (DobbyDestroy((void*) Source) != 0)
            {
                throw new Exception();
            }

            IsApplied = false;
        }

        public void Dispose()
        {
            Undo();
        }

        [DllImport("dobby")]
        private static extern void dobby_enable_near_branch_trampoline();

        [DllImport("dobby")]
        private static extern int DobbyHook(void* address, void* replace_func, void** origin_func);

        [DllImport("dobby")]
        private static extern int DobbyDestroy(void* address);

        static DobbyDetour()
        {
            if (PlatformDetection.Architecture == ArchitectureKind.Arm64)
            {
                dobby_enable_near_branch_trampoline();
            }
        }
    }
}
#endif
