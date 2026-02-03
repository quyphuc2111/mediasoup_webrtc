use mediasoup::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Peer in a room
#[derive(Debug)]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub is_teacher: bool,
    pub send_transport: RwLock<Option<WebRtcTransport>>,
    pub recv_transport: RwLock<Option<WebRtcTransport>>,
    pub producers: RwLock<HashMap<ProducerId, Producer>>,
    pub consumers: RwLock<HashMap<ConsumerId, Consumer>>,
}

impl Peer {
    pub fn new(id: String, name: String, is_teacher: bool) -> Self {
        Self {
            id,
            name,
            is_teacher,
            send_transport: RwLock::new(None),
            recv_transport: RwLock::new(None),
            producers: RwLock::new(HashMap::new()),
            consumers: RwLock::new(HashMap::new()),
        }
    }

    pub fn close(&self) {
        // Close all producers
        for producer in self.producers.write().drain() {
            drop(producer);
        }
        // Close all consumers
        for consumer in self.consumers.write().drain() {
            drop(consumer);
        }
        // Close transports
        if let Some(transport) = self.send_transport.write().take() {
            drop(transport);
        }
        if let Some(transport) = self.recv_transport.write().take() {
            drop(transport);
        }
    }
}

/// Room containing peers
pub struct Room {
    pub id: String,
    pub router: Router,
    peers: RwLock<HashMap<String, Arc<Peer>>>,
    teacher_id: RwLock<Option<String>>,
}

impl Room {
    pub fn new(router: Router, room_id: Option<String>) -> Self {
        Self {
            id: room_id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            router,
            peers: RwLock::new(HashMap::new()),
            teacher_id: RwLock::new(None),
        }
    }

    pub fn rtp_capabilities(&self) -> RtpCapabilitiesFinalized {
        self.router.rtp_capabilities().clone()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.read().len()
    }

    pub fn has_teacher(&self) -> bool {
        self.teacher_id.read().is_some()
    }

    pub fn add_peer(&self, id: String, name: String, is_teacher: bool) -> Arc<Peer> {
        let peer = Arc::new(Peer::new(id.clone(), name.clone(), is_teacher));
        self.peers.write().insert(id.clone(), peer.clone());

        if is_teacher {
            *self.teacher_id.write() = Some(id.clone());
        }

        tracing::info!(
            "[Room {}] Peer joined: {} ({})",
            self.id,
            name,
            if is_teacher { "Teacher" } else { "Student" }
        );

        peer
    }

    pub fn get_peer(&self, id: &str) -> Option<Arc<Peer>> {
        self.peers.read().get(id).cloned()
    }

    pub fn remove_peer(&self, id: &str) -> Option<Arc<Peer>> {
        let peer = self.peers.write().remove(id);

        if let Some(ref p) = peer {
            p.close();

            if p.is_teacher {
                *self.teacher_id.write() = None;
            }

            tracing::info!("[Room {}] Peer left: {}", self.id, p.name);
        }

        peer
    }

    pub fn get_all_peers(&self) -> Vec<Arc<Peer>> {
        self.peers.read().values().cloned().collect()
    }

    pub fn get_students(&self) -> Vec<Arc<Peer>> {
        self.peers
            .read()
            .values()
            .filter(|p| !p.is_teacher)
            .cloned()
            .collect()
    }

    pub fn get_teacher(&self) -> Option<Arc<Peer>> {
        let teacher_id = self.teacher_id.read().clone()?;
        self.peers.read().get(&teacher_id).cloned()
    }

    /// Get teacher's producers
    pub fn teacher_producers(&self) -> Vec<Producer> {
        self.get_teacher()
            .map(|t| t.producers.read().values().cloned().collect())
            .unwrap_or_default()
    }

    /// Find producer by ID across all peers
    pub fn find_producer(&self, producer_id: &ProducerId) -> Option<(Producer, Arc<Peer>)> {
        for peer in self.peers.read().values() {
            if let Some(producer) = peer.producers.read().get(producer_id) {
                return Some((producer.clone(), peer.clone()));
            }
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.peers.read().is_empty()
    }

    pub fn close(&self) {
        for peer in self.peers.write().drain() {
            peer.1.close();
        }
        tracing::info!("[Room {}] Closed", self.id);
    }
}
