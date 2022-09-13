use std::{
    collections::VecDeque,
    sync::{
        Arc, 
        atomic::{AtomicUsize, Ordering, AtomicBool},
        RwLock, 
        // Mutex, 
        // mpsc,
    },
    thread,
};

use anyhow::anyhow;
use cpal::traits::{HostTrait, DeviceTrait, StreamTrait};
use rubato::Resampler;
use tokio::{
    time::{sleep, Duration}, 
    sync::{mpsc, Mutex}, 
    task::{self, JoinHandle},
    runtime::Handle,
};

use crate::request;

pub struct AudioPlayer {
    inner: Arc<Mutex<AudioPlayerInner>>,
    thread_run_states: Vec<Arc<AtomicBool>>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel::<&'static str>(64);

        let inner = Arc::new(
            Mutex::new(
                AudioPlayerInner::new(tx.clone())
            )
        );

        let mut thread_run_states: Vec<Arc<AtomicBool>> = Vec::new();

        let _inner_1 = inner.clone();

        // ref: https://www.reddit.com/r/rust/comments/nwbtsz/help_understanding_how_to_start_and_stop_threads/
        let receiver_run_state = Arc::new(AtomicBool::new(true));
        let _receiver_run_state = receiver_run_state.clone();
        thread::spawn(move || loop {
            println!("start receiver");
            if !_receiver_run_state.load(Ordering::Relaxed) {
                println!("terminate receiver");
                break;
            }

            while let Some(data) = rx.blocking_recv() {
                println!("received message: {:?}", data);
                match data {
                    "stop" => _inner_1.blocking_lock().remove_audio(),
                    _ => (),
                }
            }
        });
        
        thread_run_states.push(receiver_run_state);

