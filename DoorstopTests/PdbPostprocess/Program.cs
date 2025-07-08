using Mono.Cecil;
using Mono.Cecil.Cil;
using Mono.Cecil.Mdb;
using Mono.Cecil.Pdb;
using Mono.Cecil.Rocks;

namespace PdbPostprocess;

internal static class Program
{
    public static int Main(string[] args)
    {
        var files = new List<string>();
        foreach (var path in args)
        {
            if (Directory.Exists(path))
            {
                files.AddRange(Directory.GetFiles(path, "*.dll"));
            }
            else
            {
                files.Add(path);
            }
        }

        var failed = false;

        foreach (var path in files)
        {
            try
            {
                Console.WriteLine($"Processing {path}");

                var module = ModuleDefinition.ReadModule(path, new ReaderParameters
                {
                    ReadSymbols = true,
                    ReadWrite = true,
                });

                var symbolFormat = module.SymbolReader switch
                {
                    NativePdbReader => "pdb",
                    MdbReader => "mdb",
                    PortablePdbReader => "ppdb",
                    EmbeddedPortablePdbReader => "embedded ppdb",
                    _ => throw new ArgumentOutOfRangeException($"Unknown symbol reader: {module.SymbolReader}"),
                };

                Console.WriteLine($"Current symbol format: {symbolFormat}");

                if (module.SymbolReader is not PortablePdbReader)
                {
                    Console.WriteLine("Writing external ppdb");
                    module.Write(new WriterParameters
                    {
                        WriteSymbols = true,
                        SymbolWriterProvider = new PortablePdbWriterProvider(),
                    });
                }

                using (var writer = new MdbWriterProvider().GetSymbolWriter(module, path))
                {
                    Console.WriteLine("Writing mdb");
                    Write(writer, module);
                }
            }
            catch (SymbolsNotFoundException)
            {
                Console.ForegroundColor = ConsoleColor.Yellow;
                Console.WriteLine("No symbols found");
                Console.ResetColor();
            }
            catch (SymbolsNotMatchingException e)
            {
                Console.ForegroundColor = ConsoleColor.Yellow;
                Console.WriteLine(e.Message);
                Console.ResetColor();
            }
            catch (Exception e)
            {
                failed = true;

                Console.ForegroundColor = ConsoleColor.Red;
                Console.Error.WriteLine(e);
                Console.ResetColor();
            }
        }

        return failed ? 1 : 0;
    }

    private static void Write(ISymbolWriter writer, ModuleDefinition module)
    {
        writer.Write();

        foreach (var type in module.GetAllTypes())
        {
            foreach (var method in type.Methods)
            {
                if (method.DebugInformation != null)
                {
                    // Cecil will try reading the file from disk for null/incompatible hashes, so avoid it by setting a blank hash
                    foreach (var sequencePoint in method.DebugInformation.SequencePoints)
                    {
                        var document = sequencePoint.Document;
                        if (document.Hash is not { Length: 16 })
                        {
                            document.Hash = new byte[16];
                        }
                    }

                    writer.Write(method.DebugInformation);
                }
            }

            if (type.HasCustomDebugInformations)
            {
                writer.Write(type);
            }
        }
    }
}
