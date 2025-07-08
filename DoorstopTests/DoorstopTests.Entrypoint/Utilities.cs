using System;
using System.Runtime.InteropServices;

namespace DoorstopTests.Entrypoint;

internal static class Utilities
{
    public static void Terminate(byte exitCode)
    {
        Console.WriteLine("Terminating with exit code " + exitCode);

        if (Environment.OSVersion.Platform == PlatformID.Unix)
        {
            Inner();

            void Inner()
            {
                _exit(exitCode);

                [DllImport("libc")]
                static extern void _exit(int status);
            }
        }
        else
        {
            Inner();

            void Inner()
            {
                TerminateProcess(GetCurrentProcess(), exitCode);

                [DllImport("kernel32")]
                static extern IntPtr GetCurrentProcess();

                [DllImport("kernel32")]
                static extern bool TerminateProcess(IntPtr hProcess, uint uExitCode);
            }
        }

        throw new InvalidOperationException("Terminate failed");
    }
}
