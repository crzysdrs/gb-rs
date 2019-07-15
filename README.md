# Rust Gameboy Emulator

## Description

This project is a Game Boy Emulator intended to act as a learning excercise for both emulators and Rust. It has no documentation (as it isn't intended to be a perfect emulator (see [MoonEye](https://github.com/Gekkio/mooneye-gb) for that level of quality)). It takes a fairly object-oriented approach to managing the various subsystems in the emulated hardware.

Video Demo: [Mario and the Six Golden Coins - First Level](https://youtu.be/slTPkHDkcG0)

### To Play

Make sure to have access to the ROMs that you have legally acquired and an optional boot ROM. Build instructions are for Ubuntu.

NOTE: It does not currently save/load game save files. This feature is easy to add but will distract from future development.


```
$ apt-get install libsdl2-gfx-dev
$ cargo run --release $YOUR_ROM
```

#### Controls

| Game Boy | Key        |
| -------- | ---------- |
| Dpad     | Arrow keys |
| A        | A          |
| B        | Z          |
| Start    | Return     |
| Select   | Tab        |

### Implemented
* Display
* MMU
* CPU
* DMA
* Color Palettes
* Most Memory Bank Controllers
* Sound Channels (with dubious correctness)
* User Input
* Timers

### Unimplemented
* Serial Communication
* Gameboy Color DMA

## Passing Tests
 * Passes all Blargg's non-sound tests. 
   * Sound controllers have really obnoxious edge cases. The sound quality was better before I started trying to fix these tests. More research required.
 * Passes all Mooneye MBC related tests.
   * Other tests haven't been investigated much, relying mostly on blarggs for correctness.

### Features I Want To Add

* Rewind
* Full Game Boy Color Support
* Saving Games 

## Known Issues

* Sound Channel 4 (WAV Channel) does not always play at correct speed. (See: Pokemon Yello)
* Other sound channels sound *wrong* after passing the Blarg sound tests (probably related to frequency envelope calcuations).
* Game Boy Color is not yet supported (in progress).
* Screen is updated all at once at Vsync, which does not account for window shifting on a per LCD line basis.



