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
    // Cheap adversarial precheck is only needed when heads could meet next
    // turn. It uses the exact FutureGraph action generator and resolver.
    let simulated = crate::simulation::state::SimulatedGameState::from(&state);
    let tactics = crate::analysis::RootTactics::when_head_contact_possible(
        &simulated,
        Instant::now() + std::time::Duration::from_millis(15),
        16,
    );
    let mut decision = runtime().decide(&state, started).await;
    if let Some(report) = tactics.as_ref() {
        if report.any_verified_safe() && report.demonstrated_loss(decision.direction) {
            let previous = decision.direction;
            let replacement =
                crate::strategy::choose_move_baseline_with_tactics(&state, Some(report));
            if report.verified_safe(replacement.direction) {
                log::warn!(
                    "lambda_tactical_guard game={} turn={} rejected={:?} reply={:?} selected={:?} selected_reply={:?}",
                    state.game.id,
                    state.turn,
                    previous,
                    report.entry(previous),
                    replacement.direction,
                    report.entry(replacement.direction)
                );
                decision = replacement;
            }
        }
    }
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
