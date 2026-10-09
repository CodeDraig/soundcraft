//! TCP control connections, timeouts and bounded reconnect retry.

use super::Backend;
use serde_json::Value;
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// A TCP client for a running app's control channel.
pub struct Remote {
    addr: String,
    conn: Option<(TcpStream, BufReader<TcpStream>)>,
    next_id: u64,
}

impl Remote {
    /// `target` is a port (`7979`) or `host:port`.
    pub fn new(target: &str) -> Self {
        let addr = if target.contains(':') { target.to_string() } else { format!("127.0.0.1:{target}") };
        Remote { addr, conn: None, next_id: 1 }
    }

    fn connect(&mut self) -> Result<(), String> {
        if self.conn.is_some() {
            return Ok(());
        }
        let sock = self.addr.parse().map_err(|e| format!("{}: {e}", self.addr))?;
        let s =
            TcpStream::connect_timeout(&sock, Duration::from_millis(800)).map_err(|e| format!("cannot reach SoundCraft at {}: {e}", self.addr))?;
        s.set_read_timeout(Some(Duration::from_secs(120))).map_err(|e| e.to_string())?;
        let r = s.try_clone().map_err(|e| e.to_string())?;
        self.conn = Some((s, BufReader::new(r)));
        Ok(())
    }

    fn roundtrip(&mut self, method: &str, params: &Value) -> Result<Value, String> {
        self.connect()?;
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        let (w, r) = self.conn.as_mut().ok_or("not connected")?;
        writeln!(w, "{line}").map_err(|e| e.to_string())?;
        let mut buf = String::new();
        r.read_line(&mut buf).map_err(|e| e.to_string())?;
        if buf.is_empty() {
            return Err("connection closed".into());
        }
        let v: Value = serde_json::from_str(&buf).map_err(|e| e.to_string())?;
        if v.get("ok").and_then(Value::as_bool).unwrap_or(false) {
            Ok(v.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(v.get("error").and_then(Value::as_str).unwrap_or("error").to_string())
        }
    }
}

impl Backend for Remote {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        match self.roundtrip(method, &params) {
            Ok(v) => Ok(v),
            Err(e) if e.contains("closed") || e.contains("Broken pipe") || e.contains("reset") => {
                // Retry once on a fresh connection.
                self.conn = None;
                self.roundtrip(method, &params)
            }
            Err(e) => Err(e),
        }
    }
    fn has_ui(&self) -> bool {
        true
    }
    fn describe(&self) -> String {
        format!("SoundCraft app at {}", self.addr)
    }
}
