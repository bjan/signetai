//! Predictor integration: scoring and training via the predictor library.
//!
//! Instead of spawning a sidecar process, the predictor runs as a library
//! with a dedicated training thread (non-tokio) and a bounded scoring channel.

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::warn;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Predictor status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictorStatus {
    pub trained: bool,
    pub training_pairs: usize,
    pub model_version: u64,
    pub last_trained: Option<String>,
    pub native_dimensions: usize,
    pub feature_dimensions: usize,
}

/// Score request for a batch of candidates.
#[derive(Debug, Clone)]
pub struct ScoreRequest {
    pub context_embedding: Vec<f64>,
    pub candidate_ids: Vec<String>,
    pub candidate_embeddings: Vec<Vec<f64>>,
    pub candidate_features: Vec<Vec<f64>>,
}

/// Individual candidate score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScore {
    pub id: String,
    pub score: f64,
}

/// Score response.
#[derive(Debug, Clone)]
pub struct ScoreResponse {
    pub scores: Vec<CandidateScore>,
}

/// Training result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrainResult {
    pub loss: f64,
    pub step: u64,
    pub samples_used: usize,
    pub samples_skipped: usize,
    pub duration_ms: u64,
    pub checkpoint_saved: bool,
}

// ---------------------------------------------------------------------------
// Predictor handle
// ---------------------------------------------------------------------------

/// Handle to the predictor (fail-open design — returns None when unavailable).
pub struct PredictorHandle {
    status: Mutex<PredictorStatus>,
}

impl PredictorHandle {
    /// Create a new predictor handle.
    pub fn new(native_dims: usize) -> Self {
        Self {
            status: Mutex::new(PredictorStatus {
                trained: false,
                training_pairs: 0,
                model_version: 0,
                last_trained: None,
                native_dimensions: native_dims,
                feature_dimensions: 17, // FEATURE_DIM from predictor protocol
            }),
        }
    }

    /// Get current status.
    pub async fn status(&self) -> PredictorStatus {
        self.status.lock().await.clone()
    }

    /// Score candidates against context. Returns None if predictor unavailable.
    pub async fn score(&self, _req: ScoreRequest) -> Option<ScoreResponse> {
        let status = self.status.lock().await;
        if !status.trained {
            return None;
        }

        // TODO: Integrate predictor::model::CrossAttentionScorer directly
        // For now, return None (fail-open, baseline scoring continues)
        None
    }

    /// Train from database. Returns None on failure.
    pub async fn train_from_db(
        &self,
        _db_path: &str,
        _checkpoint_path: Option<&str>,
    ) -> Option<TrainResult> {
        // TODO: Spawn training on dedicated OS thread, call predictor::training
        warn!("predictor training not yet integrated");
        None
    }

    /// Save checkpoint.
    pub async fn save_checkpoint(&self, _path: &str) -> bool {
        // TODO: Save model checkpoint via predictor::checkpoint
        false
    }
}

impl Default for PredictorHandle {
    fn default() -> Self {
        Self::new(768)
    }
}

/// Feature dimension count (must match predictor::protocol::FEATURE_DIM).
pub const FEATURE_DIM: usize = 17;

/// Build a feature vector for a memory candidate.
#[allow(clippy::too_many_arguments)]
pub fn build_features(
    age_days: f64,
    importance: f64,
    access_count: u64,
    tod_hour: f64,
    dow: f64,
    moy: f64,
    session_gap_days: f64,
    is_embedded: bool,
    is_superseded: bool,
    entity_slot: f64,
    aspect_slot: f64,
    is_constraint: bool,
    structural_density: f64,
    is_ka_traversal: bool,
) -> Vec<f64> {
    use std::f64::consts::PI;

    vec![
        (age_days.max(0.0) + 1.0).ln(),          // [0] log(age_days)
        importance,                              // [1] importance
        (access_count as f64 + 1.0).ln(),        // [2] log(access_count + 1)
        (2.0 * PI * tod_hour / 24.0).sin(),      // [3] tod_sin
        (2.0 * PI * tod_hour / 24.0).cos(),      // [4] tod_cos
        (2.0 * PI * dow / 7.0).sin(),            // [5] dow_sin
        (2.0 * PI * dow / 7.0).cos(),            // [6] dow_cos
        (2.0 * PI * moy / 12.0).sin(),           // [7] moy_sin
        (2.0 * PI * moy / 12.0).cos(),           // [8] moy_cos
        (session_gap_days.max(0.0) + 1.0).ln(),  // [9] log(session_gap_days)
        if is_embedded { 1.0 } else { 0.0 },     // [10] is_embedded
        if is_superseded { 1.0 } else { 0.0 },   // [11] is_superseded
        entity_slot.clamp(0.0, 1.0),             // [12] entity_slot
        aspect_slot.clamp(0.0, 1.0),             // [13] aspect_slot
        if is_constraint { 1.0 } else { 0.0 },   // [14] is_constraint
        (structural_density + 1.0).ln(),         // [15] log(density + 1)
        if is_ka_traversal { 1.0 } else { 0.0 }, // [16] is_ka_traversal
    ]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feature_vector_length() {
        let features = build_features(
            1.0, 0.5, 3, 14.0, 2.0, 6.0, 0.5, true, false, 0.3, 0.7, false, 2.0, true,
        );
        assert_eq!(features.len(), FEATURE_DIM);
    }

    #[test]
    fn feature_vector_values() {
        let features = build_features(
            0.0, 1.0, 0, 0.0, 0.0, 0.0, 0.0, false, false, 0.0, 0.0, false, 0.0, false,
        );
        // [0] log(0+1) = 0
        assert!((features[0] - 0.0).abs() < f64::EPSILON);
        // [1] importance = 1.0
        assert!((features[1] - 1.0).abs() < f64::EPSILON);
        // [10] is_embedded = 0
        assert!((features[10] - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn predictor_score_untrained() {
        let handle = PredictorHandle::new(768);
        let req = ScoreRequest {
            context_embedding: vec![0.0; 768],
            candidate_ids: vec!["m1".into()],
            candidate_embeddings: vec![vec![0.0; 768]],
            candidate_features: vec![vec![0.0; FEATURE_DIM]],
        };
        // Untrained predictor returns None (fail-open)
        assert!(handle.score(req).await.is_none());
    }
}
