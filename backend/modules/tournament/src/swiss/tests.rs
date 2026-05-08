#[cfg(test)]
mod tests {
    use super::super::*;
    use uuid::Uuid;

    fn create_test_players() -> Vec<Player> {
        vec![
            Player::new(Uuid::new_v4(), "Alice".to_string(), 2000),
            Player::new(Uuid::new_v4(), "Bob".to_string(), 1900),
            Player::new(Uuid::new_v4(), "Charlie".to_string(), 1800),
            Player::new(Uuid::new_v4(), "Diana".to_string(), 1700),
            Player::new(Uuid::new_v4(), "Eve".to_string(), 1600),
        ]
    }

    #[test]
    fn test_tournament_state_creation() {
        let players = create_test_players();
        let tournament = TournamentState::new(players.clone(), 5);
        
        assert_eq!(tournament.players.len(), 5);
        assert_eq!(tournament.current_round, 1);
        assert_eq!(tournament.completed_rounds, 0);
        assert_eq!(tournament.total_rounds, 5);
        
        // Check all players start with 0 score
        for player in tournament.players.values() {
            assert_eq!(player.score, 0.0);
            assert!(player.opponents.is_empty());
            assert!(player.color_history.is_empty());
        }
    }

    #[test]
    fn test_player_sorting_by_score_then_rating() {
        let mut tournament = TournamentState::new(create_test_players(), 5);

        // Modify scores by player name to avoid HashMap iteration-order nondeterminism.
        let alice_id = tournament
            .players
            .values()
            .find(|p| p.name == "Alice")
            .map(|p| p.id)
            .unwrap();
        let bob_id = tournament
            .players
            .values()
            .find(|p| p.name == "Bob")
            .map(|p| p.id)
            .unwrap();
        let charlie_id = tournament
            .players
            .values()
            .find(|p| p.name == "Charlie")
            .map(|p| p.id)
            .unwrap();

        if let Some(player) = tournament.players.get_mut(&alice_id) {
            player.score = 2.0;
        }
        if let Some(player) = tournament.players.get_mut(&bob_id) {
            player.score = 2.0;
            player.rating = 2100; // Higher rating than Alice
        }
        if let Some(player) = tournament.players.get_mut(&charlie_id) {
            player.score = 1.5;
        }
        
        let sorted_players = tournament.get_players_sorted_by_score_then_rating();
        
        // Should be: player 1 (2.0, 2100), player 0 (2.0, 2000), player 2 (1.5, 1800)
        assert_eq!(sorted_players[0].rating, 2100);
        assert_eq!(sorted_players[1].rating, 2000);
        assert_eq!(sorted_players[2].score, 1.5);
    }

    #[test]
    fn test_color_balance_tracking() {
        let mut player = Player::new(Uuid::new_v4(), "Test".to_string(), 1500);
        
        assert_eq!(player.get_color_balance(), 0);
        assert!(!player.should_prefer_white());
        
        // Add white game
        player.color_history.push(Color::White);
        assert_eq!(player.get_color_balance(), 1);
        assert!(!player.should_prefer_white()); // Prefers black now
        
        // Add black game
        player.color_history.push(Color::Black);
        assert_eq!(player.get_color_balance(), 0);
        assert!(!player.should_prefer_white()); // Balanced
        
        // Add another black game
        player.color_history.push(Color::Black);
        assert_eq!(player.get_color_balance(), -1);
        assert!(player.should_prefer_white()); // Prefers white now
    }

    #[test]
    fn test_game_result_application() {
        let mut tournament = TournamentState::new(create_test_players(), 5);
        let player_ids: Vec<Uuid> = tournament.players.keys().cloned().collect();
        
        // Create a pairing for round 1
        let pairing = Pairing {
            white_player: player_ids[0],
            black_player: player_ids[1],
            round: 1,
        };
        tournament.pairings.push(pairing);
        
        // Apply results
        let results = vec![
            (player_ids[0], GameResult::Win),  // White wins
            (player_ids[1], GameResult::Loss), // Black loses
        ];
        
        tournament.apply_round_results(results);
        
        // Check scores
        assert_eq!(tournament.players[&player_ids[0]].score, 1.0);
        assert_eq!(tournament.players[&player_ids[1]].score, 0.0);
        assert_eq!(tournament.players[&player_ids[0]].opponents.len(), 1);
        assert_eq!(tournament.players[&player_ids[1]].opponents.len(), 1);
        assert_eq!(tournament.completed_rounds, 1);
        assert_eq!(tournament.current_round, 2);
    }

    #[test]
    fn test_swiss_pairing_even_players() {
        let players = create_test_players();
        let mut tournament = TournamentState::new(players, 5);
        let pairer = SwissPairer::new(SwissConfig::default());
        
        let pairings = pairer.pair_round(&mut tournament).unwrap();
        
        // Should have 2 pairings (4 players) and 1 bye (5th player)
        assert_eq!(pairings.len(), 3);
        
        let pairing_count = pairings.iter().filter(|p| matches!(p, PairingResult::Paired(_))).count();
        let bye_count = pairings.iter().filter(|p| matches!(p, PairingResult::Bye(_))).count();
        
        assert_eq!(pairing_count, 2);
        assert_eq!(bye_count, 1);
    }

