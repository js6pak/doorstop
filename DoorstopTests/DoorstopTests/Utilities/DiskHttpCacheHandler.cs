using System.Net;
using System.Net.Http.Headers;

namespace DoorstopTests.Utilities;

internal sealed class DiskHttpCacheHandler : DelegatingHandler
{
    private static string HttpCacheDirectory { get; } = Path.Combine(Constants.BaseOutputPath, "http-cache");

    private static string? ReadIfExists(string path)
    {
        try
        {
            return File.ReadAllText(path);
        }
        catch (IOException e) when (e is FileNotFoundException or DirectoryNotFoundException)
        {
            return null;
        }
    }

    protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
    {
        var cacheKey = request.RequestUri!.PathAndQuery[1..].Replace('/', '_');
        var cacheETagPath = Path.Combine(HttpCacheDirectory, cacheKey + ".etag");
        var cacheContentPath = Path.Combine(HttpCacheDirectory, cacheKey);

        if (DateTime.Now - File.GetLastWriteTime(cacheContentPath) < TimeSpan.FromHours(1))
        {
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(await File.ReadAllTextAsync(cacheContentPath, cancellationToken), new MediaTypeHeaderValue("application/json")),
                RequestMessage = request,
            };
        }

        if (ReadIfExists(cacheETagPath) is { } etag)
        {
            request.Headers.IfNoneMatch.TryParseAdd(etag);
        }

        var response = await base.SendAsync(request, cancellationToken);

        if (response.StatusCode == HttpStatusCode.NotModified && ReadIfExists(cacheContentPath) is { } cachedContent)
        {
            File.SetLastWriteTime(cacheContentPath, DateTime.Now);
            return new HttpResponseMessage(HttpStatusCode.OK)
            {
                Content = new StringContent(cachedContent, new MediaTypeHeaderValue("application/json")),
                RequestMessage = request,
            };
        }

        if (response.Headers.ETag != null)
        {
            var content = await response.Content.ReadAsStringAsync(cancellationToken);
            Directory.CreateDirectory(HttpCacheDirectory);
            await File.WriteAllTextAsync(cacheContentPath, content, cancellationToken);
            await File.WriteAllTextAsync(cacheETagPath, response.Headers.ETag.Tag, cancellationToken);
        }

        return response;
    }
}
