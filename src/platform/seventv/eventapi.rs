use std::marker::PhantomData;

use either::Either;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, Visitor},
    ser::SerializeMap,
};
use tokio::sync::mpsc;
use ulid::Ulid;

type WebSocket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde_repr::Serialize_repr,
    serde_repr::Deserialize_repr,
)]
#[repr(u8)]
enum OpCode {
    Dispatch = 0,
    Hello = 1,
    Heartbeat = 2,
    Reconnect = 4,
    Ack = 5,
    Error = 6,
    EndOfStream = 7,
    Identity = 33,
    Resume = 34,
    Subscribe = 35,
    Unsubscribe = 36,
    Signal = 37,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde_repr::Serialize_repr,
    serde_repr::Deserialize_repr,
)]
#[repr(u16)]
pub enum CloseCode {
    ServerError = 4000,
    UnknownOperation = 4001,
    InvalidPayload = 4002,
    AuthFailure = 4003,
    AlreadyIdentified = 4004,
    RateLimit = 4005,
    Restart = 4006,
    Maintenance = 4007,
    Timeout = 4008,
    AlreadySubscribed = 4009,
    NotSubscribed = 4010,
    InsufficientPrivilege = 4011,
    Reconnect = 4012,
    NormalClosure = 4013,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct EventApiMessage {
    d: EventApiPayload,
    op: OpCode,
    t: i64,
}

macro_rules! thingi {
    ($($op:ident),+) => {
        impl EventApiMessage {
            fn is_opcode_right(&self) -> bool {
                match self.d {
                    $(EventApiPayload::$op(..) => self.op == OpCode::$op),+
                }
            }
        }

        #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
        #[serde(untagged)]
        enum EventApiPayload {
            $($op($op)),+
        }

        $(impl From<$op> for EventApiPayload {
            fn from(value: $op) -> EventApiPayload {
                EventApiPayload::$op(value)
            }
        })+

        $(impl From<$op> for EventApiMessage {
            fn from(value: $op) -> EventApiMessage {
                EventApiMessage {
                    d: value.into(),
                    op: OpCode::$op,
                    t: 0
                }
            }
        })+
    };
}

thingi!(Dispatch, Hello);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct Dispatch {
    #[serde(rename = "type")]
    event_type: String,
    body: ChangeMap,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct ChangeMap {
    id: Ulid,
    contextual: Option<bool>,
    actor: serde_json::Value,
    added: Option<Vec<ChangeField>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct ChangeField {
    key: String,
    index: Option<i64>,
    nested: bool,
    old_value: Option<serde_json::Value>,
    value: Option<Either<Vec<ChangeField>, serde_json::Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
struct Hello {
    heartbeat_interval: u32,
    session_id: String,
    subscription_limit: i32,
}

pub enum EventApiError {
    Io(std::io::Error),
}

enum ActorRequest {}

enum ActorResponse {}

pub struct EventApiClient {
    handle: tokio::task::JoinHandle<EventApiError>,
    tx: mpsc::Sender<ActorRequest>,
}

impl EventApiClient {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(128);
        let handle = tokio::spawn(async move {
            let (socket, _resp) =
                match tokio_tungstenite::connect_async("wss://events.7tv.io/v3").await {
                    Ok(s) => s,
                    Err(e) => todo!(),
                };

            todo!()
        });
        Self { handle, tx }
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use crate::platform::seventv::eventapi::{EventApiMessage, EventApiPayload, Hello, OpCode};

    #[test]
    fn ser() {
        let hello = EventApiMessage {
            t: 0,
            d: EventApiPayload::Hello(Hello {
                heartbeat_interval: 1000,
                session_id: "aaa".into(),
                subscription_limit: 100,
            }),
            op: OpCode::Hello,
        };

        let expected = json!({
            "t": 0,
            "op": OpCode::Hello,
            "d": {
                "heartbeat_interval": 1000,
                "session_id": "aaa",
                "subscription_limit": 100,
            }
        });

        assert_eq!(serde_json::to_value(hello).unwrap(), expected)
    }

    #[test]
    fn deser() {
        let hello = EventApiMessage {
            t: 0,
            d: EventApiPayload::Hello(Hello {
                heartbeat_interval: 1000,
                session_id: "aaa".into(),
                subscription_limit: 100,
            }),
            op: OpCode::Hello,
        };

        let expected = json!({
            "t": 0,
            "op": OpCode::Hello,
            "d": {
                "heartbeat_interval": 1000,
                "session_id": "aaa",
                "subscription_limit": 100,
            }
        });

        assert_eq!(
            hello,
            serde_json::from_str(&serde_json::to_string(&expected).unwrap()).unwrap()
        )
    }
}
