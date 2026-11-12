## Download

| OS | File |
| --- | --- |
| macOS (Apple Silicon and Intel) | `capup_<version>_universal.dmg` |
| Windows | `capup_<version>_x64-setup.exe` or `capup_<version>_x64_en-US.msi` |

## Opening capup on macOS

capup is not signed with an Apple Developer ID.
macOS therefore blocks it the first time, saying it "is damaged" or "cannot be opened".

1. Open the `.dmg` and drag **capup** into **Applications**.
2. Remove the quarantine flag that macOS put on the downloaded app:

   ```bash
   xattr -dr com.apple.quarantine /Applications/capup.app
   ```

3. Open capup. Its icon appears in the menu bar.

Instead of step 2, you can also try to open capup once, then press **Open Anyway** in System Settings › Privacy & Security.

capup needs the **Screen & System Audio Recording** permission.
Grant it in System Settings › Privacy & Security, then restart capup.

Every build has a new signature, so macOS may forget the permission after an update.
If recording fails after an update, remove capup from the permission list, add it again, and restart capup.

## Opening capup on Windows

Windows SmartScreen may warn that it "protected your PC", because the installer is not signed.
Choose **More info**, then **Run anyway**.
