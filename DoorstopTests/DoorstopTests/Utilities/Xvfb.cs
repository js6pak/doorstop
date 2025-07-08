using System.Diagnostics;

namespace DoorstopTests.Utilities;

internal static class Xvfb
{
    public const string DisplayId = "3785252";

    private static Process? s_xvfbProcess;

    [Before(TestSession)]
    public static async ValueTask Start()
    {
        if (!OperatingSystem.IsLinux()) return;

        var process = Process.Start(new ProcessStartInfo("Xvfb", [":" + DisplayId, "-screen", "0", "640x480x24", "-nolisten", "tcp"])
        {
            UseShellExecute = false,
            RedirectStandardError = true,
        }) ?? throw new InvalidOperationException("Failed to start Xvfb");

        try
        {
            using var cancellationTokenSource = new CancellationTokenSource(TimeSpan.FromSeconds(15));
            await process.WaitForExitAsync(cancellationTokenSource.Token);
        }
        catch (OperationCanceledException)
        {
        }

        if (process.HasExited)
        {
            var error = await process.StandardError.ReadToEndAsync();

            if (error.Contains("Server is already active"))
            {
                return;
            }

            throw new InvalidOperationException("Failed to start Xvfb: " + error);
        }

        if (!File.Exists($"/tmp/.X11-unix/X{DisplayId}"))
        {
            throw new InvalidOperationException("Xvfb did not create the X11 socket");
        }

        s_xvfbProcess = process;
    }

    [After(TestSession)]
    public static void Cleanup()
    {
        // TODO stop it?
        _ = s_xvfbProcess;
    }
}
