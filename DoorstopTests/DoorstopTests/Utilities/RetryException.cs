namespace DoorstopTests.Utilities;

internal sealed class RetryException : Exception
{
    public RetryException()
    {
    }

    public RetryException(string message) : base(message)
    {
    }

    public RetryException(string message, Exception innerException) : base(message, innerException)
    {
    }

    internal sealed class Attribute(int times) : RetryAttribute(times)
    {
        public override async Task<bool> ShouldRetry(TestContext context, Exception exception, int currentRetryCount)
        {
            if (exception is RetryException)
            {
                await Task.Delay(TimeSpan.FromSeconds(10 * currentRetryCount));
                return true;
            }

            return false;
        }
    }
}
