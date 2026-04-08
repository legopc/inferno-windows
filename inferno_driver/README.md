# inferno_driver — SYSVAD Virtual Audio Driver

This directory contains the Windows kernel audio driver for Inferno.
It is based on Microsoft's TabletAudioSample (SYSVAD) from the Windows Driver Samples repository.

## Prerequisites
- Windows Driver Kit (WDK) 10.0.26100.0+
- Visual Studio 2022 with "Desktop development with C++" workload
- WDK Visual Studio extension

## Build
```cmd
msbuild inferno_driver.vcxproj /p:Configuration=Release /p:Platform=x64
```

## Architecture
Audio flows from WASAPI → shared memory ring buffer → inferno_wasapi reads → Dante network
The driver writes to a named shared memory section: `Global\InfernoAudioShm`
