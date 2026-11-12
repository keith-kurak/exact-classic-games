# live-audio

A stopgap until Exact can stream audio: a GPU module pushes mono `f32` samples as it makes them,
and they play.

```rust
let audio = live_audio::LiveAudio::open(48_000, 120)?;   // rate, most latency held ahead (ms)
audio.push(&samples);                                    // each frame's samples
audio.pause();                                           // hidden; resume() when shown
```

On macOS and iOS it opens an AudioQueue (three 1024-sample float buffers, about 64 ms), filled
on the queue's own thread from a ring the game writes. An empty ring plays silence; a ring more
than the latency ahead drops its oldest samples, so the sound never drifts behind the picture.
Elsewhere `open` returns `None`.

Because it opens the output itself, it bypasses Exact: no `audio_session`, no voice table, and
nothing the agent can record. Open it only when the display owns the clock (a surface's
`seekable` is false), so tests stay silent. Exactulator uses it.
`cargo test tone -- --ignored` plays a short test tone.