    #[test]
    fn test_swiss_pairing_odd_players() {
        let mut players = create_test_players();
        players.pop(); // Remove one player to make it even (4 players)
        
        let mut tournament = TournamentState::new(players, 5);
        let pairer = SwissPairer::new(SwissConfig::default());
        
        let pairings = pairer.pair_round(&mut tournament).unwrap();
        
        // Should have exactly 2 pairings, no byes
        assert_eq!(pairings.len(), 2);
        
        for pairing in &pairings {
            match pairing {
                PairingResult::Paired(_) => {}, // Expected
                PairingResult::Bye(_) => panic!("Unexpected bye with even number of players"),
            }
        }
    }

    #[test]
    fn test_bye_assignment() {
        let players = create_test_players();
        let mut tournament = TournamentState::new(players, 5);
        let pairer = SwissPairer::new(SwissConfig::default());
        
        // Find who gets the bye (should be lowest rated)
        let initial_players = tournament.get_players_sorted_by_score_then_rating();
        let expected_bye_candidate = initial_players.last().unwrap(); // Lowest rated
        let expected_id = expected_bye_candidate.id;
        
        let pairings = pairer.pair_round(&mut tournament).unwrap();
        
        // Find the bye
        let bye_player_id = pairings.iter()
            .find_map(|p| {
                if let PairingResult::Bye(id) = p {
                    Some(id)
                } else {
                    None
                }
            })
            .unwrap();
        
        assert_eq!(*bye_player_id, expected_id);
        
        // Check that bye player received 1 point
        assert_eq!(tournament.players[bye_player_id].score, 1.0);
    }

    #[test]
    fn test_avoid_repeat_pairings() {
        let players = create_test_players();
        let mut tournament = TournamentState::new(players, 5);
        let pairer = SwissPairer::new(SwissConfig::default());
        
        // First round
        let first_round_pairings = pairer.pair_round(&mut tournament).unwrap();
        tournament.pairings.clear();
        
        // Convert pairing results to actual pairings
        for pairing_result in &first_round_pairings {
            if let PairingResult::Paired(pairing) = pairing_result {
                tournament.pairings.push(pairing.clone());
            }
        }
        
        // Apply dummy results to advance
        let player_ids: Vec<Uuid> = tournament.players.keys().cloned().collect();
        let results: Vec<(Uuid, GameResult)> = player_ids.iter()
            .map(|&id| {
                if tournament.players[&id].score > 0.5 {
                    (id, GameResult::Win)
                } else {
                    (id, GameResult::Loss)
                }
            })
            .collect();
        
        tournament.apply_round_results(results);
        
        // Second round
        let second_round_pairings = pairer.pair_round(&mut tournament).unwrap();
        
        // Verify no repeat pairings
        for pairing_result in &second_round_pairings {
            if let PairingResult::Paired(pairing) = pairing_result {
                let has_played_before = tournament.players[&pairing.white_player]
                    .has_played_against(&pairing.black_player);
                assert!(!has_played_before, "Players should not be paired against each other again");
            }
        }
    }

    #[test]
    fn test_tournament_completion() {
        let players = create_test_players();
        let mut tournament = TournamentState::new(players, 3); // 3 rounds
        
        assert!(!tournament.is_complete());
        
        tournament.completed_rounds = 3;
        assert!(tournament.is_complete());
    }

    #[test]
    fn test_example_tournament_scenario() {
        // Create a realistic tournament scenario
        let players = vec![
            Player::new(Uuid::new_v4(), "GM Magnus".to_string(), 2847),
            Player::new(Uuid::new_v4(), "GM Hikaru".to_string(), 2786),
            Player::new(Uuid::new_v4(), "GM Fabiano".to_string(), 2760),
            Player::new(Uuid::new_v4(), "GM Wesley".to_string(), 2720),
            Player::new(Uuid::new_v4(), "GM Ding".to_string(), 2691),
            Player::new(Uuid::new_v4(), "GM Ian".to_string(), 2686),
            Player::new(Uuid::new_v4(), "GM Leinier".to_string(), 2676),
            Player::new(Uuid::new_v4(), "GM Anish".to_string(), 2673),
        ];
        
        let mut tournament = TournamentState::new(players, 5);
        let pairer = SwissPairer::new(SwissConfig::default());
        
        // Simulate first round
        let round1_pairings = pairer.pair_round(&mut tournament).unwrap();
        assert_eq!(round1_pairings.len(), 4); // 4 pairings, no byes (8 players)
        
        // Apply realistic first round results (higher rated players tend to win)
        let mut results = Vec::new();
        for pairing_result in &round1_pairings {
            if let PairingResult::Paired(pairing) = pairing_result {
                let white_rating = tournament.players[&pairing.white_player].rating;
                let black_rating = tournament.players[&pairing.black_player].rating;
                
                // Higher rated player wins (simplified)
                if white_rating > black_rating {
                    results.push((pairing.white_player, GameResult::Win));
                    results.push((pairing.black_player, GameResult::Loss));
                } else {
                    results.push((pairing.white_player, GameResult::Loss));
                    results.push((pairing.black_player, GameResult::Win));
                }
            }
        }
        
        tournament.apply_round_results(results);
        
        // Verify tournament state after round 1
        assert_eq!(tournament.completed_rounds, 1);
        assert_eq!(tournament.current_round, 2);
        
        // Check that players have different scores
        let scores: Vec<f32> = tournament.players.values().map(|p| p.score).collect();
        let mut unique_scores = std::collections::HashSet::new();
        for score in scores {
            let score_bits = score.to_bits();
            unique_scores.insert(score_bits);
        }
        assert!(unique_scores.len() > 1, "Players should have different scores after round 1");
        
        // Second round should pair players with same scores when possible
        let round2_pairings = pairer.pair_round(&mut tournament).unwrap();
        assert_eq!(round2_pairings.len(), 4);
    }
}
