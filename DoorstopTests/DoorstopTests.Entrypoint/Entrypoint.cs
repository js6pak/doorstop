using System;
using System.Diagnostics.CodeAnalysis;
using System.IO;
using System.Reflection;
using DoorstopTests.Entrypoint;
using MonoMod.Utils;

[assembly: SuppressMessage("Style", "IDE0130:Namespace does not match folder structure", Justification = "N/A", Scope = "namespace", Target = "~N:Doorstop")]

// ReSharper disable once CheckNamespace
namespace Doorstop;

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

        var assemblyDirectory = Path.GetDirectoryName(Assembly.GetExecutingAssembly().Location)!;
        var frameworkSpecificDirectory = Path.Combine(assemblyDirectory, Environment.Version.Major >= 4 && false ? "net45" : "net35");
        AppDomain.CurrentDomain.AssemblyResolve += (_, args) =>
        {
            var assemblyName = new AssemblyName(args.Name);

            try
            {
                return Assembly.LoadFrom(Path.Combine(frameworkSpecificDirectory, $"{assemblyName.Name}.dll"));
            }
            catch (FileNotFoundException)
            {
            }

            return null;
        };

        AssertRuntime();

        UnityEntrypoint.Setup();
    }

    private static void AssertRuntime()
    {
        var runtime = Environment.GetEnvironmentVariable("DOORSTOPTESTS_ASSERT_RUNTIME");
        if (!string.IsNullOrEmpty(runtime))
        {
            var expected = Enum.Parse<RuntimeKind>(runtime);
            var actual = PlatformDetection.Runtime;
            if (expected != actual)
            {
                throw new Exception($"Runtime assert failed: expected {expected}, but found {actual}");
            }
        }
    }
}

// TODO move to backports
#if !NETCOREAPP2_0_OR_GREATER
internal static class EnumExtensions
{
    extension(Enum)
    {
        public static TEnum Parse<TEnum>(string value) where TEnum : struct
        {
            return (TEnum) Enum.Parse(typeof(TEnum), value);
        }
    }
}
#endif
