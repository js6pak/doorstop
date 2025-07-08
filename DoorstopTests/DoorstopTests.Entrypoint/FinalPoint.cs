#if MONO
using System;
using UnityEngine;
#endif

namespace DoorstopTests.Entrypoint;

internal static class FinalPoint
{
    public static void Run()
    {
#if MONO
        var gameObject = new GameObject
        {
            hideFlags = HideFlags.HideAndDontSave,
        };
        UnityEngine.Object.DontDestroyOnLoad(gameObject);
        gameObject.AddComponent<TestComponent>();
#else
        ExitSuccess();
#endif
    }

    public static void ExitSuccess()
    {
        Utilities.Terminate(0xAA);
    }

#if MONO
    public sealed class TestComponent : MonoBehaviour
    {
        static TestComponent()
        {
            Console.WriteLine("TestComponent.cctor");
        }

        public TestComponent()
        {
            Console.WriteLine("TestComponent.ctor");
        }

        public void Awake()
        {
            Console.WriteLine("TestComponent.Awake");
        }

        public void Start()
        {
            Console.WriteLine("TestComponent.Start");
            ExitSuccess();
        }
    }
#endif
}
