use crate::manager::MediasoupManager;
use crate::messages::*;
use futures_util::{SinkExt, StreamExt};
use mediasoup::prelude::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_tungstenite::{accept_async, tungstenite::Message};

type Tx = mpsc::UnboundedSender<Message>;

/// Client connection info
struct ClientInfo {
    peer_id: String,
    room_id: String,
    tx: Tx,
}

/// Signaling server
pub struct SignalingServer {
    manager: Arc<MediasoupManager>,
    clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
}

impl SignalingServer {
    pub fn new(manager: Arc<MediasoupManager>) -> Self {
        Self {
            manager,
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the signaling server
    pub async fn run(&self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let addr = format!("0.0.0.0:{}", port);
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("Signaling server listening on port {}", port);

        loop {
            let (stream, addr) = listener.accept().await?;
            tracing::info!("New WebSocket connection from {}", addr);

            let manager = self.manager.clone();
            let clients = self.clients.clone();

            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, addr, manager, clients).await {
                    tracing::error!("Connection error: {}", e);
                }
            });
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    manager: Arc<MediasoupManager>,
    clients: Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    // Spawn task to forward messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming messages
    while let Some(msg) = ws_receiver.next().await {
        let msg = msg?;

        if let Message::Text(text) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(client_msg) => {
                    if let Err(e) =
                        handle_message(addr, client_msg, &manager, &clients, &tx).await
                    {
                        tracing::error!("Error handling message: {}", e);
                        send_error(&tx, &e.to_string());
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to parse message: {}", e);
                    send_error(&tx, "Invalid message format");
                }
            }
        }
    }

    // Cleanup on disconnect
    handle_disconnect(addr, &manager, &clients).await;
    send_task.abort();

    Ok(())
}

fn send_message(tx: &Tx, message: &ServerMessage) {
    if let Ok(json) = serde_json::to_string(message) {
        let _ = tx.send(Message::Text(json));
    }
}

fn send_error(tx: &Tx, message: &str) {
    send_message(
        tx,
        &ServerMessage::Error(ErrorData {
            message: message.to_string(),
        }),
    );
}

fn broadcast_to_room(
    clients: &RwLock<HashMap<SocketAddr, ClientInfo>>,
    room_id: &str,
    message: &ServerMessage,
    exclude: Option<SocketAddr>,
) {
    let json = serde_json::to_string(message).unwrap();
    let clients = clients.read();

    for (addr, info) in clients.iter() {
        if info.room_id == room_id && Some(*addr) != exclude {
            let _ = info.tx.send(Message::Text(json.clone()));
        }
    }
}

async fn handle_message(
    addr: SocketAddr,
    message: ClientMessage,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match message {
        ClientMessage::Join { data } => {
            handle_join(addr, data, manager, clients, tx).await?;
        }
        ClientMessage::GetRouterRtpCapabilities => {
            handle_get_router_rtp_capabilities(addr, manager, clients, tx)?;
        }
        ClientMessage::CreateTransport { data } => {
            handle_create_transport(addr, data, manager, clients, tx).await?;
        }
        ClientMessage::ConnectTransport { data } => {
            handle_connect_transport(addr, data, manager, clients, tx).await?;
        }
        ClientMessage::Produce { data } => {
            handle_produce(addr, data, manager, clients, tx).await?;
        }
        ClientMessage::Consume { data } => {
            handle_consume(addr, data, manager, clients, tx).await?;
        }
        ClientMessage::ResumeConsumer { data } => {
            handle_resume_consumer(addr, data, manager, clients, tx).await?;
        }
        ClientMessage::GetProducers { .. } => {
            handle_get_producers(addr, manager, clients, tx)?;
        }
        ClientMessage::ChatMessage { data } => {
            handle_chat_message(addr, data, manager, clients)?;
        }
    }
    Ok(())
}

async fn handle_join(
    addr: SocketAddr,
    data: JoinData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let room = manager.get_or_create_room(&data.room_id).await?;

    // Check if room already has teacher
    if data.is_teacher && room.has_teacher() {
        send_error(tx, "Room already has a teacher");
        return Ok(());
    }

    // Check max clients
    if room.peer_count() >= manager.max_clients_per_room() {
        send_error(tx, "Room is full");
        return Ok(());
    }

    room.add_peer(data.peer_id.clone(), data.name.clone(), data.is_teacher);

    // Store client info
    clients.write().insert(
        addr,
        ClientInfo {
            peer_id: data.peer_id.clone(),
            room_id: data.room_id.clone(),
            tx: tx.clone(),
        },
    );

    // Send joined response
    send_message(
        tx,
        &ServerMessage::Joined(JoinedData {
            room_id: room.id.clone(),
            peer_id: data.peer_id.clone(),
            is_teacher: data.is_teacher,
            rtp_capabilities: room.rtp_capabilities(),
        }),
    );

    // Notify others
    broadcast_to_room(
        clients,
        &data.room_id,
        &ServerMessage::PeerJoined(PeerJoinedData {
            peer_id: data.peer_id.clone(),
            name: data.name.clone(),
            is_teacher: data.is_teacher,
        }),
        Some(addr),
    );

    tracing::info!(
        "Peer {} joined room {} as {}",
        data.name,
        data.room_id,
        if data.is_teacher { "Teacher" } else { "Student" }
    );

    Ok(())
}

