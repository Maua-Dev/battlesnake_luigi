//! Internal SnakeBollada simulation types; Lambda input lives in models.rs.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Game {
    pub(crate) id: String,
    pub(crate) ruleset: HashMap<String, Value>,
    pub(crate) timeout: u32,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Board {
    pub(crate) height: u32,
    pub(crate) width: u32,
    pub(crate) food: Vec<Coord>,
    pub(crate) snakes: Vec<Battlesnake>,
    pub(crate) hazards: Vec<Coord>,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct Battlesnake {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) health: i32,
    pub(crate) body: Vec<Coord>,
    pub(crate) head: Coord,
    pub(crate) length: u32,
    #[serde(default)]
    pub(crate) latency: String,
    #[serde(default)]
    pub(crate) shout: Option<String>,
}
#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Coord {
    pub(crate) x: i32,
    pub(crate) y: i32,
}
#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct GameState {
    pub(crate) game: Game,
    pub(crate) turn: i32,
    pub(crate) board: Board,
    pub(crate) you: Battlesnake,
}

impl From<&crate::models::Coord> for Coord {
    fn from(c: &crate::models::Coord) -> Self {
        Self { x: c.x, y: c.y }
    }
}
impl From<&crate::models::Battlesnake> for Battlesnake {
    fn from(s: &crate::models::Battlesnake) -> Self {
        Self {
            id: s.id.clone(),
            name: s.name.clone(),
            health: s.health,
            body: s.body.iter().map(Coord::from).collect(),
            head: Coord::from(&s.head),
            length: s.length.max(0) as u32,
            latency: s.latency.clone().unwrap_or_default(),
            shout: s.shout.clone(),
        }
    }
}
impl From<&crate::models::GameState> for GameState {
    fn from(state: &crate::models::GameState) -> Self {
        let you = Battlesnake::from(&state.you);
        let mut snakes: Vec<Battlesnake> =
            state.board.snakes.iter().map(Battlesnake::from).collect();
        // Tolerate isolated handler fixtures without "you" in board.snakes.
        if !snakes.iter().any(|snake| snake.id == you.id) {
            snakes.push(you.clone());
        }
        Self {
            game: Game {
                id: state.game.id.clone(),
                ruleset: state.game.ruleset.clone(),
                timeout: state.game.timeout,
            },
            turn: state.turn,
            board: Board {
                height: state.board.height.max(0) as u32,
                width: state.board.width.max(0) as u32,
                food: state.board.food.iter().map(Coord::from).collect(),
                snakes,
                hazards: state.board.hazards.iter().map(Coord::from).collect(),
            },
            you,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_uses_our_snake_and_preserves_board() {
        let payload = serde_json::json!({
            "game": {"id": "g", "ruleset": {"name": "standard"}, "timeout": 500},
            "turn": 1,
            "board": {"width": 11, "height": 11, "food": [], "hazards": [], "snakes": []},
            "you": {"id": "ours", "name": "ours", "health": 90,
                    "body": [{"x": 3, "y": 3}], "head": {"x": 3, "y": 3},
                    "length": 1}
        });
        let parsed: crate::models::GameState = serde_json::from_value(payload).unwrap();
        let normalized = GameState::from(&parsed);
        assert_eq!(normalized.board.snakes.len(), 1);
        assert_eq!(normalized.board.snakes[0].id, normalized.you.id);
    }
}
