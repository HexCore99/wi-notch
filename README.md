# wi-notch

wi-notch is a desktop overlay for Windows that puts the clock, media controls, and notifications in a notch at the top of your display. Hover over the clock to expand it. Use the bell to browse notifications or the gear to open Settings.

## Features

- **Clock and media controls** with the date, album artwork, playback progress, and previous, play/pause, and next controls for media sessions exposed by Windows.
- **Windows notifications** with an unread indicator, brief previews, and a scrollable list with animated transitions. Select a notification to open its source app.
- **Prayer times** with a live countdown and an offline city picker. Use either side arrow on the expanded notch to switch between Music and Prayer Times.
- **Live settings** for the background, corner rounding, display, and vertical position.

## Run from source

Use Windows 11 on x64 with stable Rust for `x86_64-pc-windows-msvc` and the Visual Studio C++ build tools and Windows SDK. [Rust MSVC prerequisites](https://rust-lang.github.io/rustup/installation/windows-msvc.html)

```powershell
git clone https://github.com/HexCore99/wi-notch.git
cd wi-notch
cargo run --locked
```

Use `cargo run` for the clock, media controls, and settings. To read Windows notifications, launch the registered development package.

### Enable Windows notifications

Enable Developer Mode in Windows Settings. [Microsoft’s Developer Mode guide](https://learn.microsoft.com/en-us/windows/advanced-settings/developer-mode)

Close any running copy of wi-notch and run this command from the repository root in PowerShell.

```powershell
powershell.exe -NoProfile -File .\packaging\register-dev.ps1
```

Open **wi-notch (Development)** from the Start menu. Hover over the notch, click the bell, and select **Enable Windows notifications** if prompted.

You can also launch the registered app from your editor’s PowerShell terminal.

```powershell
explorer.exe 'shell:AppsFolder\HexCore99.WiNotch.Dev_qe18bhqmcs1eg!WiNotch'
```

After changing the code, close the running app and rebuild the executable in the existing package.

```powershell
powershell.exe -NoProfile -File .\packaging\register-dev.ps1 -StageOnly
```

Launch **wi-notch (Development)** again. See the development package guide for registration and removal steps. [Development package guide](packaging/README.md)

### Notification behavior

The bell shows a red dot for unread notifications. A new notification can briefly appear as a preview while the notch is collapsed. Click the preview to open the list, then select a row to open the source app.

For supported Microsoft Edge, Zen, and desktop Firefox notifications, wi-notch looks up the original click action in Windows notification history with a read-only query. When that action is available, the browser can open the specific page. Otherwise, wi-notch opens or focuses the source app. Notification text stays in memory.

## Settings

Hover over the notch and click the gear. Changes are applied immediately and saved to `%LOCALAPPDATA%\wi-notch\settings.json`.

- **Appearance** lets you choose a black or charcoal background and toggle rounded corners.
- **Display** lets you pick a monitor or follow the Windows primary display. **Top gap** sets the distance from the top edge, and **Offset** sets how much each press of **−** or **+** changes it.
- **Prayer Times** lets you select a city. Dhaka is the default. The schedule is calculated for the city's local date using the Karachi method and Hanafi Asr.

The bundled city list adapts the GeoNames `cities15000`, `admin1CodesASCII`, and `countryInfo` datasets. [GeoNames downloads](https://download.geonames.org/export/dump/), [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/)

## Development

wi-notch is built in Rust with GPUI 0.2.2. [GPUI documentation](https://docs.rs/gpui/0.2.2/gpui/)

- The notch, settings, media controls, and notification views live in the UI modules. [src/ui](src/ui)
- Windows integration lives in its own modules. [src/windows](src/windows)
- The development manifest and registration script live in the packaging directory. [packaging](packaging)

```powershell
cargo fmt --all -- --check
cargo test --locked
```
