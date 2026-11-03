using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Threading;

public class InterruptTool {
    [DllImport("kernel32.dll")]
    static extern uint GetConsoleProcessList(uint[] processes, uint count);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GenerateConsoleCtrlEvent(uint signal, uint group);

    public static int Main() {
        Console.CancelKeyPress += (sender, e) => {
            e.Cancel = true;
            File.WriteAllText("interrupted", "received");
            var deadline = DateTime.UtcNow.AddSeconds(15);
            while (!File.Exists("release") && DateTime.UtcNow < deadline) Thread.Sleep(10);
            File.WriteAllText("cleaned", "finished");
            Environment.Exit(37);
        };
        File.WriteAllText("ready", Process.GetCurrentProcess().Id.ToString());
        var limit = DateTime.UtcNow.AddSeconds(15);
        while (!File.Exists("send") && DateTime.UtcNow < limit) Thread.Sleep(10);
        // Never broadcast into the test runner's console or another user's console.
        if (GetConsoleProcessList(new uint[8], 8) != 2) return 81;
        if (!GenerateConsoleCtrlEvent(0, 0)) return 82;
        while (DateTime.UtcNow < limit) Thread.Sleep(10);
        return 83;
    }
}
