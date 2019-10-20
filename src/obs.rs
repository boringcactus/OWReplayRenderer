use serde_json::Value;
use websocket::client::sync::Client;
use websocket::stream::sync::TcpStream;
use websocket::ws::dataframe::DataFrame;
use websocket::{ClientBuilder, Message};

pub struct OBSClient {
    client: Client<TcpStream>,
}

impl OBSClient {
    pub fn new() -> OBSClient {
        let client = ClientBuilder::new("ws://localhost:4444")
            .unwrap()
            .connect_insecure()
            .unwrap();
        let mut result = OBSClient { client };
        result.send_request(json!({
            "request-type": "SetHeartbeat",
            "enable": false,
        }));
        result
    }

    fn recv(&mut self) -> Value {
        let response = self.client.recv_message().unwrap();
        let response: Value = serde_json::from_slice(response.take_payload().as_slice()).unwrap();
        // ignore heartbeats or other update events
        if response.as_object().unwrap().contains_key("update-type") {
            return self.recv();
        }
        response
    }

    fn send_request(&mut self, mut request: Value) -> Value {
        let message_id = "x";
        request["message-id"] = Value::String(message_id.to_string());
        let request = Message::text(serde_json::to_string(&request).unwrap());
        self.client.send_message(&request).unwrap();
        let response = self.recv();
        let status = response["status"].as_str().unwrap();
        if status == "error" {
            eprintln!("OBS WebSocket failure: {}", response["error"]);
            std::process::exit(1);
        }
        response
    }

    pub fn start_recording(&mut self) {
        self.send_request(json!({
            "request-type": "StartRecording",
        }));
    }

    pub fn stop_recording(&mut self) {
        self.send_request(json!({
            "request-type": "StopRecording",
        }));
    }
}