fn handle_get_router_rtp_capabilities(
    addr: SocketAddr,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let info = clients.read().get(&addr).map(|i| i.room_id.clone());
    let room_id = info.ok_or("Not joined")?;

    let room = manager.get_room(&room_id).ok_or("Room not found")?;

    send_message(
        tx,
        &ServerMessage::RouterRtpCapabilities(room.rtp_capabilities()),
    );

    Ok(())
}

async fn handle_create_transport(
    addr: SocketAddr,
    data: CreateTransportData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    let (transport, params) = manager.create_webrtc_transport(&room).await?;

    match data.direction {
        TransportDirection::Send => {
            *peer.send_transport.write() = Some(transport);
        }
        TransportDirection::Recv => {
            *peer.recv_transport.write() = Some(transport);
        }
    }

    send_message(
        tx,
        &ServerMessage::TransportCreated(TransportCreatedData {
            direction: data.direction,
            id: params.id,
            ice_parameters: params.ice_parameters,
            ice_candidates: params.ice_candidates,
            dtls_parameters: params.dtls_parameters,
        }),
    );

    Ok(())
}

async fn handle_connect_transport(
    addr: SocketAddr,
    data: ConnectTransportData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    let transport = match data.direction {
        TransportDirection::Send => peer.send_transport.read().clone(),
        TransportDirection::Recv => peer.recv_transport.read().clone(),
    };

    let transport = transport.ok_or("Transport not found")?;
    MediasoupManager::connect_transport(&transport, data.dtls_parameters).await?;

    send_message(
        tx,
        &ServerMessage::TransportConnected(TransportConnectedData {
            direction: data.direction,
        }),
    );

    Ok(())
}

async fn handle_produce(
    addr: SocketAddr,
    data: ProduceData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    // Only teacher can produce video
    if !peer.is_teacher && data.kind == MediaKind::Video {
        send_error(tx, "Only teacher can share screen");
        return Ok(());
    }

    let transport = peer
        .send_transport
        .read()
        .clone()
        .ok_or("Send transport not found")?;

    let producer =
        MediasoupManager::create_producer(&transport, data.kind, data.rtp_parameters).await?;

    let producer_id = producer.id().to_string();
    peer.producers.write().insert(producer.id(), producer);

    send_message(
        tx,
        &ServerMessage::Produced(ProducedData {
            producer_id: producer_id.clone(),
            kind: data.kind,
        }),
    );

    // Notify others about new producer
    let new_producer_msg = ServerMessage::NewProducer(NewProducerData {
        producer_id: producer_id.clone(),
        kind: data.kind,
        peer_id: peer_id.clone(),
    });

    if peer.is_teacher {
        // Teacher produced - notify all students
        broadcast_to_room(clients, &room_id, &new_producer_msg, Some(addr));
        tracing::info!("Teacher produced {:?}: {}", data.kind, producer_id);
    } else if data.kind == MediaKind::Audio {
        // Student produced audio - notify teacher only
        let clients_read = clients.read();
        for (client_addr, info) in clients_read.iter() {
            if info.room_id == room_id && *client_addr != addr {
                if let Some(other_peer) = room.get_peer(&info.peer_id) {
                    if other_peer.is_teacher {
                        let _ = info
                            .tx
                            .send(Message::Text(serde_json::to_string(&new_producer_msg)?));
                        break;
                    }
                }
            }
        }
        tracing::info!("Student {} produced audio: {}", peer.name, producer_id);
    }

    Ok(())
}

