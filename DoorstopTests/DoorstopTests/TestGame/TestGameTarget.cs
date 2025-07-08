using System.Runtime.InteropServices;
using System.Text.RegularExpressions;

namespace DoorstopTests.TestGame;

[StructLayout(LayoutKind.Auto)]
public readonly partial record struct TestGameTarget(Platform Platform, PlatformArchitecture PlatformArchitecture, ScriptingImplementation ScriptingImplementation)
{
    [GeneratedRegex("^(?<platform>win|osx|linux|android)-(?<architecture>x64|x86|arm|arm64|x64x86|x64arm64|universal)-(?<scripting_implementation>mono|il2cpp)$")]
    private static partial Regex TargetRegex { get; }

    public static TestGameTarget Parse(string text)
    {
        var target = TargetRegex.Match(text);
        var platform = Platform.Deserialize(target.Groups["platform"].Value);
        var platformArchitecture = Enum.Parse<PlatformArchitecture>(target.Groups["architecture"].Value, true);
        var scriptingImplementation = Enum.Parse<ScriptingImplementation>(target.Groups["scripting_implementation"].Value, true);
        return new TestGameTarget(platform, platformArchitecture, scriptingImplementation);
    }

    public bool CanRun()
    {
        var supportedPlatforms = new List<Platform>();
        if (OperatingSystem.IsWindows()) supportedPlatforms.Add(Platform.Windows);
        if (OperatingSystem.IsMacOS()) supportedPlatforms.Add(Platform.MacOS);
        if (OperatingSystem.IsLinux()) supportedPlatforms.AddRange(Platform.Linux, /* wine */ Platform.Windows);

        if (!supportedPlatforms.Contains(Platform)) return false;

        var supportedArchitectures = new List<PlatformArchitecture>();
        switch (RuntimeInformation.OSArchitecture)
        {
            case Architecture.X64:
                supportedArchitectures.Add(PlatformArchitecture.X64);
                if (!OperatingSystem.IsMacOS()) supportedArchitectures.Add(PlatformArchitecture.X86);
                supportedArchitectures.AddRange(PlatformArchitecture.X64X86, PlatformArchitecture.X64Arm64);
                break;
            case Architecture.Arm64:
                supportedArchitectures.Add(PlatformArchitecture.Arm64);
                supportedArchitectures.Add(PlatformArchitecture.X64Arm64);
                if (OperatingSystem.IsMacOS()) supportedArchitectures.AddRange(PlatformArchitecture.X64, PlatformArchitecture.X64X86);
                break;
            default:
                throw new PlatformNotSupportedException();
        }

        if (!supportedArchitectures.Contains(PlatformArchitecture)) return false;

        return true;
    }
}
