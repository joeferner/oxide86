# TODO

- [ ] Refactor WASM to not call document.add_event_listener_with_callback but instead listen externally and call wasm functions from typescript
- [ ] Fix examples/audio.bas audio timing and high pitched noise (see plans/fix-play-sound-issues.md)
- [ ] 8086 Test applications
  - [ ] Lotus 1-2-3
  - [ ] WordStar
- [ ] Implement CGA video mode
- [ ] 8086 Test CGA games
  - [ ] Alley Cat
  - [ ] Flight Simulator 1.0
- [ ] Refactor read_char to be blocking in GUI
- [ ] Upgrade to latest pixels/winit/egui (see https://github.com/parasyte/pixels/blob/main/examples/minimal-egui/Cargo.toml)
- [ ] Install SvarDOS (open source dos)
- [ ] Add support FreeDos-1.4 (requires 32-bit instructions)
- [ ] Remove dead code markers
- [ ] Test CLI/GUI on Windows
- [ ] Test CLI/GUI on OSX