async fn handle_consume(
    addr: SocketAddr,
    data: ConsumeData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    let producer_id: ProducerId = data.producer_id.parse()?;
    let (producer, producer_peer) = room
        .find_producer(&producer_id)
        .ok_or("Producer not found")?;

    // Students can only consume from teacher
    if !peer.is_teacher && !producer_peer.is_teacher {
        send_error(tx, "Students can only consume from teacher");
        return Ok(());
    }

    // Teacher can only consume audio from students
    if peer.is_teacher && producer_peer.is_teacher {
        send_error(tx, "Teacher cannot consume from self");
        return Ok(());
    }

    if peer.is_teacher && producer.kind() != MediaKind::Audio {
        send_error(tx, "Teacher can only consume audio from students");
        return Ok(());
    }

    let recv_transport = peer
        .recv_transport
        .read()
        .clone()
        .ok_or("Recv transport not found")?;

    let consumer = MediasoupManager::create_consumer(
        &room,
        &recv_transport,
        &producer,
        &data.rtp_capabilities,
    )
    .await?;

    if let Some(consumer) = consumer {
        let consumer_id = consumer.id().to_string();
        let rtp_parameters = consumer.rtp_parameters().clone();
        let kind = consumer.kind();

        peer.consumers.write().insert(consumer.id(), consumer);

        send_message(
            tx,
            &ServerMessage::Consumed(ConsumedData {
                consumer_id,
                producer_id: data.producer_id,
                kind,
                rtp_parameters,
            }),
        );
    } else {
        send_error(tx, "Cannot consume");
    }

    Ok(())
}

async fn handle_resume_consumer(
    addr: SocketAddr,
    data: ResumeConsumerData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    let consumer_id: ConsumerId = data.consumer_id.parse()?;
    let consumer = peer
        .consumers
        .read()
        .get(&consumer_id)
        .cloned()
        .ok_or("Consumer not found")?;

    consumer.resume().await?;

    send_message(
        tx,
        &ServerMessage::ConsumerResumed(ConsumerResumedData {
            consumer_id: data.consumer_id,
        }),
    );

    Ok(())
}

fn handle_get_producers(
    addr: SocketAddr,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
    tx: &Tx,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    let mut producers = Vec::new();

    if peer.is_teacher {
        // Teacher gets teacher's own + student audio
        for p in room.teacher_producers() {
            producers.push(ProducerInfo {
                producer_id: p.id().to_string(),
                kind: p.kind(),
                peer_id: peer_id.clone(),
            });
        }
        // Add student audio producers
        for student in room.get_students() {
            for p in student.producers.read().values() {
                if p.kind() == MediaKind::Audio {
                    producers.push(ProducerInfo {
                        producer_id: p.id().to_string(),
                        kind: p.kind(),
                        peer_id: student.id.clone(),
                    });
                }
            }
        }
    } else {
        // Students only get teacher's producers
        if let Some(teacher) = room.get_teacher() {
            for p in room.teacher_producers() {
                producers.push(ProducerInfo {
                    producer_id: p.id().to_string(),
                    kind: p.kind(),
                    peer_id: teacher.id.clone(),
                });
            }
        }
    }

    send_message(tx, &ServerMessage::Producers(producers));

    Ok(())
}

fn handle_chat_message(
    addr: SocketAddr,
    data: ChatMessageData,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (room_id, peer_id) = {
        let clients = clients.read();
        let info = clients.get(&addr).ok_or("Not joined")?;
        (info.room_id.clone(), info.peer_id.clone())
    };

    let room = manager.get_room(&room_id).ok_or("Room not found")?;
    let peer = room.get_peer(&peer_id).ok_or("Peer not found")?;

    broadcast_to_room(
        clients,
        &room_id,
        &ServerMessage::ChatMessage(ChatMessageBroadcast {
            sender_id: peer_id,
            sender_name: peer.name.clone(),
            content: data.content.clone(),
            timestamp: data.timestamp,
            is_teacher: peer.is_teacher,
        }),
        Some(addr),
    );

    tracing::info!("[Chat] {}: {}", peer.name, data.content);

    Ok(())
}

async fn handle_disconnect(
    addr: SocketAddr,
    manager: &Arc<MediasoupManager>,
    clients: &Arc<RwLock<HashMap<SocketAddr, ClientInfo>>>,
) {
    let info = clients.write().remove(&addr);

    if let Some(info) = info {
        if let Some(room) = manager.get_room(&info.room_id) {
            let peer = room.get_peer(&info.peer_id);
            let was_teacher = peer.as_ref().map(|p| p.is_teacher).unwrap_or(false);

            room.remove_peer(&info.peer_id);

            // Notify others
            broadcast_to_room(
                clients,
                &info.room_id,
                &ServerMessage::PeerLeft(PeerLeftData {
                    peer_id: info.peer_id.clone(),
                    was_teacher,
                }),
                None,
            );

            // Clean up empty room
            if room.is_empty() {
                manager.remove_room(&info.room_id);
            }
        }

        tracing::info!("Peer {} disconnected", info.peer_id);
    }
}
