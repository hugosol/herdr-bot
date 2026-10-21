//! 测试用的真实 SSE 客户端：自己发请求、自己按 chunked 编码解出 SSE 帧。
//!
//! 与页面同一条路径（HTTP/1.1 + `text/event-stream` + chunked），不经过任何前端代码，也不读任何
//! 进程内状态：用例看到的就是浏览器 `EventSource` 会收到的字节。会话观察与关机状态共用这一条
//! 只读长连接，因此这里只有一套读帧设施，不另建第二套读取约定。

#![allow(dead_code)]

use std::net::SocketAddr;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// 在字节串里找子串。
pub fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// 一帧已解析的 SSE 帧。
#[derive(Debug, Clone)]
pub struct SseFrame {
    /// 帧的原始字节（已被 chunked 解码）。
    pub raw: String,
    /// `event:` 字段；注释帧（keep-alive）没有它。
    pub event: Option<String>,
    /// `data:` 字段（多行以 `\n` 拼接）。
    pub data: Option<String>,
    /// `retry:` 字段（毫秒）。
    pub retry: Option<u64>,
}

impl SseFrame {
    pub fn parse(raw: &[u8]) -> Self {
        let text = String::from_utf8_lossy(raw).into_owned();
        let mut frame = SseFrame {
            raw: text.clone(),
            event: None,
            data: None,
            retry: None,
        };
        let mut data: Vec<&str> = Vec::new();
        for line in text.lines() {
            // 注释帧（keep-alive）以冒号开头，没有字段名。
            if line.starts_with(':') {
                continue;
            }
            let Some((field, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.strip_prefix(' ').unwrap_or(value);
            match field {
                "event" => frame.event = Some(value.to_string()),
                "data" => data.push(value),
                "retry" => frame.retry = value.parse().ok(),
                other => panic!("读路径不得下发该 SSE 字段：{other}（帧：{text:?}）"),
            }
        }
        if !data.is_empty() {
            frame.data = Some(data.join("\n"));
        }
        frame
    }

    /// 帧的 `data` 必须是 JSON（所有命名事件的内容都是 JSON）。
    pub fn json(&self) -> Value {
        let data = self
            .data
            .as_deref()
            .unwrap_or_else(|| panic!("该帧必须带 data：{self:?}"));
        serde_json::from_str(data).unwrap_or_else(|e| panic!("帧 data 必须是 JSON（{e}）：{data}"))
    }

    /// 断言这是一帧指定的命名事件。
    ///
    /// 帧上的未知 SSE 字段（例如游标用的 `id`）在 [`SseFrame::parse`] 里直接让用例失败，
    /// 因此「不引入 `Last-Event-ID`/游标」这条由解析本身覆盖。
    pub fn named(&self, expected: &str) {
        assert_eq!(
            self.event.as_deref(),
            Some(expected),
            "帧的命名事件不符：{self:?}"
        );
    }
}

/// 一条真实的 SSE 长连接。
pub struct SseConnection {
    stream: TcpStream,
    /// 已从 socket 读入、但还没按 chunked 解开的字节。
    raw: Vec<u8>,
    /// 已解开、等待成帧的 SSE 字节。
    decoded: Vec<u8>,
    /// 响应状态码。
    pub status: u16,
    /// 响应头（名字已小写）。
    pub headers: Vec<(String, String)>,
}

impl SseConnection {
    /// 建立一条长连接；`path` 是读取路径，`extra_headers` 用于重连等附加请求头。
    pub async fn open(addr: SocketAddr, path: &str, extra_headers: &[(&str, &str)]) -> Self {
        let mut stream = TcpStream::connect(addr).await.expect("连接测试服务器");
        let mut request = format!(
            "GET {path} HTTP/1.1\r\nHost: {addr}\r\n\
             Accept: text/event-stream\r\nConnection: keep-alive\r\n"
        );
        for (name, value) in extra_headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("发送 SSE 请求");
        stream.flush().await.expect("发送 SSE 请求");

        let mut connection = Self {
            stream,
            raw: Vec::new(),
            decoded: Vec::new(),
            status: 0,
            headers: Vec::new(),
        };
        connection.read_head().await;
        connection
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value.as_str())
    }

    async fn read_head(&mut self) {
        loop {
            if let Some(end) = find(&self.raw, b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&self.raw[..end]).into_owned();
                self.raw.drain(..end + 4);
                let mut lines = head.lines();
                let status_line = lines.next().expect("响应必须包含状态行");
                self.status = status_line
                    .split_whitespace()
                    .nth(1)
                    .expect("状态行必须包含状态码")
                    .parse()
                    .expect("状态码必须是数字");
                self.headers = lines
                    .filter_map(|line| {
                        line.split_once(':').map(|(name, value)| {
                            (name.trim().to_lowercase(), value.trim().to_string())
                        })
                    })
                    .collect();
                return;
            }
            assert!(self.fill().await, "响应头不完整连接就断了");
        }
    }

