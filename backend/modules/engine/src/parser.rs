use crate::EngineResult;

pub fn parse_uci_line(line: &str) -> Option<UciMessage> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "id" => {
            if parts.len() >= 3 {
                match parts[1] {
                    "name" => Some(UciMessage::IdName(parts[2..].join(" "))),
                    "author" => Some(UciMessage::IdAuthor(parts[2..].join(" "))),
                    _ => None,
                }
            } else {
                None
            }
        }
        "uciok" => Some(UciMessage::UciOk),
        "readyok" => Some(UciMessage::ReadyOk),
        "bestmove" => {
            if parts.len() >= 2 {
                let best_move = parts[1].to_string();
                let ponder = if parts.len() >= 4 && parts[2] == "ponder" {
                    Some(parts[3].to_string())
                } else {
                    None
                };
                Some(UciMessage::BestMove { best_move, ponder })
            } else {
                None
            }
        }
        "info" => {
            let mut depth = None;
            let mut score_cp = None;
            let mut score_mate = None;
            let mut pv = Vec::new();

            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "depth" => {
                        if i + 1 < parts.len() {
                            depth = parts[i + 1].parse::<u8>().ok();
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    "score" => {
                        if i + 2 < parts.len() {
                            match parts[i + 1] {
                                "cp" => {
                                    score_cp = parts[i + 2].parse::<i32>().ok();
                                    i += 3;
                                }
                                "mate" => {
                                    score_mate = parts[i + 2].parse::<i32>().ok();
                                    i += 3;
                                }
                                _ => {
                                    i += 1;
                                }
                            }
                        } else {
                            i += 1;
                        }
                    }
                    "pv" => {
                        i += 1;
                        while i < parts.len() {
                            pv.push(parts[i].to_string());
                            i += 1;
                        }
                    }
                    _ => {
                        i += 1;
                    }
                }
            }
            Some(UciMessage::Info {
                depth,
                score_cp,
                score_mate,
                pv,
            })
        }
        _ => Some(UciMessage::Unknown(line.to_string())),
    }
}

#[derive(Debug, Clone)]
pub enum UciMessage {
    IdName(String),
    IdAuthor(String),
    UciOk,
    ReadyOk,
    BestMove {
        best_move: String,
        ponder: Option<String>,
    },
    Info {
        depth: Option<u8>,
        score_cp: Option<i32>,
        score_mate: Option<i32>,
        pv: Vec<String>,
    },
    Unknown(String),
}

impl From<UciMessage> for Option<EngineResult> {
    fn from(msg: UciMessage) -> Self {
        match msg {
            UciMessage::BestMove { best_move, .. } => Some(EngineResult {
                best_move,
                evaluation: None,
                depth: None,
                principal_variation: Vec::new(),
            }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_uciok() {
        let msg = parse_uci_line("uciok").unwrap();
        assert!(matches!(msg, UciMessage::UciOk));
    }

    #[test]
    fn test_parse_readyok() {
        let msg = parse_uci_line("readyok").unwrap();
        assert!(matches!(msg, UciMessage::ReadyOk));
    }

    #[test]
    fn test_parse_bestmove() {
        let msg = parse_uci_line("bestmove e2e4 ponder e7e5").unwrap();
        if let UciMessage::BestMove { best_move, ponder } = msg {
            assert_eq!(best_move, "e2e4");
            assert_eq!(ponder, Some("e7e5".to_string()));
        } else {
            panic!("Expected BestMove");
        }
    }

    #[test]
    fn test_parse_info() {
        let msg = parse_uci_line("info depth 12 score cp 35 pv e2e4 e7e5 Ng1f3").unwrap();
        if let UciMessage::Info {
            depth,
            score_cp,
            score_mate,
            pv,
        } = msg
        {
            assert_eq!(depth, Some(12));
            assert_eq!(score_cp, Some(35));
            assert_eq!(score_mate, None);
            assert_eq!(pv, vec!["e2e4", "e7e5", "Ng1f3"]);
        } else {
            panic!("Expected Info");
        }
    }

    #[test]
    fn test_parse_info_mate() {
        let msg = parse_uci_line("info depth 12 score mate 3 pv e2e4 e7e5 Ng1f3").unwrap();
        if let UciMessage::Info {
            depth,
            score_cp,
            score_mate,
            pv,
        } = msg
        {
            assert_eq!(depth, Some(12));
            assert_eq!(score_cp, None);
            assert_eq!(score_mate, Some(3));
            assert_eq!(pv, vec!["e2e4", "e7e5", "Ng1f3"]);
        } else {
            panic!("Expected Info");
        }
    }

    #[test]
    fn test_parse_id() {
        let msg = parse_uci_line("id name Stockfish 16").unwrap();
        if let UciMessage::IdName(name) = msg {
            assert_eq!(name, "Stockfish 16");
        } else {
            panic!("Expected IdName");
        }
    }
}
