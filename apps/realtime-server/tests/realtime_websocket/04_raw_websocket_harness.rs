fn event_set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

enum Frame {
    Text(String),
    Close(u16),
    Other,
}

struct RawWebSocket {
    stream: tokio::net::TcpStream,
    buffered: Vec<u8>,
    seen_text: String,
}

impl RawWebSocket {
    async fn connect(address: SocketAddr, token: &str) -> Self {
        Self::connect_with_authentication(address, token, false).await
    }

    async fn connect_as_browser(address: SocketAddr, token: &str) -> Self {
        Self::connect_with_authentication(address, token, true).await
    }

    async fn connect_with_authentication(
        address: SocketAddr,
        token: &str,
        browser_protocol: bool,
    ) -> Self {
        let mut stream = tokio::net::TcpStream::connect(address)
            .await
            .expect("connect raw websocket");
        let authentication = if browser_protocol {
            format!("Sec-WebSocket-Protocol: trpg.realtime.v1, trpg.auth.{token}\r\n")
        } else {
            format!("Sec-WebSocket-Protocol: trpg.realtime.v1\r\nAuthorization: Bearer {token}\r\n")
        };
        let request = format!(
            "GET /ws/v1/campaigns/{CAMPAIGN}/rooms/{CAMPAIGN} HTTP/1.1\r\n\
             Host: {address}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             {authentication}\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .expect("write websocket handshake");
        let mut received = Vec::new();
        loop {
            let mut chunk = [0_u8; 1_024];
            let read = stream.read(&mut chunk).await.expect("read handshake");
            assert!(read > 0, "websocket handshake closed");
            received.extend_from_slice(&chunk[..read]);
            if let Some(end) = find_bytes(&received, b"\r\n\r\n") {
                let headers =
                    String::from_utf8(received[..end].to_vec()).expect("utf8 websocket response");
                assert!(
                    headers.starts_with("HTTP/1.1 101"),
                    "upgrade failed: {headers}"
                );
                assert!(
                    headers
                        .to_ascii_lowercase()
                        .contains("sec-websocket-protocol: trpg.realtime.v1"),
                    "subprotocol not negotiated: {headers}"
                );
                assert!(
                    !headers.contains(token),
                    "authentication subprotocol must not be echoed"
                );
                return Self {
                    stream,
                    buffered: received[end + 4..].to_vec(),
                    seen_text: String::new(),
                };
            }
        }
    }

    async fn expect_connected(&mut self) {
        let envelope = self.next_server().await;
        assert!(matches!(
            envelope.message,
            ServerMessage::Connected { ref binding, .. }
                if binding.tenant_id == "tenant_ar07"
                    && binding.campaign_id == CAMPAIGN
                    && binding.authority_epoch == 1
        ));
    }

    async fn subscribe(
        &mut self,
        request_id: &str,
        subscription: RoomSubscription,
        cursor: u64,
        resume_token: Option<String>,
    ) {
        self.send_client(ClientEnvelope {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.to_owned(),
            message: ClientMessage::Subscribe {
                subscription,
                cursor,
                resume_token,
            },
        })
        .await;
        let subscribed = self.next_server().await;
        assert!(matches!(
            subscribed.message,
            ServerMessage::Subscribed { cursor: subscribed_cursor, .. }
                if subscribed_cursor == cursor
        ));
    }

    async fn pong(&mut self, request_id: &str) {
        self.send_client(ClientEnvelope {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.to_owned(),
            message: ClientMessage::Pong { nonce: 1 },
        })
        .await;
    }

    async fn ack(&mut self, request_id: &str, cursor: u64) -> String {
        self.send_client(ClientEnvelope {
            version: REALTIME_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.to_owned(),
            message: ClientMessage::Ack { cursor },
        })
        .await;
        loop {
            match self.next_server().await.message {
                ServerMessage::Acked {
                    request_id: ack_request,
                    cursor: ack_cursor,
                    resume_token,
                } if ack_request == request_id && ack_cursor == cursor => return resume_token,
                ServerMessage::Heartbeat { .. } => {}
                other => panic!("unexpected server message before ack: {other:?}"),
            }
        }
    }

    async fn collect_checkpoint(&mut self, expected: u64) -> (BTreeSet<String>, String) {
        let mut events = BTreeSet::new();
        loop {
            let envelope = tokio::time::timeout(Duration::from_secs(2), self.next_server())
                .await
                .expect("checkpoint timeout");
            match envelope.message {
                ServerMessage::Event { event, .. } => {
                    events.insert(event.event_type);
                }
                ServerMessage::Checkpoint {
                    cursor,
                    resume_token,
                } if cursor == expected => return (events, resume_token),
                ServerMessage::Checkpoint { cursor, .. } if cursor < expected => {}
                ServerMessage::Heartbeat { .. } => {}
                other => panic!("unexpected server message before checkpoint: {other:?}"),
            }
        }
    }

    async fn send_client(&mut self, envelope: ClientEnvelope) {
        let text = envelope.to_json().expect("serialize client envelope");
        self.send_text(&text).await;
    }

    async fn next_server(&mut self) -> ServerEnvelope {
        loop {
            match self.read_frame().await {
                Frame::Text(text) => {
                    self.seen_text.push_str(&text);
                    self.seen_text.push('\n');
                    return ServerEnvelope::parse_json(&text)
                        .unwrap_or_else(|error| panic!("server envelope {error}: {text}"));
                }
                Frame::Close(code) => panic!("unexpected websocket close {code}"),
                Frame::Other => {}
            }
        }
    }

    fn assert_never_saw(&self, canary: &str) {
        assert!(
            !self.seen_text.contains(canary),
            "unauthorized canary leaked in a server text frame: {canary}"
        );
    }

    async fn next_close(&mut self) -> u16 {
        loop {
            match self.read_frame().await {
                Frame::Close(code) => return code,
                Frame::Text(_) | Frame::Other => {}
            }
        }
    }

    async fn send_text(&mut self, text: &str) {
        let payload = text.as_bytes();
        let mut frame = Vec::with_capacity(payload.len() + 16);
        frame.push(0x81);
        if payload.len() < 126 {
            frame.push(0x80 | u8::try_from(payload.len()).expect("short payload"));
        } else {
            frame.push(0x80 | 126);
            frame.extend_from_slice(
                &u16::try_from(payload.len())
                    .expect("test websocket payload below 64KiB")
                    .to_be_bytes(),
            );
        }
        let mask = [0x11, 0x22, 0x33, 0x44];
        frame.extend_from_slice(&mask);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(index, byte)| byte ^ mask[index % mask.len()]),
        );
        self.stream
            .write_all(&frame)
            .await
            .expect("write websocket frame");
    }

    async fn read_frame(&mut self) -> Frame {
        let first = self.read_exact_buffered(2).await;
        let opcode = first[0] & 0x0f;
        let masked = first[1] & 0x80 != 0;
        assert!(!masked, "server frames must not be masked");
        let mut length = u64::from(first[1] & 0x7f);
        if length == 126 {
            let bytes = self.read_exact_buffered(2).await;
            length = u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        } else if length == 127 {
            let bytes = self.read_exact_buffered(8).await;
            length = u64::from_be_bytes(bytes.try_into().expect("eight length bytes"));
        }
        let payload = self
            .read_exact_buffered(usize::try_from(length).expect("bounded test frame"))
            .await;
        match opcode {
            0x1 => Frame::Text(String::from_utf8(payload).expect("utf8 websocket text")),
            0x8 => {
                let code = payload
                    .get(..2)
                    .map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
                    .unwrap_or(1005);
                Frame::Close(code)
            }
            _ => Frame::Other,
        }
    }

    async fn read_exact_buffered(&mut self, length: usize) -> Vec<u8> {
        while self.buffered.len() < length {
            let mut chunk = vec![0_u8; 4_096];
            let read = self.stream.read(&mut chunk).await.expect("read websocket");
            assert!(read > 0, "websocket closed before frame completed");
            self.buffered.extend_from_slice(&chunk[..read]);
        }
        self.buffered.drain(..length).collect()
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
