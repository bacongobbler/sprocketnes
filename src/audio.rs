//! SDL audio interface. Used by the APU to actually play audio.

//
// Author: Patrick Walton
//

// TODO: This module is very unsafe. Adding a reader-writer audio lock to SDL would help make it
// safe.

use sdl2;
use sdl2::audio::{AudioDevice, AudioCallback, AudioSpec, AudioSpecDesired, AudioDeviceLockGuard};
use std::cmp;
use std::mem;
use std::slice::from_raw_parts_mut;
use std::sync::{Mutex, Condvar};

//
// The audio callback
//

const SAMPLE_COUNT: usize = 4410 * 2;

static mut G_AUDIO_DEVICE: Option<*mut AudioDevice<NesAudioCallback>> = None;

static mut G_OUTPUT_BUFFER: Option<*mut OutputBuffer> = None;

lazy_static! {
    pub static ref AUDIO_MUTEX: Mutex<()> = Mutex::new(());
    pub static ref AUDIO_CONDVAR: Condvar = Condvar::new();
}

pub struct OutputBuffer {
    pub samples: [u8; SAMPLE_COUNT],
    pub play_offset: usize,
}

pub struct NesAudioCallback {
    samples: Vec<f32>,
    spec: AudioSpec
}

impl AudioCallback for NesAudioCallback {
    type Channel = f32;
    fn callback(&mut self, out: &mut [f32]) {
        if self.samples.len() < out.len() {
            // Zero out the buffer to avoid damaging the listener's eardrums.
            for value in out.iter_mut() {
                *value = 0.0
            }
        }

        let mut leftovers = Vec::new();
        for (i, sample) in mem::replace(&mut self.samples, Vec::new()).into_iter().enumerate() {
            if i < out.len() {
                out[i] = sample
            } else {
                leftovers.push(sample);
            }
        }
        self.samples = leftovers
    }
}

/// Audio initialization. If successful, returns a pointer to an allocated `OutputBuffer` that can
/// be filled with raw audio data.
pub fn open() -> Option<*mut OutputBuffer> {
    let sdl_context = sdl2::init().unwrap();
    let sdl_audio = sdl_context.audio().unwrap();

    let output_buffer = Box::new(OutputBuffer {
        samples: [ 0; SAMPLE_COUNT ],
        play_offset: 0,
    });
    let output_buffer_ptr: *mut OutputBuffer = unsafe {
        mem::transmute(&*output_buffer)
    };

    unsafe {
        G_OUTPUT_BUFFER = Some(output_buffer_ptr);
        mem::forget(output_buffer);
    }

    let desired_spec = AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1),
        samples: Some(4410),
    };

    let device = sdl_audio.open_playback(None, &desired_spec, |spec| NesAudioCallback {
        samples: Vec::new(),
        spec: spec,
    }).unwrap();

    device.resume();
    G_AUDIO_DEVICE = Some(mem::transmute(Box::new(device)));
    return Some(output_buffer_ptr);
}

//
// Audio tear-down
//

pub fn close() {
    unsafe {
        match G_AUDIO_DEVICE {
            None => {}
            Some(ptr) => {
                let _: Box<AudioDevice<NesAudioCallback>> = mem::transmute(ptr);
                G_AUDIO_DEVICE = None;
            }
        }
    }
}

pub fn lock<'a>() -> Option<AudioDeviceLockGuard<'a, NesAudioCallback>> {
    unsafe {
        G_AUDIO_DEVICE.map(|dev| (*dev).lock())
    }
}
