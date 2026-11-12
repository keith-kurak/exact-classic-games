//! Live audio from a GPU module: push mono `f32` samples as you make them,
//! and they play. A stopgap until Exact can stream audio itself: it opens
//! the platform's output directly (AudioQueue on macOS and iOS; nothing
//! elsewhere), so it ignores the app's `audio_session` and Exact's voice
//! table, and must not be opened when the agent owns the clock (tests).
//!
//! The device pulls 1024-sample buffers on its own thread from a ring the
//! game fills. An empty ring plays silence; a ring more than `max_latency`
//! ahead drops its oldest samples, so a game that runs fast never drifts
//! behind the picture.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

struct Shared {
    ring: Mutex<VecDeque<f32>>,
    max: usize,
    volume: Mutex<f32>,
}

/// An open audio output.
pub struct LiveAudio {
    shared: Arc<Shared>,
    rate: u32,
    #[cfg(target_vendor = "apple")]
    queue: apple::Queue,
}

impl LiveAudio {
    /// Open the output at `rate` samples a second, holding at most
    /// `max_latency_ms` of sound ahead. `None` when there is no output here.
    pub fn open(rate: u32, max_latency_ms: u32) -> Option<LiveAudio> {
        let shared = Arc::new(Shared {
            ring: Mutex::new(VecDeque::with_capacity(rate as usize)),
            max: (rate as u64 * max_latency_ms as u64 / 1000) as usize,
            volume: Mutex::new(1.0),
        });
        #[cfg(target_vendor = "apple")]
        {
            let queue = apple::Queue::open(rate, shared.clone())?;
            Some(LiveAudio { shared, rate, queue })
        }
        #[cfg(not(target_vendor = "apple"))]
        {
            let _ = shared;
            None
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Samples to play after those already waiting.
    pub fn push(&self, samples: &[f32]) {
        let mut ring = self.shared.ring.lock().unwrap();
        ring.extend(samples.iter().copied());
        let over = ring.len().saturating_sub(self.shared.max);
        ring.drain(..over);
    }

    /// Samples waiting to play.
    pub fn queued(&self) -> usize {
        self.shared.ring.lock().unwrap().len()
    }

    /// 0 (silent) to 1.
    pub fn set_volume(&self, v: f32) {
        *self.shared.volume.lock().unwrap() = v.clamp(0.0, 1.0);
    }

    /// Stop pulling (the app is hidden); what waits is dropped.
    pub fn pause(&self) {
        self.shared.ring.lock().unwrap().clear();
        #[cfg(target_vendor = "apple")]
        self.queue.pause();
    }

    pub fn resume(&self) {
        #[cfg(target_vendor = "apple")]
        self.queue.resume();
    }
}

impl Shared {
    /// Fill `out` from the ring; silence where it runs dry.
    fn fill(&self, out: &mut [f32]) {
        let volume = *self.volume.lock().unwrap();
        let mut ring = self.ring.lock().unwrap();
        for s in out.iter_mut() {
            *s = ring.pop_front().unwrap_or(0.0) * volume;
        }
    }
}

#[cfg(target_vendor = "apple")]
mod apple {
    //! AudioQueue, by hand: the few calls and structures an output needs.
    use super::Shared;
    use std::ffi::c_void;
    use std::sync::Arc;

    type OSStatus = i32;
    type AudioQueueRef = *mut c_void;

    #[repr(C)]
    struct AudioStreamBasicDescription {
        sample_rate: f64,
        format_id: u32,
        format_flags: u32,
        bytes_per_packet: u32,
        frames_per_packet: u32,
        bytes_per_frame: u32,
        channels_per_frame: u32,
        bits_per_channel: u32,
        reserved: u32,
    }

    #[repr(C)]
    struct AudioQueueBuffer {
        capacity: u32,
        data: *mut c_void,
        byte_size: u32,
        user_data: *mut c_void,
        packet_description_capacity: u32,
        packet_descriptions: *mut c_void,
        packet_description_count: u32,
    }
    type AudioQueueBufferRef = *mut AudioQueueBuffer;
    type Callback = extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef);

    #[link(name = "AudioToolbox", kind = "framework")]
    extern "C" {
        fn AudioQueueNewOutput(
            format: *const AudioStreamBasicDescription,
            callback: Callback,
            user_data: *mut c_void,
            run_loop: *const c_void,
            run_loop_mode: *const c_void,
            flags: u32,
            out: *mut AudioQueueRef,
        ) -> OSStatus;
        fn AudioQueueAllocateBuffer(queue: AudioQueueRef, bytes: u32, out: *mut AudioQueueBufferRef) -> OSStatus;
        fn AudioQueueEnqueueBuffer(queue: AudioQueueRef, buffer: AudioQueueBufferRef, packets: u32, descriptions: *const c_void) -> OSStatus;
        fn AudioQueueStart(queue: AudioQueueRef, at: *const c_void) -> OSStatus;
        fn AudioQueuePause(queue: AudioQueueRef) -> OSStatus;
        fn AudioQueueStop(queue: AudioQueueRef, immediate: u8) -> OSStatus;
        fn AudioQueueDispose(queue: AudioQueueRef, immediate: u8) -> OSStatus;
    }

    const FRAMES: u32 = 1024;
    const BUFFERS: usize = 3;
    const LINEAR_PCM: u32 = u32::from_be_bytes(*b"lpcm");
    const FLOAT_PACKED: u32 = 1 | 8;

    extern "C" fn refill(user: *mut c_void, queue: AudioQueueRef, buffer: AudioQueueBufferRef) {
        // SAFETY: `user` is the Arc<Shared> the queue was made with, alive
        // until the queue is disposed; `buffer` is one of its buffers.
        unsafe {
            let shared = &*(user as *const Shared);
            let b = &mut *buffer;
            let out = std::slice::from_raw_parts_mut(b.data as *mut f32, (b.capacity / 4) as usize);
            shared.fill(out);
            b.byte_size = b.capacity;
            AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null());
        }
    }

