# Test Windows notifications in wi-notch

Windows requires the notification listener capability and your permission. Use the local development package on an x64 Windows PC with Developer Mode enabled. Launch the registered app through Start so the process has package identity.

1. Close wi-notch if it is running.
2. From the repository root, run `powershell.exe -NoProfile -File .\packaging\register-dev.ps1`. It builds wi-notch and registers the development package from `target\notifications-dev`.
3. Open **wi-notch (Development)** from Start. Click the bell, choose **Enable Windows notifications**, and respond to the Windows prompt.
4. Move the pointer away from the notch and trigger a toast in a Windows app. The notch briefly shows a preview, and the bell opens the list.

Use `-StageOnly` to build the package folder without registering it. The normal `cargo run` version can open the notification panel and explains how to use the development package. Check **Settings → Privacy & security → Notifications** to change permission later.

To unregister the development app for your account, close it and run `Get-AppxPackage -Name HexCore99.WiNotch.Dev | Remove-AppxPackage` in PowerShell.

[Windows notification listener](https://learn.microsoft.com/en-us/windows/apps/develop/notifications/app-notifications/notification-listener) · [Development registration](https://learn.microsoft.com/powershell/module/appx/add-appxpackage?view=windows-11)
