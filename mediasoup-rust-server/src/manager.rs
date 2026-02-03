use crate::config::{self, Config};
use crate::room::Room;
use mediasoup::prelude::*;
use mediasoup::worker_manager::WorkerManager;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Mediasoup manager handling workers and rooms
pub struct MediasoupManager {
    workers: Vec<Worker>,
    rooms: RwLock<HashMap<String, Arc<Room>>>,
    next_worker_index: AtomicUsize,
    config: Config,
    local_ip: String,
}

impl MediasoupManager {
    /// Initialize the manager with workers
    pub async fn new(config: Config) -> Result<Self, BoxError> {
        let local_ip = config::get_local_ip();
        tracing::info!("Creating {} mediasoup workers...", config.num_workers);

        let worker_manager = WorkerManager::new();
        let mut workers = Vec::with_capacity(config.num_workers);

        for i in 0..config.num_workers {
            let worker = worker_manager
                .create_worker(config::worker_settings())
                .await?;

            let worker_id = worker.id();
            worker
                .on_dead(move |_| {
                    tracing::error!("Worker {} died!", worker_id);
                })
                .detach();

            tracing::info!("Worker {} created [id: {}]", i, worker.id());
            workers.push(worker);
        }

        Ok(Self {
            workers,
            rooms: RwLock::new(HashMap::new()),
            next_worker_index: AtomicUsize::new(0),
            config,
            local_ip,
        })
    }

    fn get_next_worker(&self) -> &Worker {
        let index = self.next_worker_index.fetch_add(1, Ordering::Relaxed) % self.workers.len();
        &self.workers[index]
    }

    /// Create a new room
    pub async fn create_room(
        &self,
        room_id: Option<String>,
    ) -> Result<Arc<Room>, BoxError> {
        let worker = self.get_next_worker();
        let router = worker.create_router(config::router_options()).await?;

        let room = Arc::new(Room::new(router, room_id));
        self.rooms.write().insert(room.id.clone(), room.clone());

        tracing::info!("Room created: {}", room.id);
        Ok(room)
    }

    /// Get existing room
    pub fn get_room(&self, room_id: &str) -> Option<Arc<Room>> {
        self.rooms.read().get(room_id).cloned()
    }

    /// Get or create room
    pub async fn get_or_create_room(
        &self,
        room_id: &str,
    ) -> Result<Arc<Room>, BoxError> {
        if let Some(room) = self.get_room(room_id) {
            return Ok(room);
        }
        self.create_room(Some(room_id.to_string())).await
    }

    /// Remove room
    pub fn remove_room(&self, room_id: &str) {
        if let Some(room) = self.rooms.write().remove(room_id) {
            room.close();
        }
    }

    /// Create WebRTC transport
    pub async fn create_webrtc_transport(
        &self,
        room: &Room,
    ) -> Result<(WebRtcTransport, TransportParams), BoxError> {
        let options = config::webrtc_transport_options(self.local_ip.clone());
        let transport = room.router.create_webrtc_transport(options).await?;

        // Set max incoming bitrate
        transport
            .set_max_incoming_bitrate(self.config.max_incoming_bitrate)
            .await?;

        let params = TransportParams {
            id: transport.id().to_string(),
            ice_parameters: transport.ice_parameters().clone(),
            ice_candidates: transport.ice_candidates().clone(),
            dtls_parameters: transport.dtls_parameters(),
        };

        Ok((transport, params))
    }

    /// Connect transport
    pub async fn connect_transport(
        transport: &WebRtcTransport,
        dtls_parameters: DtlsParameters,
    ) -> Result<(), BoxError> {
        transport.connect(WebRtcTransportRemoteParameters { dtls_parameters }).await?;
        Ok(())
    }

    /// Create producer
    pub async fn create_producer(
        transport: &WebRtcTransport,
        kind: MediaKind,
        rtp_parameters: RtpParameters,
    ) -> Result<Producer, BoxError> {
        let options = ProducerOptions::new(kind, rtp_parameters);
        let producer = transport.produce(options).await?;

        tracing::info!(
            "Producer {} created (kind: {:?})",
            producer.id(),
            producer.kind()
        );

        Ok(producer)
    }

    /// Create consumer
    pub async fn create_consumer(
        room: &Room,
        transport: &WebRtcTransport,
        producer: &Producer,
        rtp_capabilities: &RtpCapabilities,
    ) -> Result<Option<Consumer>, BoxError> {
        if !room
            .router
            .can_consume(&producer.id(), rtp_capabilities)
        {
            tracing::warn!("Cannot consume producer {}", producer.id());
            return Ok(None);
        }

        let mut options = ConsumerOptions::new(producer.id(), rtp_capabilities.clone());
        options.paused = true; // Start paused

        let consumer = transport.consume(options).await?;

        tracing::info!(
            "Consumer {} created for producer {}",
            consumer.id(),
            producer.id()
        );

        Ok(Some(consumer))
    }

    pub fn max_clients_per_room(&self) -> usize {
        self.config.max_clients_per_room
    }

    pub fn local_ip(&self) -> &str {
        &self.local_ip
    }

    pub fn listen_port(&self) -> u16 {
        self.config.listen_port
    }
}

/// Transport parameters for client
#[derive(Debug, Clone, serde::Serialize)]
pub struct TransportParams {
    pub id: String,
    #[serde(rename = "iceParameters")]
    pub ice_parameters: IceParameters,
    #[serde(rename = "iceCandidates")]
    pub ice_candidates: Vec<IceCandidate>,
    #[serde(rename = "dtlsParameters")]
    pub dtls_parameters: DtlsParameters,
}
