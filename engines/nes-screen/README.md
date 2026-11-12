# nes-screen

Shows an [`nes-ppu`](../nes-ppu) frame on an Exact GPU surface. A game implements `NesGame`:

```rust
impl nes_screen::NesGame for MyGame {
    fn bind(&mut self, inputs: &[Value]) -> Result<(), SurfaceError> { /* read the view's inputs */ }
    fn draw(&mut self, frame: u64, rgba: &mut [u8]) { /* nes_ppu::render(...).to_rgba(rgba) */ }
}

pub static REGISTRY: Registry = Registry {
    surfaces: &[("ppu", INPUTS, nes_screen::surface::<MyGame>)],
    shaders: nes_screen::shaders::SHADERS,
};
```

The crate counts frames (60 a second), uploads the 256×240 picture and draws it at the largest
whole-number scale that fits, letterboxed black (`shaders/screen.wgsl`). The app lists the shader
folder in `app.json` so the host registers it:

```json
"gpu": { "shaderRoots": ["../../engines/nes-screen/shaders"] }
```
