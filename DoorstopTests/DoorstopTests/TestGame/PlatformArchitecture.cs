namespace DoorstopTests.TestGame;

public enum PlatformArchitecture
{
    X64,
    X86,
    Arm,
    Arm64,

    // MacOS-specific
    X64X86,
    X64Arm64,

    // Android-specific
    Universal,
}
