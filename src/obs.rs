use crate::image::{Screenshot, OWContext};

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use websocket::client::sync::Client;
use websocket::stream::sync::TcpStream;
use websocket::ws::dataframe::DataFrame;
use websocket::{ClientBuilder, Message};

pub struct OBSClient {
    client: Client<TcpStream>,
    orig_dir: Option<String>,
}

impl OBSClient {
    pub fn new() -> OBSClient {
        let client = ClientBuilder::new("ws://localhost:4444")
            .unwrap()
            .connect_insecure()
            .unwrap();
        let mut result = OBSClient {
            client,
            orig_dir: None,
        };
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
            panic!("OBS WebSocket failure: {}", response["error"]);
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

    fn get_output_dir(&mut self) -> String {
        let response = self.send_request(json!({
            "request-type": "GetRecordingFolder",
        }));
        response["rec-folder"]
            .as_str()
            .expect("Recording folder was not a string!")
            .to_string()
    }

    fn set_output_dir(&mut self, output_dir: &str) {
        self.send_request(json!({
            "request-type": "SetRecordingFolder",
            "rec-folder": output_dir,
        }));
    }

    pub fn use_subdir(&mut self) -> PathBuf {
        let orig_dir = self.get_output_dir();
        let timestamp = crate::timestamp();
        let mut new_dir = PathBuf::from(&orig_dir);
        new_dir.push(timestamp);
        fs::create_dir(&new_dir).expect("Failed to create recording subdirectory");
        self.orig_dir = Some(orig_dir);
        self.set_output_dir(new_dir.to_str().expect("Failed to record in subdirectory"));
        new_dir
    }

    pub fn get_screenshot<C: OWContext>(&mut self) -> Screenshot<C> {
        let response = self.send_request(json!({
            "request-type": "GetCurrentScene",
        }));
        let scene_name = response["name"]
            .as_str()
            .expect("Scene name was not a string!")
            .to_string();
        let response = self.send_request(json!({
            "request-type": "TakeSourceScreenshot",
            "sourceName": scene_name,
            "embedPictureFormat": "png",
            "width": 1920,
            "height": 1080,
        }));
        let data = response["img"].as_str().expect("Screenshot data URI was not a string!");
        Screenshot::new(data)
    }
}

impl Drop for OBSClient {
    fn drop(&mut self) {
        if let Some(ref orig_dir) = self.orig_dir {
            let orig_dir = orig_dir.clone();
            self.set_output_dir(&orig_dir);
        }
    }
}
