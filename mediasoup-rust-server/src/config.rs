use mediasoup::prelude::*;
use mediasoup::worker::{WorkerLogLevel, WorkerLogTag, WorkerSettings};
use std::num::{NonZeroU32, NonZeroU8};

/// Server configuration
pub struct Config {
    pub listen_port: u16,
    pub num_workers: usize,
    pub max_clients_per_room: usize,
    pub max_incoming_bitrate: u32,
}

impl Default for Config {
    fn default() -> Self {
        let num_cpus = num_cpus::get().min(3);
        Self {
            listen_port: 3016,
            num_workers: num_cpus,
            max_clients_per_room: 50,
            max_incoming_bitrate: 6_000_000, // 6 Mbps
        }
    }
}

/// Create worker settings
pub fn worker_settings() -> WorkerSettings {
    let mut settings = WorkerSettings::default();
    settings.rtc_port_range = 40000..=45000;
    settings.log_level = WorkerLogLevel::Warn;
    settings.log_tags = vec![
        WorkerLogTag::Ice,
        WorkerLogTag::Dtls,
        WorkerLogTag::Rtp,
        WorkerLogTag::Rtcp,
    ];
    settings
}

/// Create router options with media codecs
pub fn router_options() -> RouterOptions {
    RouterOptions::new(media_codecs())
}

/// Media codecs for LAN streaming
fn media_codecs() -> Vec<RtpCodecCapability> {
    vec![
        // Audio - Opus
        RtpCodecCapability::Audio {
            mime_type: MimeTypeAudio::Opus,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(48000).unwrap(),
            channels: NonZeroU8::new(2).unwrap(),
            parameters: RtpCodecParametersParameters::from([
                ("useinbandfec", 1u32.into()),
                ("minptime", 10u32.into()),
            ]),
            rtcp_feedback: vec![],
        },
        // Video - H264 (Primary)
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::H264,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::from([
                ("packetization-mode", 1u32.into()),
                ("profile-level-id", "42e01f".into()),
                ("level-asymmetry-allowed", 1u32.into()),
                ("x-google-start-bitrate", 3000u32.into()),
                ("x-google-max-bitrate", 5000u32.into()),
            ]),
            rtcp_feedback: vec![],
        },
        // Video - VP8 (Fallback)
        RtpCodecCapability::Video {
            mime_type: MimeTypeVideo::Vp8,
            preferred_payload_type: None,
            clock_rate: NonZeroU32::new(90000).unwrap(),
            parameters: RtpCodecParametersParameters::from([
                ("x-google-start-bitrate", 2500u32.into()),
                ("x-google-max-bitrate", 4000u32.into()),
            ]),
            rtcp_feedback: vec![],
        },
    ]
}

/// Create WebRTC transport options
pub fn webrtc_transport_options(announced_ip: String) -> WebRtcTransportOptions {
    let listen_info = ListenInfo {
        protocol: Protocol::Udp,
        ip: "0.0.0.0".parse().unwrap(),
        announced_address: Some(announced_ip),
        port: None,
        port_range: None,
        flags: None,
        send_buffer_size: None,
        recv_buffer_size: None,
        expose_internal_ip: false,
    };

    let listen_infos = WebRtcTransportListenInfos::new(listen_info);

    let mut options = WebRtcTransportOptions::new(listen_infos);
    options.initial_available_outgoing_bitrate = 6_000_000;
    options
}

/// Get local IP address
pub fn get_local_ip() -> String {
    local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
