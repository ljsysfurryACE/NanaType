use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use crate::mixer::{Command, Mixer};
use crate::{Bus, Pcm, Scheduled, SoundId, Voice};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    NoDevice,
    Device(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDevice => write!(f, "no audio output device"),
            Self::Device(e) => write!(f, "audio device error: {e}"),
        }
    }
}

impl std::error::Error for AudioError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioStatus {
    Running {
        device: String,
        rate: u32,
    },
    /// The device disappeared; the engine is reopening the default one.
    Lost(String),
    Failed(String),
}

/// Cheap to clone; every method is non-blocking.
#[derive(Clone)]
pub struct AudioHandle {
    tx: Sender<Command>,
    control: Sender<Control>,
}

enum Control {
    Device(Option<String>),
    Stop,
}

impl AudioHandle {
    fn send(&self, command: Command) {
        let _ = self.tx.send(command);
    }

    pub fn load(&self, id: SoundId, pcm: Arc<Pcm>) {
        self.send(Command::Load(id, pcm));
    }

    pub fn unload(&self, id: SoundId) {
        self.send(Command::Unload(id));
    }

    pub fn play(&self, voice: Voice) {
        self.send(Command::Play(voice));
    }

    /// Queue sounds at instants; a later `cancel(epoch)` drops those not started.
    pub fn schedule(&self, epoch: u64, items: Vec<Scheduled>) {
        if !items.is_empty() {
            self.send(Command::Schedule { epoch, items });
        }
    }

    pub fn cancel(&self, epoch: u64) {
        self.send(Command::Cancel { epoch });
    }

    pub fn set_master(&self, gain: f32) {
        self.send(Command::Master(gain));
    }

    pub fn set_bus(&self, bus: Bus, gain: f32) {
        self.send(Command::BusGain(bus, gain));
    }

    /// Switch output device (`None` = system default).
    pub fn set_device(&self, name: Option<String>) {
        let _ = self.control.send(Control::Device(name));
    }

    pub fn stop(&self) {
        let _ = self.control.send(Control::Stop);
    }
}

pub struct AudioEngine;

/// Output device names for the settings picker.
pub fn list_devices() -> Vec<String> {
    let host = cpal::default_host();
    host.output_devices()
        .map(|devices| {
            devices
                .filter_map(|d| d.description().ok().map(|desc| desc.name().to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn open(
    name: Option<&str>,
    rx: Receiver<Command>,
    lost: Sender<String>,
) -> Result<(cpal::Stream, String, u32), AudioError> {
    let host = cpal::default_host();
    let device = match name {
        Some(name) => host
            .output_devices()
            .map_err(|e| AudioError::Device(e.to_string()))?
            .find(|d| d.description().is_ok_and(|desc| desc.name() == name))
            .or_else(|| host.default_output_device()),
        None => host.default_output_device(),
    }
    .ok_or(AudioError::NoDevice)?;
    let device_name = device
        .description()
        .map(|d| d.name().to_owned())
        .unwrap_or_else(|_| "default".into());
    let config = device
        .default_output_config()
        .map_err(|e| AudioError::Device(e.to_string()))?;
    let rate = config.sample_rate();
    let channels = usize::from(config.channels());
    let mut mixer = Mixer::new(rate);
    let stream_config: cpal::StreamConfig = config.config();
    let err_tx = lost.clone();
    let mut scratch: Vec<f32> = Vec::new();
    let build = |format: cpal::SampleFormat| -> Result<cpal::Stream, cpal::Error> {
        match format {
            cpal::SampleFormat::F32 => device.build_output_stream(
                stream_config,
                move |out: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    while let Ok(c) = rx.try_recv() {
                        mixer.apply(c);
                    }
                    let ts = info.timestamp();
                    let ahead = ts.playback.duration_since(ts.callback);
                    mixer.render(out, channels, Instant::now() + ahead);
                },
                move |e| {
                    let _ = err_tx.send(e.to_string());
                },
                None,
            ),
            other => {
                let rx = rx.clone();
                device.build_output_stream_raw(
                    stream_config,
                    other,
                    move |data: &mut cpal::Data, info: &cpal::OutputCallbackInfo| {
                        while let Ok(c) = rx.try_recv() {
                            mixer.apply(c);
                        }
                        let len = data.len();
                        scratch.resize(len, 0.0);
                        let ts = info.timestamp();
                        let ahead = ts.playback.duration_since(ts.callback);
                        mixer.render(&mut scratch, channels, Instant::now() + ahead);
                        if let Some(out) = data.as_slice_mut::<i16>() {
                            for (o, s) in out.iter_mut().zip(&scratch) {
                                *o = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                            }
                        } else if let Some(out) = data.as_slice_mut::<u16>() {
                            for (o, s) in out.iter_mut().zip(&scratch) {
                                *o =
                                    ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * f32::from(u16::MAX)) as u16;
                            }
                        } else if let Some(out) = data.as_slice_mut::<i32>() {
                            for (o, s) in out.iter_mut().zip(&scratch) {
                                *o = (f64::from(s.clamp(-1.0, 1.0)) * f64::from(i32::MAX)) as i32;
                            }
                        }
                    },
                    move |e| {
                        let _ = lost.send(e.to_string());
                    },
                    None,
                )
            }
        }
    };
    let stream = build(config.sample_format()).map_err(|e| AudioError::Device(e.to_string()))?;
    stream
        .play()
        .map_err(|e| AudioError::Device(e.to_string()))?;
    Ok((stream, device_name, rate))
}

impl AudioEngine {
    /// Start the audio thread. `status` is called from that thread whenever
    /// the device state changes; the host forwards it to its UI.
    pub fn start(
        device: Option<String>,
        status: impl Fn(AudioStatus) + Send + 'static,
    ) -> AudioHandle {
        let (tx, rx) = unbounded::<Command>();
        let (control_tx, control_rx) = unbounded::<Control>();
        thread::Builder::new()
            .name("nanatype-audio".into())
            .spawn(move || {
                let mut wanted = device;
                loop {
                    let (lost_tx, lost_rx) = bounded::<String>(4);
                    let stream = match open(wanted.as_deref(), rx.clone(), lost_tx) {
                        Ok((stream, name, rate)) => {
                            status(AudioStatus::Running { device: name, rate });
                            Some(stream)
                        }
                        Err(e) => {
                            status(AudioStatus::Failed(e.to_string()));
                            None
                        }
                    };
                    // Wait for a control message, a device error, or retry later.
                    let retry = if stream.is_some() {
                        Duration::from_secs(3600)
                    } else {
                        Duration::from_secs(5)
                    };
                    crossbeam_channel::select! {
                        recv(control_rx) -> msg => match msg {
                            Ok(Control::Device(name)) => wanted = name,
                            Ok(Control::Stop) | Err(_) => return,
                        },
                        recv(lost_rx) -> err => {
                            if let Ok(e) = err {
                                status(AudioStatus::Lost(e));
                            }
                            thread::sleep(Duration::from_millis(500));
                        },
                        default(retry) => {}
                    }
                    drop(stream);
                    // Drain commands meant for the dead stream except sound loads,
                    // which the next stream needs; the mixer is rebuilt from scratch,
                    // so the host reloads banks on `Running`.
                    while rx.try_recv().is_ok() {}
                }
            })
            .expect("spawn audio thread");
        AudioHandle {
            tx,
            control: control_tx,
        }
    }
}
