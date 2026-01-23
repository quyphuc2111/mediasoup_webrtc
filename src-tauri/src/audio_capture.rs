// Audio capture service similar to RustDesk
// Uses cpal to capture audio directly in Rust instead of from frontend

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BufferSize, Device, Host, InputCallbackInfo, SampleFormat, StreamConfig, SupportedStreamConfig,
};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use dasp::sample::ToSample;

pub struct AudioCapture {
    input_buffer: Arc<Mutex<VecDeque<i16>>>,
    sample_rate: u32,
    channels: u16,
    _stream_handle: Option<std::thread::JoinHandle<()>>, // Keep stream alive in background thread
}

impl AudioCapture {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            input_buffer: Arc::new(Mutex::new(VecDeque::new())),
            sample_rate: 48000,
            channels: 1, // Mono
            _stream_handle: None,
        })
    }

    pub fn start_capture(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or("No input device available")?;

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get input config: {}", e))?;

        log::info!("Audio device: {}", device.name().unwrap_or("Unknown".to_string()));
        log::info!("Audio config: {:?}", config);

        // Normalize sample rate to 48000 or closest supported
        let sample_rate_0 = config.sample_rate().0;
        let sample_rate = if sample_rate_0 < 12000 {
            8000
        } else if sample_rate_0 < 16000 {
            12000
        } else if sample_rate_0 < 24000 {
            16000
        } else if sample_rate_0 < 48000 {
            24000
        } else {
            48000
        };

        self.sample_rate = sample_rate;
        self.channels = config.channels();

        // Clear buffer
        self.input_buffer.lock().unwrap().clear();

        let input_buffer = self.input_buffer.clone();
        let stream_config = StreamConfig {
            channels: config.channels(),
            sample_rate: config.sample_rate(),
            buffer_size: BufferSize::Default,
        };

        let stream = match config.sample_format() {
            SampleFormat::I8 => self.build_input_stream::<i8>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::I16 => self.build_input_stream::<i16>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::I32 => self.build_input_stream::<i32>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::I64 => self.build_input_stream::<i64>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::U8 => self.build_input_stream::<u8>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::U16 => self.build_input_stream::<u16>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::U32 => self.build_input_stream::<u32>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::U64 => self.build_input_stream::<u64>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::F32 => self.build_input_stream::<f32>(device, &config, stream_config, input_buffer.clone())?,
            SampleFormat::F64 => self.build_input_stream::<f64>(device, &config, stream_config, input_buffer.clone())?,
            _ => return Err("Unsupported sample format".to_string()),
        };

        stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;
        
        // Keep stream alive in a background thread
        let stream_handle = std::thread::spawn(move || {
            // Keep stream alive - block forever
            std::thread::park();
        });
        self._stream_handle = Some(stream_handle);

        Ok(())
    }

    fn build_input_stream<T>(
        &self,
        device: Device,
        _config: &SupportedStreamConfig,
        stream_config: StreamConfig,
        input_buffer: Arc<Mutex<VecDeque<i16>>>,
    ) -> Result<cpal::Stream, String>
    where
        T: cpal::SizedSample + ToSample<i16>,
    {
        let err_fn = move |err| {
            log::error!("Audio stream error: {}", err);
        };

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[T], _: &InputCallbackInfo| {
                    // Convert samples to i16 and add to buffer
                    // Use dasp's ToSample trait like RustDesk
                    let buffer: Vec<i16> = data.iter().map(|s| s.to_sample()).collect();
                    let mut lock = input_buffer.lock().unwrap();
                    lock.extend(buffer);
                },
                err_fn,
                None,
            )
            .map_err(|e| format!("Failed to build input stream: {}", e))?;

        Ok(stream)
    }

    pub fn stop_capture(&mut self) {
        if let Some(handle) = self._stream_handle.take() {
            handle.thread().unpark();
        }
        self.input_buffer.lock().unwrap().clear();
    }

    pub fn read_samples(&self, count: usize) -> Vec<i16> {
        let mut buffer = self.input_buffer.lock().unwrap();
        let mut samples = Vec::with_capacity(count);
        
        for _ in 0..count {
            if let Some(sample) = buffer.pop_front() {
                samples.push(sample);
            } else {
                break;
            }
        }
        
        samples
    }

    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn get_channels(&self) -> u16 {
        self.channels
    }

    pub fn has_samples(&self, count: usize) -> bool {
        let buffer = self.input_buffer.lock().unwrap();
        buffer.len() >= count
    }
}

