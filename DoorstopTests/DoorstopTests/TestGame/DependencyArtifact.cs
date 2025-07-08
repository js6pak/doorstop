using System.Diagnostics;

namespace DoorstopTests.TestGame;

public abstract class DependencyArtifact
{
    private static readonly SemaphoreSlim s_lock = new(1, 1);

    public abstract string InstallPath { get; }

    public abstract string Hash { get; }

    private string HashPath => Path.Combine(InstallPath, ".hash");

    private bool IsUpToDate()
    {
        return File.Exists(HashPath) && File.ReadAllText(HashPath) == Hash;
    }

    private static async Task<FileStream> LockFileAsync(string path)
    {
        var startTime = Stopwatch.GetTimestamp();
        var firstTry = true;

        while (true)
        {
            try
            {
                return new FileStream(path, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
            }
            catch (IOException e)
            {
                if (Stopwatch.GetElapsedTime(startTime) > TimeSpan.FromMinutes(5))
                {
                    throw new Exception($"Timed out waiting for lock on {path}", e);
                }

                if (firstTry) Console.WriteLine($"Waiting for lock on {path}");
                firstTry = false;

                await Task.Delay(500);
            }
        }
    }

    public async Task EnsureDownloadedAsync()
    {
        await s_lock.WaitAsync();
        try
        {
            Directory.CreateDirectory(InstallPath);

            if (IsUpToDate())
            {
                return;
            }

            var lockPath = Path.Combine(InstallPath, ".lock");
            await using (await LockFileAsync(lockPath))
            {
                if (IsUpToDate())
                {
                    return;
                }

                await DownloadAsync();

                await File.WriteAllTextAsync(HashPath, Hash);
            }

            File.Delete(lockPath);
        }
        finally
        {
            s_lock.Release();
        }
    }

    protected abstract Task DownloadAsync();

    protected async Task DownloadFileAsync(string url, string destination)
    {
        using var httpClient = new HttpClient();
        using var response = await httpClient.GetAsync(url);
        await using var fs = File.Create(destination);
        await response.Content.CopyToAsync(fs);
    }
}
