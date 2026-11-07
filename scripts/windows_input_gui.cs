// Discover the real Break menu command in the exact child Stata process.
// SendMessageTimeout proves delivery before the native parked worker is released.
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class FeregInputGui {
    private delegate bool EnumWindowsProc(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindowsProc callback, IntPtr data);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint pid);
    [DllImport("user32.dll")] private static extern IntPtr GetMenu(IntPtr window);
    [DllImport("user32.dll")] private static extern int GetMenuItemCount(IntPtr menu);
    [DllImport("user32.dll")] private static extern IntPtr GetSubMenu(IntPtr menu, int index);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetMenuStringW(IntPtr menu, uint item, StringBuilder text, int capacity, uint flags);
    [DllImport("user32.dll")] private static extern uint GetMenuItemID(IntPtr menu, int index);
    [DllImport("user32.dll")] private static extern uint GetMenuState(IntPtr menu, uint item, uint flags);
    [DllImport("user32.dll")] private static extern bool GetMenuInfo(IntPtr menu, ref MenuInfo info);
    [DllImport("user32.dll", SetLastError=true)] private static extern IntPtr SendMessageTimeoutW(IntPtr window, uint message, UIntPtr wParam, IntPtr lParam, uint flags, uint timeout, out UIntPtr result);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] private static extern uint GetTempPath2W(uint size, StringBuilder path);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode)] private static extern uint GetTempPathW(uint size, StringBuilder path);
    [StructLayout(LayoutKind.Sequential)] private struct MenuInfo {
        public uint size, mask, style, maxHeight;
        public IntPtr background;
        public uint contextHelp;
        public UIntPtr data;
    }
    private struct Command { public IntPtr Window, Menu; public int Position; }
    private static void Search(IntPtr window, IntPtr menu, int depth, List<Command> found) {
        if (menu == IntPtr.Zero || depth > 8) return;
        int count = GetMenuItemCount(menu);
        if (count < 0 || count > 512) throw new InvalidOperationException();
        for (int i = 0; i < count; i++) {
            var text = new StringBuilder(256);
            GetMenuStringW(menu, (uint)i, text, text.Capacity, 0x400);
            string label = text.ToString().Replace("&", "").Split('\t')[0].Trim();
            IntPtr child = GetSubMenu(menu, i);
            if (child == IntPtr.Zero && label == "Break")
                found.Add(new Command { Window = window, Menu = menu, Position = i });
            Search(window, child, depth + 1, found);
        }
    }
    public static int Break(uint pid) {
        var found = new List<Command>();
        bool hasWindow = false;
        EnumWindows(delegate(IntPtr window, IntPtr ignored) {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == pid) { hasWindow = true; Search(window, GetMenu(window), 0, found); }
            return true;
        }, IntPtr.Zero);
        if (!hasWindow) return 1;
        if (found.Count == 0) return 2;
        if (found.Count != 1) return 3;
        Command command = found[0];
        uint state = GetMenuState(command.Menu, (uint)command.Position, 0x400);
        if (state == 0xffffffff || (state & 3) != 0) return 4;
        var info = new MenuInfo { size = (uint)Marshal.SizeOf(typeof(MenuInfo)), mask = 0x10 };
        if (!GetMenuInfo(command.Menu, ref info)) return 5;
        uint message;
        UIntPtr wParam;
        IntPtr lParam;
        if ((info.style & 0x08000000) != 0) {
            message = 0x126; wParam = (UIntPtr)(uint)command.Position; lParam = command.Menu;
        } else {
            uint id = GetMenuItemID(command.Menu, command.Position);
            if (id > 65535) return 5;
            message = 0x111; wParam = (UIntPtr)id; lParam = IntPtr.Zero;
        }
        UIntPtr result;
        return SendMessageTimeoutW(command.Window, message, wParam, lParam, 0x23, 5000, out result) == IntPtr.Zero ? 6 : 0;
    }
    public static string TemporaryPath() {
        var path = new StringBuilder(32768);
        uint length;
        try { length = GetTempPath2W((uint)path.Capacity, path); }
        catch (EntryPointNotFoundException) { length = GetTempPathW((uint)path.Capacity, path); }
        if (length == 0 || length >= path.Capacity) throw new InvalidOperationException();
        return path.ToString();
    }
}