    pub struct Queue {
        queue: AudioQueueRef,
        user: *const Shared,
    }

    // The queue is driven by its own thread; the handle only starts,
    // pauses and disposes of it.
    unsafe impl Send for Queue {}

    impl Queue {
        pub fn open(rate: u32, shared: Arc<Shared>) -> Option<Queue> {
            let format = AudioStreamBasicDescription {
                sample_rate: rate as f64,
                format_id: LINEAR_PCM,
                format_flags: FLOAT_PACKED,
                bytes_per_packet: 4,
                frames_per_packet: 1,
                bytes_per_frame: 4,
                channels_per_frame: 1,
                bits_per_channel: 32,
                reserved: 0,
            };
            let user = Arc::into_raw(shared);
            let mut queue: AudioQueueRef = std::ptr::null_mut();
            // SAFETY: plain AudioQueue calls on the structures above; on any
            // failure the Arc is taken back.
            unsafe {
                if AudioQueueNewOutput(&format, refill, user as *mut c_void, std::ptr::null(), std::ptr::null(), 0, &mut queue) != 0 {
                    drop(Arc::from_raw(user));
                    return None;
                }
                for _ in 0..BUFFERS {
                    let mut buffer: AudioQueueBufferRef = std::ptr::null_mut();
                    if AudioQueueAllocateBuffer(queue, FRAMES * 4, &mut buffer) != 0 {
                        AudioQueueDispose(queue, 1);
                        drop(Arc::from_raw(user));
                        return None;
                    }
                    refill(user as *mut c_void, queue, buffer);
                }
                if AudioQueueStart(queue, std::ptr::null()) != 0 {
                    AudioQueueDispose(queue, 1);
                    drop(Arc::from_raw(user));
                    return None;
                }
            }
            Some(Queue { queue, user })
        }

        pub fn pause(&self) {
            unsafe {
                AudioQueuePause(self.queue);
            }
        }

        pub fn resume(&self) {
            unsafe {
                AudioQueueStart(self.queue, std::ptr::null());
            }
        }
    }

    impl Drop for Queue {
        fn drop(&mut self) {
            // SAFETY: stop and dispose synchronously, so no callback runs
            // after, then release the Arc the callbacks used.
            unsafe {
                AudioQueueStop(self.queue, 1);
                AudioQueueDispose(self.queue, 1);
                drop(Arc::from_raw(self.user));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_drops_its_oldest_past_the_latency() {
        let shared = Shared { ring: Mutex::new(VecDeque::new()), max: 4, volume: Mutex::new(1.0) };
        let audio_like = |samples: &[f32]| {
            let mut ring = shared.ring.lock().unwrap();
            ring.extend(samples.iter().copied());
            let over = ring.len().saturating_sub(shared.max);
            ring.drain(..over);
        };
        audio_like(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let mut out = [0.0; 6];
        shared.fill(&mut out);
        assert_eq!(out, [3.0, 4.0, 5.0, 6.0, 0.0, 0.0], "newest kept, silence after");
    }

    /// Opens the real output and plays a quiet half-second tone (run by hand:
    /// `cargo test tone -- --ignored`).
    #[test]
    #[ignore]
    fn tone() {
        let Some(audio) = LiveAudio::open(48_000, 500) else { return };
        let samples: Vec<f32> = (0..24_000).map(|i| ((i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin()) * 0.2).collect();
        audio.push(&samples);
        std::thread::sleep(std::time::Duration::from_millis(600));
        assert!(audio.queued() < 2_000);
    }
}
