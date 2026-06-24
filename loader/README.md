# randommenu_loader — C# BepInEx plugin

This is the permanently-installed piece. Build it once, put it in your BepInEx plugins folder, and it auto-downloads the latest `randommenu.dll` (Rust) from GitHub releases on every game launch.

## Build

```
set GTAG_MANAGED=C:\Program Files (x86)\Steam\steamapps\common\Gorilla Tag\Gorilla Tag_Data\Managed
set GTAG_BEPINEX=C:\<your bepinex path>\BepInEx\core
dotnet build -c Release
```

Output: `bin/Release/netstandard2.0/randommenu_loader.dll`

## Install

1. Install [BepInEx 5](https://github.com/BepInEx/BepInEx/releases) into Gorilla Tag
2. Drop `randommenu_loader.dll` into `BepInEx/plugins/`
3. Launch the game — it downloads `randommenu.dll` automatically next to `Gorilla Tag.exe`