    /// 读入更多原始字节；返回 false 表示对端已关闭连接。
    async fn fill(&mut self) -> bool {
        let mut chunk = [0u8; 16 * 1024];
        match self.stream.read(&mut chunk).await {
            Ok(0) => false,
            Ok(read) => {
                self.raw.extend_from_slice(&chunk[..read]);
                true
            }
            Err(error) => panic!("读取 SSE 响应失败：{error}"),
        }
    }

    /// 把已读入的字节按 chunked 编码解进 `decoded`；返回是否已读到结束块。
    fn decode_available(&mut self) -> bool {
        loop {
            let Some(line_end) = find(&self.raw, b"\r\n") else {
                return false;
            };
            let field = String::from_utf8_lossy(&self.raw[..line_end]).into_owned();
            let size_field = field.split(';').next().unwrap_or_default().trim();
            let size: usize = usize::from_str_radix(size_field, 16)
                .unwrap_or_else(|_| panic!("chunk 长度必须是十六进制：{field:?}"));
            let body_start = line_end + 2;
            if size == 0 {
                self.raw.clear();
                return true;
            }
            // 块体与它结尾的 CRLF 都到齐了才消费，半个块留在缓冲里等下一段。
            if self.raw.len() < body_start + size + 2 {
                return false;
            }
            self.decoded
                .extend_from_slice(&self.raw[body_start..body_start + size]);
            self.raw.drain(..body_start + size + 2);
        }
    }

    /// 从已解开的字节里取出一帧（帧以空行结束）。
    fn take_frame(&mut self) -> Option<SseFrame> {
        let end = find(&self.decoded, b"\n\n")?;
        let frame: Vec<u8> = self.decoded.drain(..end + 2).collect();
        Some(SseFrame::parse(&frame))
    }

    /// 取下一帧；超时返回 `None`（未成帧的半个帧留在缓冲里，下次继续）。
    ///
    /// 顺序是先解开、再成帧、最后才等更多字节：一轮里刚解出的完整帧不会被后面的等待吞掉。
    /// 连接在预期之外结束（对端不再推、也没推完这一帧）直接让用例失败：只读长连接在用例期间一直开着。
    pub async fn next_frame_within(&mut self, timeout: Duration) -> Option<SseFrame> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let ended = self.decode_available();
            if let Some(frame) = self.take_frame() {
                return Some(frame);
            }
            assert!(!ended, "只读长连接在用例期间不得结束");
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return None;
            }
            if tokio::time::timeout(remaining, self.fill()).await.is_err() {
                return None;
            }
        }
    }

    /// 取下一帧；超时即用例失败（失败信息带上已经拿到的字节，便于定位是「没推」还是「没解开」）。
    pub async fn next_frame(&mut self) -> SseFrame {
        match self.next_frame_within(Duration::from_secs(5)).await {
            Some(frame) => frame,
            None => panic!(
                "等待 SSE 帧超时：已解开 {:?}，未解开 {} 字节",
                String::from_utf8_lossy(&self.decoded),
                self.raw.len()
            ),
        }
    }

    /// 取下一帧**命名事件**（跳过 keep-alive 之类没有事件名的帧）。
    pub async fn next_named_frame(&mut self) -> SseFrame {
        loop {
            let frame = self.next_frame().await;
            if frame.event.is_some() {
                return frame;
            }
        }
    }

    /// 取下一帧指定的命名事件：中间出现的其他命名事件直接让用例失败（帧序列本身就是被验收的事实）。
    pub async fn next_event(&mut self, expected: &str) -> SseFrame {
        let frame = self.next_named_frame().await;
        frame.named(expected);
        frame
    }

    /// 在给定时间内断言没有命名事件到达。
    pub async fn assert_no_named_frame_within(&mut self, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return;
            }
            let Some(frame) = self.next_frame_within(remaining).await else {
                return;
            };
            assert_eq!(
                frame.event, None,
                "本次写入不应产生任何帧，却收到：{frame:?}"
            );
        }
    }
}
