using System;
using System.Diagnostics.CodeAnalysis;
using System.Reflection;
using DoorstopTests.Entrypoint;

[assembly: SuppressMessage("Style", "IDE0130:Namespace does not match folder structure", Scope = "namespace", Target = "~N:Doorstop")]

// ReSharper disable once CheckNamespace
namespace Doorstop;

[SuppressMessage("Design", "MA0048:File name must match type name")]
public static class Entrypoint
{
    public static void Start()
    {
        Console.WriteLine($"Hello from {Assembly.GetExecutingAssembly().Location}!");

        var scenario = Environment.GetEnvironmentVariable("DOORSTOPTESTS_SCENARIO");
        switch (scenario)
        {
            case "throw":
            {
                throw new Exception("Entrypoint crash test");
            }

            case "crash":
            {
                unsafe
                {
                    _ = *(nint*) 0xDEADBEEF;
                }

                return;
            }

            case "debugging":
            {
                DebuggingTest.Test();
                return;
            }

            case null: break;
            default: throw new Exception("Unknown scenario: " + scenario);
        }

        Console.WriteLine("Example stacktrace:");
        Console.WriteLine(Environment.StackTrace);

        ExitSuccess();
    }

    private static void ExitSuccess()
    {
        Utilities.Terminate(0xAA);
    }
}
