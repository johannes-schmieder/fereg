// Discover the real Break menu command in the exact child Stata process.
// SendMessageTimeout proves delivery before the native parked worker is released.
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Automation;
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
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32.dll")] private static extern bool IsIconic(IntPtr window);
    [DllImport("user32.dll")] private static extern bool ShowWindowAsync(IntPtr window, int command);
    [DllImport("user32.dll")] private static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] private static extern bool ScreenToClient(IntPtr window, ref Point point);
    [DllImport("user32.dll")] private static extern bool GetClientRect(IntPtr window, out Rect rect);
    [StructLayout(LayoutKind.Sequential)] private struct Point { public int X, Y; }
    [StructLayout(LayoutKind.Sequential)] private struct Rect { public int Left, Top, Right, Bottom; }
    private struct ToolbarButton { public IntPtr Window; public Point Center; }
    private static int ToolbarBreak(uint pid) {
        // Provider discovery can block. It runs in a bounded background thread
        // that never sends input, so a timed-out discovery cannot issue late Break.
        var matches = new List<ToolbarButton>();
        int discovery = 0;
        var thread = new Thread(delegate() {
            try {
                var windows = new List<IntPtr>();
                EnumWindows(delegate(IntPtr window, IntPtr ignored) {
                    uint owner;
                    GetWindowThreadProcessId(window, out owner);
                    if (owner == pid && IsWindowVisible(window)) windows.Add(window);
                    return true;
                }, IntPtr.Zero);
                if (windows.Count == 0) { discovery = 10; return; }
                foreach (IntPtr window in windows) {
                    var root = AutomationElement.FromHandle(window);
                    var buttons = root.FindAll(TreeScope.Descendants,
                        new PropertyCondition(AutomationElement.ControlTypeProperty, ControlType.Button));
                    if (buttons.Count > 128) { discovery = 11; return; }
                    foreach (AutomationElement button in buttons) {
                        var state = button.Current;
                        string name = state.Name.Replace("&", "").Trim();
                        if (name != "Break" && name != "Break (Ctrl+Break)" && name != "Break (Ctrl+Pause)" && name != "Break (Ctrl+Pause/Break)") continue;
                        if (!state.IsEnabled || state.IsOffscreen || state.BoundingRectangle.IsEmpty) {
                            discovery = 12; return;
                        }
                        var bounds = state.BoundingRectangle;
                        var center = new Point { X = (int)(bounds.Left + bounds.Width / 2), Y = (int)(bounds.Top + bounds.Height / 2) };
                        IntPtr toolbar = WindowFromPoint(center);
                        uint hitOwner;
                        GetWindowThreadProcessId(toolbar, out hitOwner);
                        if (toolbar == IntPtr.Zero || hitOwner != pid) { discovery = 18; return; }
                        Rect client;
                        if (!ScreenToClient(toolbar, ref center) || !GetClientRect(toolbar, out client) ||
                            center.X < 0 || center.Y < 0 || center.X >= client.Right || center.Y >= client.Bottom ||
                            center.X > 32767 || center.Y > 32767) { discovery = 13; return; }
                        matches.Add(new ToolbarButton { Window = toolbar, Center = center });
                    }
                }
            } catch { discovery = 14; }
        });
        thread.IsBackground = true;
        thread.SetApartmentState(ApartmentState.MTA);
        thread.Start();
        if (!thread.Join(10000)) return 15;
        if (discovery != 0) return discovery;
        if (matches.Count == 0) return 16;
        if (matches.Count != 1) return 17;
        ToolbarButton target = matches[0];
        uint currentOwner;
        GetWindowThreadProcessId(target.Window, out currentOwner);
        if (currentOwner != pid) return 18;
        IntPtr position = (IntPtr)((target.Center.Y << 16) | target.Center.X);
        UIntPtr result;
        // Send to the real toolbar, which performs its ordinary hit test and
        // parent notification. Do not infer command IDs from automation IDs.
        if (SendMessageTimeoutW(target.Window, 0x201, (UIntPtr)1, position, 0x23, 5000, out result) == IntPtr.Zero) return 19;
        if (SendMessageTimeoutW(target.Window, 0x202, UIntPtr.Zero, position, 0x23, 5000, out result) == IntPtr.Zero) return 19;
        return 0;
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
    public static void Restore(uint pid) {
        EnumWindows(delegate(IntPtr window, IntPtr ignored) {
            uint owner;
            GetWindowThreadProcessId(window, out owner);
            if (owner == pid && (IsWindowVisible(window) || IsIconic(window))) ShowWindowAsync(window, 9);
            return true;
        }, IntPtr.Zero);
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
        if (found.Count == 0) return ToolbarBreak(pid);
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
