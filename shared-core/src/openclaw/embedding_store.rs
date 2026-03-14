use anyhow::{Result, bail};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EmbeddingType {
    Character,
    Style,
    Scene,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub id: String,
    pub name: String,
    pub embedding_type: EmbeddingType,
    pub vector: Vec<f64>,
    pub metadata: HashMap<String, String>,
}

pub struct EmbeddingStore {
    embeddings: HashMap<String, Embedding>,
}

impl EmbeddingStore {
    pub fn new() -> Self {
        Self { embeddings: HashMap::new() }
    }

    pub fn create_embedding(&mut self, name: &str, embedding_type: EmbeddingType, vector: Vec<f64>) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let embedding = Embedding {
            id: id.clone(),
            name: name.to_string(),
            embedding_type,
            vector,
            metadata: HashMap::new(),
        };
        self.embeddings.insert(id.clone(), embedding);
        Ok(id)
    }

    pub fn get_embedding(&self, id: &str) -> Option<&Embedding> {
        self.embeddings.get(id)
    }

    pub fn get_embedding_by_name(&self, name: &str) -> Option<&Embedding> {
        self.embeddings.values().find(|e| e.name == name)
    }

    pub fn list_by_type(&self, embedding_type: EmbeddingType) -> Vec<&Embedding> {
        self.embeddings.values().filter(|e| e.embedding_type == embedding_type).collect()
    }

    pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
        if a.len() != b.len() || a.is_empty() { return 0.0; }
        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 { return 0.0; }
        dot / (norm_a * norm_b)
    }
}

impl Default for EmbeddingStore {
    fn default() -> Self { Self::new() }
}
