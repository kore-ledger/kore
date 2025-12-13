//

// Protobuf generated code for the sink service.
pub mod sink {
  tonic::include_proto!("sink");
}

use sink::{Event, Subject, sink_service_server::SinkService};
use tonic::{transport::Server, Request, Response, Status};
use tokio_stream::wrappers::ReceiverStream;

use std::collections::{HashMap, VecDeque};

pub struct SequenceEvent {
    pub sequence: u64,
    pub event: Vec<u8>,
}

#[derive(Debug)]
pub struct KoreSinkServer {
    /// In-memory storage of events per subject.
    events: HashMap<String, VecDeque<(u64, Vec<u8>)>>,
}

