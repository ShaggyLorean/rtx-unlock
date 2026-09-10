<h1 align="center">rtx-unlock</h1>

<p align="center">DLSS Frame Generation and DLSS 5 neural rendering on RTX 20 and 30 series cards.<br>One executable. Pick a game, press Install.</p>

<p align="center"><a href="https://github.com/ShaggyLorean/rtx-unlock/releases/latest">Download the latest release</a></p>

<br>

## What it does

**Frame Generation unlock.** Games that ship Streamline DLSS-G hide the option on Turing and Ampere. rtx-unlock places the [dlssg_for_sm86](https://github.com/sdli1995/dlssg_for_sm86) proxy DLL under a proxy name the game's executable already imports and writes the ini for your card (SM75 or SM86). The game's own Frame Generation setting appears and works.

**DLSS 5 neural rendering.** rtx-unlock runs [DLSS 5 Autopilot](https://github.com/Kizzuwatnaa/DLSS5-Autopilot) in the background on its native route: ReShade, the renodx-dlss5 add-on and the nvngx_dlssnr build that matches your driver. Autopilot is fetched from GitHub and checked against its SHA-256 sums.

Both are removable. Every file the tool writes is recorded in a manifest next to the game executable, and Remove deletes exactly that list.

## Requirements

- Windows 10 or 11, 64-bit, with an NVIDIA driver installed (nvidia-smi must run).
- For the Frame Generation unlock: an RTX 20, RTX 30 or GTX 16 card, and a game that ships `sl.dlss_g.dll` or `nvngx_dlssg.dll`. Without that DLL there is nothing to unlock and the checkbox stays disabled.
- For DLSS 5: a DirectX 12 game with DLSS Super Resolution.

## Use

1. Run `rtx-unlock.exe`. Your Steam library is listed on the left. Use "Choose folder" for anything else.
2. Select a game. The right side shows the executable, the engine, which proxy names are free and which are taken, and your GPU.
3. Each component shows its state. The checkbox next to it reads Install when it is missing and Remove when it is present. Tick what you want; the button says exactly what will happen, for example "Install 2" or "Install 1, remove 1".
4. Launch the game.
   - Frame Generation: enable "NVIDIA DLSS Frame Generation" in the game's settings. Start with 2X.
   - DLSS 5: press Home to open ReShade, then enable neural rendering on the DLSS 5 tab. On RTX 20 and 30 the model runs in FP16 and costs roughly half your frame rate at full model resolution; lower the model resolution.
5. Check reports what is installed and what the logs say. Removing deletes only the files listed in the tool's manifest, so a hand-made install is left alone and named as such.

## DLSS 5 settings

When DLSS 5 is installed, a settings panel edits the `[RenoDX.DLSS5]` section of ReShade.ini: style, preset, intensity, local tone and structure, skin structure, UI correction and paper-white. Save while the game is closed; ReShade rewrites the ini on exit.

**The frame looks grey or washed out.** The game hands DLSS a scene-linear buffer from before tonemapping (RE Engine and Control do this). renodx-dlss5 4.55 maps that buffer to SDR with a fixed paper-white divisor, and the default of 1.0 blows every pixel out. rtx-unlock sets paper-white to 16 on RE Engine games at install time. Tune it between 8 and 32 to the scene.

**Driver note.** On NVIDIA driver 616.64 and newer, renodx-dlss5 4.6 and 4.7 fault on every frame (0 of 300 evaluates in the DLSS5-Feeder author's measurement), so 4.55 is installed and colour mapping stays manual. The 4.7 build with its automatic colour bridge runs on driver 616.56.

## Read before installing

- Online games with anti-cheat can ban for proxy DLLs and ReShade add-ons. That risk is yours.
- Antivirus software flags proxy DLLs and LoadLibrary hooks. Exclude the game folder.
- RE Engine games (Capcom) need REFramework before ReShade or the FG proxy can load. rtx-unlock installs the nightly monolithic `dinput8.dll` when it is missing and places the FG proxy under `reframework\plugins`.
- UE4SS, ReShade and OptiScaler take proxy names. When no imported name is free, rtx-unlock refuses to install and lists what is taken; Ultimate ASI Loader is the manual way around it.
- The OptiScaler route is left out on purpose: it corrupts SceneCapture rendering (scopes, mirrors) and conflicts with the game's own DLSS-G.
- dlssg_for_sm86 0.2.3 and newer give a black screen in some games. "legacy build 0.1.0" installs the older version under `version.dll`.

## Build

```
rustup default stable-x86_64-pc-windows-gnu
cargo build --release
```

The binary is `target\release\rtx-unlock.exe`. `cargo test` runs the unit tests. `RTXU_E2E_GAME=<game folder> cargo test e2e -- --ignored --nocapture` runs a real install, remove, install cycle against a game; add `RTXU_E2E_CLEAN=1` to leave the folder clean afterwards.

## Sources

Nothing is bundled. Every component is downloaded at run time from its publisher, pinned to a commit and a SHA-256 where the publisher does not sign releases.

- [sdli1995/dlssg_for_sm86](https://github.com/sdli1995/dlssg_for_sm86): the DLSS-G native wrapper. SM75 route by Coldwood1026.
- [Kizzuwatnaa/DLSS5-Autopilot](https://github.com/Kizzuwatnaa/DLSS5-Autopilot): MIT.
- [praydog/REFramework](https://github.com/praydog/REFramework): nightly monolithic build.
- ReShade by crosire and RenoDX by clshortfuse are downloaded by Autopilot from their publishers.
