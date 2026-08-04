use crate::parser::{UciMessage, parse_uci_line};
use crate::{Engine, EngineError, EngineResult, GoParams};
use async_trait::async_trait;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub struct ProcessEngine {
    child: Child,
    stdin: tokio::process::ChildStdin,
    stdout_reader: Arc<Mutex<BufReader<tokio::process::ChildStdout>>>,
}

impl ProcessEngine {
    pub async fn new(path: &str) -> Result<Self, EngineError> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child.stdin.take().ok_or(EngineError::NotRunning)?;
        let stdout = child.stdout.take().ok_or(EngineError::NotRunning)?;
        let stdout_reader = Arc::new(Mutex::new(BufReader::new(stdout)));

        let mut engine = Self {
            child,
            stdin,
            stdout_reader,
        };

        // Initialize UCI
        engine.send_command("uci").await?;

        // Wait for uciok with 5-second timeout
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let line = engine.read_line().await?;
                if let Some(UciMessage::UciOk) = parse_uci_line(&line) {
                    break;
                }
            }
            Ok::<(), EngineError>(())
        })
        .await
        .map_err(|_| EngineError::Timeout)??;

        Ok(engine)
    }

    async fn send_command(&mut self, cmd: &str) -> Result<(), EngineError> {
        self.stdin
            .write_all(format!("{}\n", cmd).as_bytes())
            .await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn read_line(&self) -> Result<String, EngineError> {
        let mut reader = self.stdout_reader.lock().await;
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            return Err(EngineError::NotRunning);
        }
        Ok(line.trim().to_string())
    }
}

#[async_trait]
impl Engine for ProcessEngine {
    async fn go(&mut self, params: GoParams) -> Result<EngineResult, EngineError> {
        let mut cmd = "go".to_string();
        if let Some(depth) = params.depth {
            cmd.push_str(&format!(" depth {}", depth));
        }
        if let Some(time) = params.time_limit_ms {
            cmd.push_str(&format!(" movetime {}", time));
        }

        self.send_command(&cmd).await?;

        let mut last_info = None;
        let timeout_duration = params
            .time_limit_ms
            .map(|t| std::time::Duration::from_millis(t as u64 + 1000))
            .unwrap_or(std::time::Duration::from_secs(30));

        let result = tokio::time::timeout(timeout_duration, async {
            loop {
                let line = self.read_line().await?;
                match parse_uci_line(&line) {
                    Some(UciMessage::BestMove { best_move, .. }) => {
                        let mut result = EngineResult {
                            best_move,
                            evaluation: None,
                            depth: None,
                            principal_variation: Vec::new(),
                        };
                        if let Some(UciMessage::Info {
                            depth,
                            score_cp,
                            score_mate: _,
                            pv,
                        }) = last_info.clone()
                        {
                            result.depth = depth;
                            result.evaluation = score_cp.map(|cp| cp as f32 / 100.0);
                            result.principal_variation = pv;
                        }
                        return Ok(result);
                    }
                    Some(UciMessage::Info {
                        depth,
                        score_cp,
                        score_mate,
                        pv,
                    }) => {
                        last_info = Some(UciMessage::Info {
                            depth,
                            score_cp,
                            score_mate,
                            pv,
                        });
                    }
                    _ => {}
                }
            }
        })
        .await;

        match result {
            Ok(res) => res,
            Err(_) => {
                let _ = self.send_command("stop").await;
                // Drain lines until BestMove
                loop {
                    let line = self.read_line().await?;
                    match parse_uci_line(&line) {
                        Some(UciMessage::BestMove { best_move, .. }) => {
                            let mut result = EngineResult {
                                best_move,
                                evaluation: None,
                                depth: None,
                                principal_variation: Vec::new(),
                            };
                            if let Some(UciMessage::Info {
                                depth,
                                score_cp,
                                score_mate: _,
                                pv,
                            }) = last_info
                            {
                                result.depth = depth;
                                result.evaluation = score_cp.map(|cp| cp as f32 / 100.0);
                                result.principal_variation = pv;
                            }
                            return Err(EngineError::Timeout);
                        }
                        Some(UciMessage::Info {
                            depth,
                            score_cp,
                            score_mate,
                            pv,
                        }) => {
                            last_info = Some(UciMessage::Info {
                                depth,
                                score_cp,
                                score_mate,
                                pv,
                            });
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    async fn stop(&mut self) -> Result<(), EngineError> {
        self.send_command("stop").await
    }

    async fn set_position(&mut self, fen: &str) -> Result<(), EngineError> {
        self.send_command(&format!("position fen {}", fen)).await
    }

    async fn is_ready(&mut self) -> Result<bool, EngineError> {
        self.send_command("isready").await?;
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let line = self.read_line().await?;
                if let Some(UciMessage::ReadyOk) = parse_uci_line(&line) {
                    return Ok(true);
                }
            }
        })
        .await;

        match result {
            Ok(res) => res,
            Err(_) => {
                let _ = self.send_command("stop").await;
                // Drain lines until ReadyOk
                loop {
                    let line = self.read_line().await?;
                    if let Some(UciMessage::ReadyOk) = parse_uci_line(&line) {
                        break;
                    }
                }
                Err(EngineError::Timeout)
            }
        }
    }

    async fn quit(&mut self) -> Result<(), EngineError> {
        self.send_command("quit").await?;
        let _ = self.child.wait().await;
        Ok(())
    }
}

impl Drop for ProcessEngine {
    fn drop(&mut self) {
        // Best effort to kill the child process
        let _ = self.child.start_kill();
    }
}
