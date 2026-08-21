# Distribution output

Build scripts place self-contained native artifacts here:

- `stfixest_linux.plugin`
- `stfixest_windows.plugin`
- `stfixest_macos.plugin` (universal x86-64 + arm64)

No placeholder binary is shipped. A plugin file in this directory is valid only
if built by the target-native script and accepted by `audit_binary.py`.
