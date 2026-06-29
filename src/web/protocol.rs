use serde::{Deserialize, Serialize};

pub const FRAME_OUTPUT: u8 = 0x01;
pub const FRAME_INPUT: u8 = 0x02;
pub const FRAME_CONTROL: u8 = 0x03;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientFrame {
    Input(Vec<u8>),
    Control(ClientControl),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientControl {
    Resize { cols: u16, rows: u16 },
    Ping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerControl {
    Status {
        writable: bool,
        backspace: Vec<u8>,
        word_erase: Vec<u8>,
    },
    Error {
        message: String,
    },
    Pong,
}

pub fn encode_output(bytes: &[u8]) -> Vec<u8> {
    encode_binary(FRAME_OUTPUT, bytes)
}

pub fn encode_input(bytes: &[u8]) -> Vec<u8> {
    encode_binary(FRAME_INPUT, bytes)
}

pub fn encode_server_control(control: &ServerControl) -> anyhow::Result<Vec<u8>> {
    let payload = serde_json::to_vec(control)?;
    Ok(encode_binary(FRAME_CONTROL, &payload))
}

pub fn decode_client_frame(bytes: &[u8]) -> anyhow::Result<ClientFrame> {
    let Some((&kind, payload)) = bytes.split_first() else {
        anyhow::bail!("empty websocket frame");
    };

    match kind {
        FRAME_INPUT => Ok(ClientFrame::Input(payload.to_vec())),
        FRAME_CONTROL => {
            let control = serde_json::from_slice(payload)?;
            Ok(ClientFrame::Control(control))
        }
        other => anyhow::bail!("unsupported client frame type {other:#x}"),
    }
}

fn encode_binary(kind: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 1);
    out.push(kind);
    out.extend_from_slice(payload);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_output_uses_typed_binary_frame() {
        assert_eq!(encode_output(b"abc"), vec![FRAME_OUTPUT, b'a', b'b', b'c']);
    }

    #[test]
    fn client_input_decodes_from_typed_binary_frame() {
        let frame = decode_client_frame(&[FRAME_INPUT, b'a', b'b']).unwrap();
        assert_eq!(frame, ClientFrame::Input(b"ab".to_vec()));
    }

    #[test]
    fn client_resize_decodes_from_json_control_frame() {
        let payload = serde_json::to_vec(&ClientControl::Resize { cols: 80, rows: 24 }).unwrap();
        let mut bytes = vec![FRAME_CONTROL];
        bytes.extend(payload);

        let frame = decode_client_frame(&bytes).unwrap();
        assert_eq!(
            frame,
            ClientFrame::Control(ClientControl::Resize { cols: 80, rows: 24 })
        );
    }
}
