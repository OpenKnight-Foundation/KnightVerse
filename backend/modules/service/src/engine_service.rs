use engine::{process::ProcessEngine, Engine, EngineError, EngineResult, GoParams};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

pub struct EngineService {
    #[allow(dead_code)]
    engines: Arc<Mutex<HashMap<Uuid, Box<dyn Engine>>>>,
    engine_path: String,
}

impl EngineService {
    pub fn new(engine_path: String) -> Self {
        Self {
            engines: Arc::new(Mutex::new(HashMap::new())),
            engine_path,
        }
    }

    pub async fn get_suggestion(
        &self,
        fen: &str,
        depth: Option<u8>,
        time_limit_ms: Option<u32>,
    ) -> Result<EngineResult, EngineError> {
        // For now, we'll create a new engine instance for each request
        // In a real scenario, we might want to pool them
        let mut engine: ProcessEngine = ProcessEngine::new(&self.engine_path).await?;
        engine.is_ready().await?;
        engine.set_position(fen).await?;

        let params = GoParams {
            depth,
            time_limit_ms,
            search_moves: None,
        };

        let result = engine.go(params).await?;
        engine.quit().await?;

        Ok(result)
    }

    pub async fn analyze_position(
        &self,
        fen: &str,
        depth: u8,
    ) -> Result<EngineResult, EngineError> {
        self.get_suggestion(fen, Some(depth), None).await
    }
}
