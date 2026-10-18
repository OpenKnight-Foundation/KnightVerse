use super::models::{Chess960Library, Chess960Position, LibraryMetadata};
use chrono::Utc;
use std::collections::HashMap;

pub struct Chess960Generator;

impl Chess960Generator {
    pub fn generate_all_positions() -> Chess960Library {
        let mut positions = HashMap::new();
        let mut position_id = 1u16;

        // Generate all valid Chess960 positions using systematic approach
        for arrangement in Self::generate_valid_arrangements() {
            let position = Self::create_position(position_id, arrangement);
            positions.insert(position_id, position);
            position_id += 1;
        }

        Chess960Library {
            positions,
            total_positions: 960,
            metadata: LibraryMetadata {
                version: "1.0.0".to_string(),
                generated_at: Utc::now().to_rfc3339(),
                description: "Complete Chess960 starting positions library".to_string(),
            },
        }
    }

    fn generate_valid_arrangements() -> Vec<[usize; 8]> {
        let mut valid_arrangements = Vec::new();

        // Systematic generation ensuring Chess960 rules
        for king in 1..=6 {
            // King must be between rooks (positions 1-6)
            for queen in 0..=7 {
                if queen == king {
                    continue;
                }

                for rook1 in 0..king {
                    if rook1 == queen {
                        continue;
                    }

                    for rook2 in (king + 1)..=7 {
                        if rook2 == queen {
                            continue;
                        }

                        for knight1 in 0..=7 {
                            if [king, queen, rook1, rook2].contains(&knight1) {
                                continue;
                            }

                            for knight2 in (knight1 + 1)..=7 {
                                if [king, queen, rook1, rook2, knight1].contains(&knight2) {
                                    continue;
                                }

                                // Find remaining positions for bishops
                                let pieces = [king, queen, rook1, rook2, knight1, knight2];
                                let occupied: std::collections::HashSet<_> =
                                    pieces.iter().copied().collect();

                                let free: Vec<usize> =
                                    (0..=7).filter(|pos| !occupied.contains(pos)).collect();

                                if free.len() == 2 {
                                    let bishop1 = free[0];
                                    let bishop2 = free[1];

                                    // Verify bishops are on opposite colors
                                    if (bishop1 + bishop2) % 2 == 1 {
                                        valid_arrangements.push([
                                            king, queen, rook1, rook2, bishop1, bishop2, knight1,
                                            knight2,
                                        ]);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        valid_arrangements
    }

    fn create_position(id: u16, arrangement: [usize; 8]) -> Chess960Position {
        let [king, queen, rook1, rook2, bishop1, bishop2, knight1, knight2] = arrangement;

        let mut back_rank = ['?'; 8];
        back_rank[king] = 'K';
        back_rank[queen] = 'Q';
        back_rank[rook1] = 'R';
        back_rank[rook2] = 'R';
        back_rank[bishop1] = 'B';
        back_rank[bishop2] = 'B';
        back_rank[knight1] = 'N';
        back_rank[knight2] = 'N';

        let back_rank_str: String = back_rank.iter().collect();
        let fen = format!(
            "{}/pppppppp/8/8/8/8/PPPPPPPP/{} w KQkq - 0 1",
            back_rank_str.to_lowercase(),
            back_rank_str
        );

        Chess960Position {
            position_number: id,
            fen,
            back_rank: back_rank_str,
            white_king_pos: king,
            white_rook_positions: [rook1.min(rook2), rook1.max(rook2)],
            white_bishop_positions: [bishop1.min(bishop2), bishop1.max(bishop2)],
            white_knight_positions: [knight1.min(knight2), knight1.max(knight2)],
            white_queen_pos: queen,
        }
    }

    pub fn verify_position(position: &Chess960Position) -> bool {
        let king_pos = position.white_king_pos;
        let [rook1, rook2] = position.white_rook_positions;
        let [bishop1, bishop2] = position.white_bishop_positions;

        // Verify king between rooks
        if !(rook1 < king_pos && king_pos < rook2) {
            return false;
        }

        // Verify bishops on opposite colors
        if (bishop1 + bishop2) % 2 == 0 {
            return false;
        }

        // Verify all positions are unique
        let positions = [
            king_pos,
            position.white_queen_pos,
            rook1,
            rook2,
            bishop1,
            bishop2,
            position.white_knight_positions[0],
            position.white_knight_positions[1],
        ];
        let unique_positions: std::collections::HashSet<_> = positions.iter().copied().collect();
        unique_positions.len() == 8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_960_positions() {
        let library = Chess960Generator::generate_all_positions();
        assert_eq!(library.positions.len(), 960);
        assert_eq!(library.total_positions, 960);
    }

    #[test]
    fn test_all_positions_valid() {
        let library = Chess960Generator::generate_all_positions();

        for position in library.positions.values() {
            assert!(Chess960Generator::verify_position(position));
        }
    }

    #[test]
    fn test_fen_format() {
        let library = Chess960Generator::generate_all_positions();

        for position in library.positions.values().take(10) {
            let fen_parts: Vec<&str> = position.fen.split(' ').collect();
            assert_eq!(fen_parts.len(), 6);
            assert_eq!(fen_parts[1], "w"); // White to move
            assert_eq!(fen_parts[2], "KQkq"); // Castling rights
            assert_eq!(fen_parts[3], "-"); // No en passant
            assert_eq!(fen_parts[4], "0"); // Halfmove clock
            assert_eq!(fen_parts[5], "1"); // Fullmove number
        }
    }
}
