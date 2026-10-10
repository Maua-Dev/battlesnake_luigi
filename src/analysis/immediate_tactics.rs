//! Verified one-ply outcomes using exactly the joint-action generator and turn
//! resolver used by FutureGraph. No Hobbs heuristic can turn partial
//! adversarial coverage into a proof of survival.
use std::time::Instant;

use crate::decision::joint_actions::JointActionGenerator;
use crate::direction::{Direction, MoveMask};
use crate::simulation::mobility::MobilityAnalysis;
use crate::simulation::resolver::resolve_turn;
use crate::simulation::state::SimulatedGameState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TacticalStatus {
    /// All legal joint replies were simulated; all leave our snake alive.
    VerifiedSafe,
    /// A concrete adversarial reply kills our snake. Other replies may survive.
    Vulnerable,
    /// All legal joint replies were simulated; every reply kills us.
    ForcedLoss,
    /// The reply generator was not exhausted within the tactical budget.
    Incomplete,
    /// Deterministic fatal next move, independent of enemy response.
    Blocked,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TacticalDirection {
    pub(crate) status: TacticalStatus,
    pub(crate) checked_replies: u16,
    pub(crate) fatal_replies: u16,
}
impl Default for TacticalDirection {
    fn default() -> Self {
        Self {
            status: TacticalStatus::Incomplete,
            checked_replies: 0,
            fatal_replies: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RootTactics {
    directions: [TacticalDirection; 4],
}
impl RootTactics {
    pub(crate) fn entry(&self, direction: Direction) -> TacticalDirection {
        self.directions[usize::from(direction.rank())]
    }
    pub(crate) fn verified_safe(&self, direction: Direction) -> bool {
        self.entry(direction).status == TacticalStatus::VerifiedSafe
    }
    pub(crate) fn any_verified_safe(&self) -> bool {
        Direction::ALL
            .into_iter()
            .any(|direction| self.verified_safe(direction))
    }
    pub(crate) fn demonstrated_loss(&self, direction: Direction) -> bool {
        matches!(
            self.entry(direction).status,
            TacticalStatus::Vulnerable | TacticalStatus::ForcedLoss | TacticalStatus::Blocked
        )
    }
    /// Only simulate root replies when a simultaneous head-to-head is geometrically
    /// possible next turn. Else return None to keep the normal V3 budget intact.
    pub(crate) fn when_head_contact_possible(
        state: &SimulatedGameState,
        deadline: Instant,
        max_replies_per_direction: usize,
    ) -> Option<Self> {
        let head = state.snake(&state.our_snake_id)?.head()?;
        let contact_possible = state.snakes.iter().any(|snake| {
            snake.alive
                && snake.id != state.our_snake_id
                && snake
                    .head()
                    .is_some_and(|enemy| head.x.abs_diff(enemy.x) + head.y.abs_diff(enemy.y) <= 2)
        });
        contact_possible.then(|| Self::analyze(state, deadline, max_replies_per_direction))
    }
    /// A complete direction checks every action generated for all living
    /// opponents. If work is interrupted, no VerifiedSafe proof is emitted.
    fn analyze(
        state: &SimulatedGameState,
        deadline: Instant,
        max_replies_per_direction: usize,
    ) -> Self {
        let mut result = Self {
            directions: [TacticalDirection::default(); 4],
        };
        let mobility = MobilityAnalysis::from_state(state);
        let Some(ours) = state.snake(&state.our_snake_id).filter(|snake| snake.alive) else {
            return result;
        };
        for direction in Direction::ALL {
            let slot = &mut result.directions[usize::from(direction.rank())];
            if mobility.classify_move(state, ours, direction).is_some() {
                slot.status = TacticalStatus::Blocked;
                continue;
            }
            if Instant::now() >= deadline {
                break;
            }
            let mut exhausted = true;
            for joint in JointActionGenerator::adversarial_root(
                state,
                MoveMask::single(direction),
                &mobility,
            ) {
                if Instant::now() >= deadline
                    || usize::from(slot.checked_replies) >= max_replies_per_direction
                {
                    exhausted = false;
                    break;
                }
                let Ok(next) = resolve_turn(state, &joint) else {
                    exhausted = false;
                    break;
                };
                slot.checked_replies += 1;
                if !next
                    .state
                    .snake(&state.our_snake_id)
                    .is_some_and(|snake| snake.alive)
                {
                    slot.fatal_replies += 1;
                }
            }
            slot.status = if exhausted && slot.checked_replies > 0 {
                if slot.fatal_replies == 0 {
                    TacticalStatus::VerifiedSafe
                } else if slot.fatal_replies == slot.checked_replies {
                    TacticalStatus::ForcedLoss
                } else {
                    TacticalStatus::Vulnerable
                }
            } else if slot.fatal_replies > 0 {
                // A single fully simulated losing counterexample is enough
                // to disprove a claim that this direction is guaranteed safe.
                TacticalStatus::Vulnerable
            } else {
                TacticalStatus::Incomplete
            };
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::state::{RulesContext, SimulatedSnake};
    use crate::Coord;
    use std::time::Duration;

    fn snake(id: &str, body: &[(i32, i32)]) -> SimulatedSnake {
        SimulatedSnake {
            id: id.into(),
            health: 100,
            body: body.iter().map(|&(x, y)| Coord { x, y }).collect(),
            alive: true,
        }
    }
    fn state(
        our_body: &[(i32, i32)],
        enemy_body: &[(i32, i32)],
        food: &[(i32, i32)],
    ) -> SimulatedGameState {
        SimulatedGameState {
            turn: 33,
            width: 11,
            height: 11,
            food: food.iter().map(|&(x, y)| Coord { x, y }).collect(),
            hazards: vec![],
            snakes: vec![snake("ours", our_body), snake("enemy", enemy_body)],
            our_snake_id: "ours".into(),
            rules: RulesContext {
                name: "standard".into(),
                max_health: 100,
                hazard_damage_per_turn: 0,
            },
        }
    }
    fn report(s: &SimulatedGameState, cap: usize) -> RootTactics {
        RootTactics::analyze(s, Instant::now() + Duration::from_secs(1), cap)
    }
    #[test]
    fn smaller_snake_cannot_claim_contested_food_against_longer_enemy() {
        let s = state(
            &[(2, 2), (2, 1)],
            &[(4, 2), (4, 1), (5, 1), (5, 2)],
            &[(3, 2)],
        );
        let r = report(&s, 16);
        assert!(r.demonstrated_loss(Direction::Right), "{r:?}");
        assert!(r.verified_safe(Direction::Up), "{r:?}");
        assert!(r.entry(Direction::Right).fatal_replies >= 1);
    }
    #[test]
    fn equal_length_head_to_head_is_not_safe_even_with_food() {
        let s = state(
            &[(2, 2), (2, 1), (1, 1)],
            &[(4, 2), (4, 1), (5, 1)],
            &[(3, 2)],
        );
        let r = report(&s, 16);
        assert!(r.demonstrated_loss(Direction::Right), "{r:?}");
        assert!(r.verified_safe(Direction::Up), "{r:?}");
    }
    #[test]
    fn reply_budget_exhaustion_cannot_prove_survival() {
        let s = state(
            &[(2, 2), (2, 1)],
            &[(4, 2), (4, 1), (5, 1), (5, 2)],
            &[(3, 2)],
        );
        let r = report(&s, 0);
        assert_eq!(r.entry(Direction::Up).status, TacticalStatus::Incomplete);
        assert!(!r.any_verified_safe());
    }
    #[test]
    fn distant_heads_skip_expensive_simulation() {
        let s = state(&[(1, 1), (1, 0)], &[(9, 9), (9, 8), (8, 8)], &[]);
        assert!(RootTactics::when_head_contact_possible(
            &s,
            Instant::now() + Duration::from_millis(15),
            16
        )
        .is_none());
    }
}
