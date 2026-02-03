use mediasoup::prelude::*;
use serde::{Deserialize, Serialize};

/// Incoming message from client
/// Uses adjacently tagged enum to handle both messages with and without data
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "camelCase")]
pub enum ClientMessage {
    Join { data: JoinData },
    GetRouterRtpCapabilities,
    CreateTransport { data: CreateTransportData },
    ConnectTransport { data: ConnectTransportData },
    Produce { data: ProduceData },
    Consume { data: ConsumeData },
    ResumeConsumer { data: ResumeConsumerData },
    #[serde(alias = "getProducers")]
    GetProducers { #[serde(default)] data: Option<serde_json::Value> },
    ChatMessage { data: ChatMessageData },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinData {
    pub room_id: String,
    pub peer_id: String,
    pub name: String,
    pub is_teacher: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTransportData {
    pub direction: TransportDirection,
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TransportDirection {
    Send,
    Recv,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectTransportData {
    pub direction: TransportDirection,
    pub dtls_parameters: DtlsParameters,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProduceData {
    pub kind: MediaKind,
    pub rtp_parameters: RtpParameters,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumeData {
    pub producer_id: String,
    pub rtp_capabilities: RtpCapabilities,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeConsumerData {
    pub consumer_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageData {
    pub content: String,
    pub timestamp: String,
}

/// Outgoing message to client
#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum ServerMessage {
    Error(ErrorData),
    Joined(JoinedData),
    RouterRtpCapabilities(RtpCapabilitiesFinalized),
    TransportCreated(TransportCreatedData),
    TransportConnected(TransportConnectedData),
    Produced(ProducedData),
    Consumed(ConsumedData),
    ConsumerResumed(ConsumerResumedData),
    Producers(Vec<ProducerInfo>),
    PeerJoined(PeerJoinedData),
    PeerLeft(PeerLeftData),
    NewProducer(NewProducerData),
    ChatMessage(ChatMessageBroadcast),
}

#[derive(Debug, Serialize)]
pub struct ErrorData {
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JoinedData {
    pub room_id: String,
    pub peer_id: String,
    pub is_teacher: bool,
    pub rtp_capabilities: RtpCapabilitiesFinalized,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportCreatedData {
    pub direction: TransportDirection,
    pub id: String,
    pub ice_parameters: IceParameters,
    pub ice_candidates: Vec<IceCandidate>,
    pub dtls_parameters: DtlsParameters,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportConnectedData {
    pub direction: TransportDirection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducedData {
    pub producer_id: String,
    pub kind: MediaKind,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumedData {
    pub consumer_id: String,
    pub producer_id: String,
    pub kind: MediaKind,
    pub rtp_parameters: RtpParameters,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsumerResumedData {
    pub consumer_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProducerInfo {
    pub producer_id: String,
    pub kind: MediaKind,
    pub peer_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerJoinedData {
    pub peer_id: String,
    pub name: String,
    pub is_teacher: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerLeftData {
    pub peer_id: String,
    pub was_teacher: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProducerData {
    pub producer_id: String,
    pub kind: MediaKind,
    pub peer_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageBroadcast {
    pub sender_id: String,
    pub sender_name: String,
    pub content: String,
    pub timestamp: String,
    pub is_teacher: bool,
}