        Self {
            inner,
            thread_run_states
        }
    }

    pub async fn add_audio(&self, audio_tag_id: &str) -> Result<f32, anyhow::Error> {
        self.inner.lock().await.add_audio(audio_tag_id).await
    }

    pub fn play(&self) -> Result<(), anyhow::Error> {
        self.inner.blocking_lock().play()
    }

    pub fn stop(&self) {
        self.inner.blocking_lock().remove_audio()
    }

    pub fn pause(&self) -> Result<(), anyhow::Error> {
        self.inner.blocking_lock().pause()
    }

    pub fn get_playback_position(&self) -> f32 {
        self.inner.blocking_lock().get_playback_position()
    }

    pub fn set_playback_position(&self) -> Result<(), anyhow::Error> {
        todo!()
    }

    pub fn get_remain_sample_buffer_sec(&self) -> f32 {
        self.inner.blocking_lock().get_remain_sample_buffer_sec()
    }

    pub fn get_status(&self) -> AudioPlayerStatus {
        self.inner.blocking_lock().status
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        for thread_run_state in &self.thread_run_states {
            thread_run_state.store(false, Ordering::Relaxed);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum AudioPlayerStatus {
    Play,
    Pause,
    Stop,
    Error,
}

impl From<usize> for AudioPlayerStatus {
    //ref: https://gist.github.com/polypus74/eabc7bb00873e6b90abe230f9e632989
    fn from(value: usize) -> Self {
        use self::AudioPlayerStatus::*;
        match value {
            0 => Play,
            1 => Pause,
            2 => Stop,
            3 => Error,
            _ => unreachable!(),
        }
    }
}

pub struct AudioPlayerInner {
    ctx: AudioContext,
    streams: VecDeque<AudioStream>,
    tx: mpsc::Sender<&'static str>,
    status: AudioPlayerStatus,
}

impl AudioPlayerInner {
    pub fn new(
        tx: mpsc::Sender<&'static str>,
    ) -> Self {
        let ctx = AudioContext::new().unwrap();

        Self {
            ctx,
            streams: VecDeque::new(),
            tx,
            status: AudioPlayerStatus::Stop,
        }
    }

    pub async fn add_audio(&mut self, audio_tag_id: &str) -> Result<f32, anyhow::Error> {
        let audio_source = AudioSource::new(audio_tag_id).await?;
        let audio_stream = AudioStream::new(
            &self.ctx, 
            audio_source, 
            self.tx.clone()
        )?;

        let content_length = audio_stream.inner.audio_sample.content_length;

        self.streams.push_back(audio_stream);
        println!("done add audio");

        Ok(content_length)
    }

    pub fn remove_audio(&mut self) {
        // self.streams.remove(0).unwrap();
        match self.streams.remove(0) {
            Some(_) => (),
            None => (),
        }
    }

    pub fn play(&mut self) -> Result<(), anyhow::Error> {
        println!("play audio");

        let current_stream = self.streams.front().unwrap();
        // current_stream.play()?;

        match current_stream.play() {
            Ok(_) => self.status = AudioPlayerStatus::Play,
            Err(e) => {
                self.status = AudioPlayerStatus::Error;

                return Err(anyhow!(e))
            }
        }

        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), anyhow::Error> {
        println!("pause audio");

        let current_stream = self.streams.front().unwrap();
        // current_stream.pause()?;
        match current_stream.pause() {
            Ok(_) => self.status = AudioPlayerStatus::Pause,
            Err(e) => {
                self.status = AudioPlayerStatus::Error;

                return Err(anyhow!(e))
            }
        }

        Ok(())
    }

    pub fn get_playback_position(&self) -> f32 {
        match self.streams.front() {
            Some(stream) => return stream.inner.audio_sample.get_current_playback_position_sec(),
            None => return 0.0,
        }
        // let current_stream = self.streams.front().unwrap();
        
        // Ok(current_stream.inner.audio_sample.get_current_playback_position_sec())
    }

    pub fn get_remain_sample_buffer_sec(&self) -> f32 {
        match self.streams.front() {
            Some(stream) => return stream.inner.audio_sample.get_remain_sample_buffer_sec(),
            None => return 0.0,
        }
        // let current_stream = self.streams.front().unwrap();

        // Ok(current_stream.inner.audio_sample.get_remain_sample_buffer_sec())
    }

    pub fn set_playback_position(&self, position_sec: f32) -> Result<(), anyhow::Error> {
        todo!()
    }
}

struct AudioContext {
    device: cpal::Device,
    stream_config: cpal::StreamConfig,
}

impl AudioContext {
    fn new() -> Result<Self, anyhow::Error> {
        let host = cpal::default_host();
    
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::Error::msg("Default output device is not available"))?;
        
        println!("Output device: {}", device.name()?);
    
        let config: cpal::StreamConfig = device.default_output_config()?.into();

        println!("Output stream properties: sample_rate: {}, channel(s): {}", 
                 config.sample_rate.0, config.channels);

        Ok(Self {
            device,
            stream_config: config,
        })
    }
}

#[derive(Debug, PartialEq)]
enum AudioSampleStatus {
    Init,
    FillBuffer,
    Play,
}

impl From<usize> for AudioSampleStatus {
    //ref: https://gist.github.com/polypus74/eabc7bb00873e6b90abe230f9e632989
    fn from(value: usize) -> Self {
        use self::AudioSampleStatus::*;
        match value {
            0 => Init,
            1 => FillBuffer,
            2 => Play,
            _ => unreachable!(),
        }
    }
}

struct AudioSample {
    source: AudioSource,
    sample_buffer: Arc<RwLock<Vec<VecDeque<f32>>>>,
    current_sample_frame: Arc<AtomicUsize>,
    buffer_status: Arc<AtomicUsize>,
    last_buf_req_pos: Arc<AtomicUsize>,
    resampler: Arc<RwLock<rubato::FftFixedInOut<f32>>>,
    resampler_frames: usize,
    remain_sample_raw: Arc<RwLock<Vec<u8>>>,
    resampled_sample_frames: usize,
    host_sample_rate: u32,
    host_output_channels: usize,
    content_length: f32,
}

impl AudioSample {
    pub fn new(source: AudioSource, host_sample_rate: u32, host_output_channels: usize) -> Self {
        let resampler = rubato::FftFixedInOut::new(
            source.metadata.sample_rate as usize, 
            host_sample_rate as usize, 
            1024, 
            2
        ).unwrap();

        let resampler_frames = resampler.input_frames_next();
        let mut sample_buffer: Vec<VecDeque<f32>> = Vec::with_capacity(2);

        for _ in 0..host_output_channels {
            sample_buffer.push(VecDeque::new());
        }

        let resampled_sample_frames = (source.metadata.sample_frames as f32 * (host_sample_rate as f32 / source.metadata.sample_rate as f32)).ceil() as usize;
        let content_length = resampled_sample_frames as f32 / host_sample_rate as f32;

        Self {
            source,
            sample_buffer: Arc::new(RwLock::new(sample_buffer)),
            current_sample_frame: Arc::new(AtomicUsize::new(0)),
            buffer_status: Arc::new(AtomicUsize::new(AudioSampleStatus::Init as usize)),
            last_buf_req_pos: Arc::new(AtomicUsize::new(0)),
            resampler: Arc::new(
                RwLock::new(
                    resampler
                )
            ),
            resampler_frames,
            remain_sample_raw: Arc::new(RwLock::new(Vec::new())),
            resampled_sample_frames,
            host_sample_rate,
            host_output_channels,
            content_length,
        }
    }

    pub fn get_remain_sample_buffer_len(&self) -> usize {
        self.sample_buffer.read().unwrap()[0].len()
    }

    pub fn get_current_sample_idx(&self) -> usize {
        self.current_sample_frame.load(Ordering::SeqCst)
    }

    pub fn get_sec_from_sample_len(&self, sample_len: usize) -> f32 {
        sample_len as f32 / self.host_sample_rate as f32
    }

    pub fn get_current_playback_position_sec(&self) -> f32 {
        self.get_sec_from_sample_len(self.get_current_sample_idx())
    }

    pub fn get_remain_sample_buffer_sec(&self) -> f32 {
        self.get_sec_from_sample_len(self.get_remain_sample_buffer_len())   
    }

    pub async fn run_buffer_thread(
        self: Arc<Self>, 
        tx: mpsc::Sender<&'static str>,
    ) {
        let buffer_margin: f32 = 20.;
        let fetch_buffer_sec: f32 = 50.;

        tx.send("msg: run buffer thread via sender").await.unwrap();

        loop {
            // let current_sample_frame = self.get_current_sample_idx();
            // let current_pos = self.get_sec_from_sample_len(current_sample_frame);
            // let remain_sample_buffer = self.get_remain_sample_buffer_len();
            // let remain_sample_buffer_sec = self.get_sec_from_sample_len(remain_sample_buffer);

            if self.content_length - self.get_current_playback_position_sec() > buffer_margin {
                if buffer_margin > self.get_remain_sample_buffer_sec() {
                    println!("fetch audio sample buffer");
                    self.get_buffer_for(fetch_buffer_sec as u32 * 1000).await.unwrap();
                } else if self.get_remain_sample_buffer_len() == 0 {
                    self.buffer_status.store(AudioSampleStatus::FillBuffer as usize, Ordering::SeqCst);
                } else {
                    self.buffer_status.store(AudioSampleStatus::Play as usize, Ordering::SeqCst);
                }
            }            

            sleep(Duration::from_millis(5000)).await;
        }

    }

    pub async fn get_buffer_for(&self, ms: u32) -> Result<(), anyhow::Error> {
        let req_samples = ms * self.source.metadata.sample_rate / 1000;
        
        println!("request audio data part");
        let buffer_status = AudioSampleStatus::from(self.buffer_status.load(Ordering::SeqCst));
        let last_buf_req_pos = self.last_buf_req_pos.load(Ordering::SeqCst);

        let sample_res = match buffer_status {
            AudioSampleStatus::Init => {
                let resp = request::get_audio_data(
                    &self.source.id, 
                    0, 
                    req_samples).await.unwrap();

                self.last_buf_req_pos.store(req_samples as usize, Ordering::SeqCst);

                resp
            },

            AudioSampleStatus::FillBuffer | AudioSampleStatus::Play => {
                let resp = request::get_audio_data(
                    &self.source.id,
                    std::cmp::min(last_buf_req_pos as u32 + 1, self.source.metadata.sample_frames as u32),
                    std::cmp::min(last_buf_req_pos as u32 + req_samples, self.source.metadata.sample_frames as u32)).await.unwrap();

                self.last_buf_req_pos.store(req_samples as usize + last_buf_req_pos, Ordering::SeqCst);

                resp
            },
        };

        println!("parse audio data response as audio sample");
        // ref: https://users.rust-lang.org/t/convert-slice-u8-to-u8-4/63836
        
        let sample_res = sample_res.into_inner();
        let chunks_per_channel = 2;
        let channel = 2;

        let mut remain_sample_raw = self.remain_sample_raw.write().unwrap();
        
        let mut p_samples: Vec<u8> = remain_sample_raw.drain(..).collect();
        p_samples.extend_from_slice(&sample_res.content);
        
        let mut chunks_items_iter = p_samples.chunks_exact(chunks_per_channel * channel * self.resampler_frames);

        while let Some(chunk_items) = chunks_items_iter.next() {
            let mut input_buf: Vec<Vec<f32>> = Vec::with_capacity(channel);

            for _ in 0..channel {
                input_buf.push(Vec::with_capacity(self.resampler_frames));
            }

            let sample_items = chunk_items
                .chunks(2)
                .map(|item| i16::from_be_bytes(item.try_into().unwrap()) as f32 / self.host_sample_rate as f32)
                .collect::<Vec<f32>>();

            for sample_item in sample_items.chunks(2) {
                for channel_idx in 0..channel {
                    input_buf[channel_idx].push(sample_item[channel_idx]);
                }
            }

            let mut resampler = self.resampler.write().unwrap();
            let resampled_wave = resampler.process(input_buf.as_ref(), None).unwrap();

            let mut sample_buffer = self.sample_buffer.write().unwrap();

            for (ch_idx, channel_sample_buffer) in sample_buffer.iter_mut().enumerate() {
                channel_sample_buffer.extend(resampled_wave.get(ch_idx).unwrap());
            }
        }

        let remain_samples = chunks_items_iter.remainder();
        remain_sample_raw.extend(remain_samples);

        println!("done resampling wave data");

        Ok(())
    }
    
    fn play_for(&self, output: &mut [f32]) {
        let mut sample_buffer = self.sample_buffer.write().unwrap();
        let mut channel_sample_read: u8 = 0;

        for output_channel_frame in output.chunks_mut(self.host_output_channels) {  
            channel_sample_read = 0;
          
            for channel_idx in 0..self.host_output_channels {
                if let Some(channel_sample) = sample_buffer[channel_idx].pop_front() {
                    output_channel_frame[channel_idx] = channel_sample;
                    channel_sample_read += 1;
                } else {
                    output_channel_frame[channel_idx] = 0.0;
                    break;
                }
            }

            let current_sample_frame = self.current_sample_frame.load(Ordering::SeqCst);
            self.current_sample_frame.store(
                current_sample_frame + (channel_sample_read / self.host_output_channels as u8) as usize, 
                Ordering::SeqCst
            );
        }
    }
}

struct AudioStream {
    inner: Arc<AudioStreamInner>,
    thread_handles: Vec<JoinHandle<()>>,
}

impl AudioStream {
    pub fn new(
        ctx: &AudioContext, 
        source: AudioSource, 
        tx: mpsc::Sender<&'static str>
    ) -> Result<Self, anyhow::Error> {
        let inner = AudioStreamInner::new(
            ctx, 
            source, 
            tx
        )?;
        let inner = Arc::new(inner);

        let mut thread_handles: Vec<JoinHandle<()>> = Vec::new();

        let _audio_sample_1 = inner.audio_sample.clone();
        let _tx_1 = inner.tx.clone();
        let buffer_thread = tokio::spawn(async move {
            _audio_sample_1.run_buffer_thread(_tx_1).await;
        });
        thread_handles.push(buffer_thread);

        let _inner_1 = inner.clone();
        let _tx_2 = inner.tx.clone();

        let playback_manage_thread = tokio::spawn(async move {
            _inner_1.manage_playback(_tx_2).await;
        });
        thread_handles.push(playback_manage_thread);

        Ok(Self {
            inner,
            thread_handles,
        })
    }

    pub fn play(&self) -> Result<(), cpal::PlayStreamError> {
        self.inner.stream.play()
    }

    pub fn pause(&self) -> Result<(), cpal::PauseStreamError> {
        self.inner.stream.pause()
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        for thread_handle in &self.thread_handles {
            thread_handle.abort();
        }
    }
}

struct AudioStreamInner {
    stream: cpal::Stream,
    audio_sample: Arc<AudioSample>,
    tx: mpsc::Sender<&'static str>,
}

unsafe impl Send for AudioStreamInner {}
unsafe impl Sync for AudioStreamInner {}

impl AudioStreamInner {
    pub fn new(
        ctx: &AudioContext, 
        source: AudioSource, 
        tx: mpsc::Sender<&'static str>
    ) -> Result<Self, anyhow::Error> {
        let host_output_sample_rate = ctx.stream_config.sample_rate.0;
        let host_output_channels = ctx.stream_config.channels;

        let audio_sample = Arc::new(
            AudioSample::new(
                source,
                host_output_sample_rate,
                host_output_channels as usize
            )
        );

        let sample_play_err_fn = |err: cpal::StreamError| {
            println!("an error occured on stream: {}", err);
        };

        let _audio_sample_1 = audio_sample.clone();

        let stream = ctx.device.build_output_stream(
            &ctx.stream_config,
            move |output: &mut [f32], _: &cpal::OutputCallbackInfo| {
                _audio_sample_1.play_for(output)
            }, 
            sample_play_err_fn
        )?;
        stream.pause().unwrap();

        let audio_stream = Self {
            stream,
            audio_sample,
            tx,
        };

        Ok(audio_stream)
    }

    async fn manage_playback(
        self: Arc<Self>, 
        tx: mpsc::Sender<&'static str>,
    ) {
        loop {
            // let current_sample_frame = self.audio_sample.get_current_sample_idx();
            // let current_pos = self.audio_sample.get_sec_from_sample_len(current_sample_frame);
            // let remain_sample_buffer = self.audio_sample.get_remain_sample_buffer_len();
            // let remain_sample_buffer_sec = self.audio_sample.get_sec_from_sample_len(remain_sample_buffer);

            // println!(
            //     "current pos: {:.2}s\tplayed samples: {}/{}\tremain sample buffer: {:.2}s",
            //     self.audio_sample.get_current_playback_position_sec(),
            //     self.audio_sample.get_current_sample_idx(),
            //     self.audio_sample.resampled_sample_frames,
            //     self.audio_sample.get_remain_sample_buffer_sec()
            // );

            // let sample_buffer_length = self.audio_sample.get_remain_sample_buffer_len();
            // let sample_buffer_sec = self.audio_sample.get_sec_from_sample_len(sample_buffer_length);

            if self.audio_sample.get_remain_sample_buffer_sec() < 0.01 {
                self.stream.pause().unwrap();

                if self.audio_sample.content_length - self.audio_sample.get_current_playback_position_sec() <= 0.5  {
                    println!("reach end of content");
                    tx.send("stop").await.unwrap();

                    break;
                }
            } else {
                self.stream.play().unwrap();
            }

            sleep(Duration::from_millis(10)).await;
        }
    }
}

struct AudioSource {
    id: String,
    metadata: AudioSourceMetadata,
}

impl AudioSource {
    async fn new(audio_tag_id: &str) -> Result<Self, anyhow::Error> {
        let metadata_res = request::get_audio_meta(audio_tag_id).await.unwrap().into_inner();

        let metadata = AudioSourceMetadata {
            bit_rate: metadata_res.bit_rate,
            sample_rate: metadata_res.sample_rate,
            channels: metadata_res.channels as usize,
            content_bytes: metadata_res.size,
            sample_frames: metadata_res.sample_frames as usize,
        };

        Ok(Self {
            id: audio_tag_id.to_string(),
            metadata,
        })
    }
}

struct AudioSourceMetadata {
    bit_rate: u32,
    sample_rate: u32,
    channels: usize,
    content_bytes: u32,
    sample_frames: usize,
}

impl AudioSourceMetadata {
    fn new(bit_rate: u32, sample_rate: u32, channels: usize, content_bytes: u32, sample_frames: usize) -> Self {
        Self {
            bit_rate,
            sample_rate,
            channels,
            content_bytes,
            sample_frames,
        }
    }
}