using System;
using System.Runtime.CompilerServices;

namespace DoorstopTests.Entrypoint;

internal static class DebuggingTest
{
    [MethodImpl(MethodImplOptions.NoOptimization)]
    public static void Test()
    {
        var local = true;
        Console.WriteLine("break here"); // note: this line's number is hardcoded in TestGameTests.Debugging.cs
        if (local)
        {
            throw new Exception("Debugging test failed, local was not changed");
        }

        Utilities.Terminate(0xAB);
    }
}
