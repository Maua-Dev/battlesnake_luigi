//! Lambda adapter for the same SnakeBollada DecisionState/FutureGraph used
//! by the native server. The Lambda handler and Terraform remain independent.
use crate::models::GameState as RequestGameState;
use crate::runtime::GameRuntime;
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::Instant;

static RUNTIME: OnceLock<GameRuntime> = OnceLock::new();
fn runtime() -> &'static GameRuntime {
    RUNTIME.get_or_init(GameRuntime::new)
}

pub fn info() -> Value {
    json!({
        "apiversion": "1",
        "author": "",
        "color": "#8B0000",
        "head": "tiger-king",
        "tail": "hook",
        "version": "ordering-v3-lambda"
    })
}
pub async fn start(input: &RequestGameState) {
    runtime().start(&crate::GameState::from(input)).await;
}
pub async fn get_move(input: &RequestGameState, started: Instant) -> Value {
    // Stopwatch began BEFORE API payload parsing in the Lambda handler.
    let state = crate::GameState::from(input);
    let mut decision = runtime().decide(&state, started).await;
    // Never return a provable immediate body/wall collision if another
    // deterministic move exists, including after a timeout fallback.
    let simulation = crate::simulation::state::SimulatedGameState::from(&state);
    if let Some(ours) = simulation.snake(&simulation.our_snake_id) {
        let mobility = crate::simulation::mobility::MobilityAnalysis::from_state(&simulation);
        if mobility
            .classify_move(&simulation, ours, decision.direction)
            .is_some()
        {
            let fallback = crate::strategy::choose_move_baseline(&state);
            if mobility
                .classify_move(&simulation, ours, fallback.direction)
                .is_none()
            {
                log::warn!(
                    "lambda_guard game={} turn={} invalid_direction={:?} replacement={:?}",
                    state.game.id,
                    state.turn,
                    decision.direction,
                    fallback.direction
                );
                decision = fallback;
            }
        }
    }
    log::info!(
        "lambda_move game={} turn={} direction={:?} reason={:?} depth={} nodes={} search_us={} request_us={}",
        state.game.id,
        state.turn,
        decision.direction,
        decision.reason,
        decision.search.completed_depth,
        decision.search.nodes,
        decision.search.elapsed_us,
        started.elapsed().as_micros()
    );
    json!({"move": decision.direction.as_str()})
}
pub async fn end(input: &RequestGameState) {
    runtime().end(&crate::GameState::from(input)).await;
}
