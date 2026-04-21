//! In-process bge-m3 ONNX embedder for document RAG and conversation vector search.
//!
//! Ported from seCall's OrtEmbedder implementation. Uses ONNX Runtime for local
//! inference with session pooling for concurrent requests.
//!
//! **Role separation**:
//! - rawq daemon = code search (snowflake-arctic-embed-s, 384dim)
//! - This embedder = document RAG + conversation vector (bge-m3, 1024dim)

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde::{Deserialize, Serialize};

use crate::errors::AppError;

// ─── Constants ──────────────────────────────────────────────────────────────

pub const BGE_M3_DIM: usize = 1024;

const MODEL_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx";
const MODEL_DATA_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/onnx/model.onnx_data";
const TOKENIZER_URL: &str = "https://huggingface.co/BAAI/bge-m3/resolve/main/tokenizer.json";

/// Global singleton — initialized once, shared across all threads.
static GLOBAL_EMBEDDER: OnceLock<Result<Arc<BgeM3Embedder>, String>> = OnceLock::new();

// ─── BgeM3Embedder ──────────────────────────────────────────────────────────

/// Local ONNX-based embedder using ort + tokenizers.
/// Session pool enables concurrent inference across CPU cores.
#[allow(dead_code)]
pub struct BgeM3Embedder {
    sessions: Vec<Arc<Mutex<ort::session::Session>>>,
    next_session: AtomicUsize,
    tokenizer: Arc<tokenizers::Tokenizer>,
    dim: usize,
}

#[allow(dead_code)]
impl BgeM3Embedder {
    /// Create with default pool size (2 sessions for desktop app).
    pub fn new(model_dir: &Path) -> Result<Self, AppError> {
        Self::with_pool_size(model_dir, 2)
    }

    pub fn with_pool_size(model_dir: &Path, pool_size: usize) -> Result<Self, AppError> {
        use ort::session::builder::GraphOptimizationLevel;

        let pool_size = pool_size.max(1);

        let tokenizer_path = model_dir.join("tokenizer.json");
        let model_path = model_dir.join("model.onnx");

        if !tokenizer_path.exists() || !model_path.exists() {
            return Err(AppError::Agent(format!(
                "bge-m3 model not found at {}. Run model download first.",
                model_dir.display()
            )));
        }

        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| AppError::Agent(format!("tokenizer load failed: {e}")))?;

        // Build first session and probe dimensions.
        // Thread limits: intra_op=2 (parallelism within a single op), inter_op=1 (sequential ops).
        // Without these, ONNX Runtime claims all available cores per session.
        let first_session = ort::session::Session::builder()
            .map_err(|e| AppError::Agent(format!("ort session builder: {e}")))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| AppError::Agent(format!("ort optimization: {e}")))?
            .with_intra_threads(2)
            .map_err(|e| AppError::Agent(format!("ort intra_threads: {e}")))?
            .with_inter_threads(1)
            .map_err(|e| AppError::Agent(format!("ort inter_threads: {e}")))?
            .commit_from_file(&model_path)
            .map_err(|e| AppError::Agent(format!("ort load model: {e}")))?;

        // Skip probe_dim — bge-m3 is always 1024 dimensions.
        // Probing via inference can fail on some ONNX model configurations.
        let dim = BGE_M3_DIM;

        let mut sessions = Vec::with_capacity(pool_size);
        sessions.push(Arc::new(Mutex::new(first_session)));

        for _ in 1..pool_size {
            let sess = ort::session::Session::builder()
                .map_err(|e| AppError::Agent(format!("ort session builder: {e}")))?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| AppError::Agent(format!("ort optimization: {e}")))?
                .with_intra_threads(2)
                .map_err(|e| AppError::Agent(format!("ort intra_threads: {e}")))?
                .with_inter_threads(1)
                .map_err(|e| AppError::Agent(format!("ort inter_threads: {e}")))?
                .commit_from_file(&model_path)
                .map_err(|e| AppError::Agent(format!("ort load model: {e}")))?;
            sessions.push(Arc::new(Mutex::new(sess)));
        }

        eprintln!("[embedder] bge-m3 ORT pool created: {} sessions, {}dim", pool_size, dim);

        Ok(Self {
            sessions,
            next_session: AtomicUsize::new(0),
            tokenizer: Arc::new(tokenizer),
            dim,
        })
    }

    /// Round-robin session selection.
    fn next_session(&self) -> Arc<Mutex<ort::session::Session>> {
        let idx = self.next_session.fetch_add(1, Ordering::Relaxed) % self.sessions.len();
        Arc::clone(&self.sessions[idx])
    }

    fn probe_dim(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
    ) -> Result<usize, AppError> {
        let embedding = Self::run_inference(session, tokenizer, "test")?;
        Ok(embedding.len())
    }

    /// Embed a single text (blocking). Use `embed_async` for non-blocking.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, AppError> {
        let session = self.next_session();
        let mut session = session
            .lock()
            .map_err(|_| AppError::Agent("ort session lock poisoned".into()))?;
        Self::run_inference(&mut session, &self.tokenizer, text)
    }

    /// Embed a batch of texts (blocking).
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, AppError> {
        let session = self.next_session();
        let mut session = session
            .lock()
            .map_err(|_| AppError::Agent("ort session lock poisoned".into()))?;
        let texts: Vec<String> = texts.iter().map(|t| t.to_string()).collect();
        Self::run_inference_batch(&mut session, &self.tokenizer, &texts)
    }

    /// Async embed (spawns blocking task on tokio threadpool).
    pub async fn embed_async(&self, text: String) -> Result<Vec<f32>, AppError> {
        let session = self.next_session();
        let tokenizer = Arc::clone(&self.tokenizer);
        tokio::task::spawn_blocking(move || {
            let mut session = session
                .lock()
                .map_err(|_| AppError::Agent("ort session lock poisoned".into()))?;
            Self::run_inference(&mut session, &tokenizer, &text)
        })
        .await
        .map_err(|e| AppError::Agent(format!("spawn_blocking join: {e}")))?
    }

    /// Async batch embed.
    pub async fn embed_batch_async(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, AppError> {
        let session = self.next_session();
        let tokenizer = Arc::clone(&self.tokenizer);
        tokio::task::spawn_blocking(move || {
            let mut session = session
                .lock()
                .map_err(|_| AppError::Agent("ort session lock poisoned".into()))?;
            Self::run_inference_batch(&mut session, &tokenizer, &texts)
        })
        .await
        .map_err(|e| AppError::Agent(format!("spawn_blocking join: {e}")))?
    }

    pub fn dimensions(&self) -> usize {
        self.dim
    }

    // ─── Inference ──────────────────────────────────────────────────────

    fn run_inference(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
        text: &str,
    ) -> Result<Vec<f32>, AppError> {
        use ndarray::Array2;
        use ort::value::TensorRef;

        let encoding = tokenizer
            .encode(text, true)
            .map_err(|e| AppError::Agent(format!("tokenize failed: {e}")))?;

        let ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();
        let mask: Vec<i64> = encoding.get_attention_mask().iter().map(|&x| x as i64).collect();
        let seq_len = ids.len();

        let ids_arr = Array2::<i64>::from_shape_vec((1, seq_len), ids)
            .map_err(|e| AppError::Agent(format!("array reshape: {e}")))?;
        let mask_arr = Array2::<i64>::from_shape_vec((1, seq_len), mask)
            .map_err(|e| AppError::Agent(format!("array reshape: {e}")))?;

        let ids_ref = TensorRef::<i64>::from_array_view(ids_arr.view())
            .map_err(|e| AppError::Agent(format!("tensor ids: {e}")))?;
        let mask_ref = TensorRef::<i64>::from_array_view(mask_arr.view())
            .map_err(|e| AppError::Agent(format!("tensor mask: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => ids_ref,
                "attention_mask" => mask_ref,
            ])
            .map_err(|e| AppError::Agent(format!("ort inference: {e}")))?;

        // bge-m3 outputs: "sentence_embedding" (already pooled) or "token_embeddings" (needs pooling)
        // Use sentence_embedding directly — already L2-normalized by the model
        let emb_arr = outputs["sentence_embedding"]
            .try_extract_array::<f32>()
            .map_err(|e| AppError::Agent(format!("extract sentence_embedding: {e}")))?;

        // Shape: [1, dim] — flatten to Vec<f32>
        let embedding: Vec<f32> = emb_arr.iter().copied().collect();

        // L2 normalize (safety — model should already produce unit vectors)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-9 && (norm - 1.0).abs() > 0.01 {
            let mut normalized = embedding;
            for e in normalized.iter_mut() {
                *e /= norm;
            }
            return Ok(normalized);
        }

        Ok(embedding)
    }

    fn run_inference_batch(
        session: &mut ort::session::Session,
        tokenizer: &tokenizers::Tokenizer,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, AppError> {
        use ndarray::Array2;
        use ort::value::TensorRef;

        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let encodings = tokenizer
            .encode_batch(texts.iter().map(|t| t.as_str()).collect::<Vec<_>>(), true)
            .map_err(|e| AppError::Agent(format!("batch tokenize: {e}")))?;

        let batch_size = texts.len();
        let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

        if max_len == 0 {
            return Ok(vec![Vec::new(); batch_size]);
        }

        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));

        for (i, enc) in encodings.iter().enumerate() {
            for (j, (&id, &m)) in enc.get_ids().iter().zip(enc.get_attention_mask().iter()).enumerate() {
                input_ids[[i, j]] = id as i64;
                attention_mask[[i, j]] = m as i64;
            }
        }

        let ids_ref = TensorRef::<i64>::from_array_view(input_ids.view())
            .map_err(|e| AppError::Agent(format!("tensor ids: {e}")))?;
        let mask_ref = TensorRef::<i64>::from_array_view(attention_mask.view())
            .map_err(|e| AppError::Agent(format!("tensor mask: {e}")))?;

        let outputs = session
            .run(ort::inputs![
                "input_ids" => ids_ref,
                "attention_mask" => mask_ref,
            ])
            .map_err(|e| AppError::Agent(format!("ort batch inference: {e}")))?;

        // bge-m3: "sentence_embedding" shape [batch_size, dim]
        let emb_arr = outputs["sentence_embedding"]
            .try_extract_array::<f32>()
            .map_err(|e| AppError::Agent(format!("extract sentence_embedding: {e}")))?;
        let dim = emb_arr.shape()[1];

        let mut results = Vec::with_capacity(batch_size);
        for i in 0..batch_size {
            let mut embedding = Vec::with_capacity(dim);
            for d in 0..dim {
                embedding.push(emb_arr[[i, d]]);
            }
            // L2 normalize (safety)
            let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-9 && (norm - 1.0).abs() > 0.01 {
                for e in embedding.iter_mut() {
                    *e /= norm;
                }
            }
            results.push(embedding);
        }

        Ok(results)
    }
}

// ─── Model Manager ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub model: String,
    pub downloaded_at: String,
    pub sha256_model: String,
    pub sha256_tokenizer: String,
    pub source_revision: String,
}

/// Manages bge-m3 model download and caching.
/// Shares cache with seCall at `~/.cache/tunaflow/models/bge-m3-onnx/`.
pub struct ModelManager {
    model_dir: PathBuf,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            model_dir: default_model_path(),
        }
    }

    pub fn model_dir(&self) -> &Path {
        &self.model_dir
    }

    pub fn is_downloaded(&self) -> bool {
        self.model_dir.join("model.onnx").exists()
            && self.model_dir.join("model.onnx_data").exists()
            && self.model_dir.join("tokenizer.json").exists()
    }

    /// Download model files if not present. Returns model directory path.
    pub async fn ensure_downloaded(&self) -> Result<PathBuf, AppError> {
        if self.is_downloaded() {
            return Ok(self.model_dir.clone());
        }

        // Also check seCall's cache (avoid duplicate download)
        let secall_path = secall_model_path();
        if secall_path.join("model.onnx").exists()
            && secall_path.join("model.onnx_data").exists()
            && secall_path.join("tokenizer.json").exists()
        {
            eprintln!("[embedder] found bge-m3 in seCall cache, symlinking...");
            std::fs::create_dir_all(&self.model_dir)
                .map_err(|e| AppError::Agent(format!("mkdir: {e}")))?;

            let files = ["model.onnx", "model.onnx_data", "tokenizer.json"];
            for fname in &files {
                let src = secall_path.join(fname);
                let dst = self.model_dir.join(fname);
                #[cfg(unix)]
                std::os::unix::fs::symlink(&src, &dst)
                    .map_err(|e| AppError::Agent(format!("symlink {fname}: {e}")))?;
                #[cfg(not(unix))]
                std::fs::copy(&src, &dst)
                    .map_err(|e| AppError::Agent(format!("copy {fname}: {e}")))?;
            }
            return Ok(self.model_dir.clone());
        }

        self.download().await?;
        Ok(self.model_dir.clone())
    }

    async fn download(&self) -> Result<(), AppError> {
        std::fs::create_dir_all(&self.model_dir)
            .map_err(|e| AppError::Agent(format!("mkdir: {e}")))?;

        eprintln!("[embedder] downloading bge-m3 model (~1.1GB)...");

        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(30))
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .map_err(|e| AppError::Agent(format!("http client: {e}")))?;

        let model_sha = self.download_file(&client, MODEL_URL, "model.onnx").await?;
        // bge-m3 uses external data: model.onnx (metadata ~725KB) + model.onnx_data (weights ~1.1GB)
        let _data_sha = self.download_file(&client, MODEL_DATA_URL, "model.onnx_data").await?;
        let tokenizer_sha = self.download_file(&client, TOKENIZER_URL, "tokenizer.json").await?;

        let version = VersionInfo {
            model: "BAAI/bge-m3".to_string(),
            downloaded_at: chrono::Utc::now().to_rfc3339(),
            sha256_model: model_sha,
            sha256_tokenizer: tokenizer_sha,
            source_revision: "main".to_string(),
        };
        let version_json = serde_json::to_string_pretty(&version)
            .map_err(|e| AppError::Agent(format!("json serialize: {e}")))?;
        std::fs::write(self.model_dir.join("version.json"), version_json)
            .map_err(|e| AppError::Agent(format!("write version: {e}")))?;

        eprintln!("[embedder] bge-m3 download complete");
        Ok(())
    }

    async fn download_file(
        &self,
        client: &reqwest::Client,
        url: &str,
        final_name: &str,
    ) -> Result<String, AppError> {
        use futures_util::StreamExt;
        use sha2::{Digest, Sha256};
        use std::io::Write;

        let tmp_path = self.model_dir.join(format!("{final_name}.tmp"));
        let final_path = self.model_dir.join(final_name);

        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Agent(format!("download request: {e}")))?;

        if !resp.status().is_success() {
            return Err(AppError::Agent(format!(
                "download failed ({}): {}",
                resp.status(),
                url
            )));
        }

        let total = resp.content_length();
        let mut stream = resp.bytes_stream();
        let mut file = std::fs::File::create(&tmp_path)
            .map_err(|e| AppError::Agent(format!("create temp: {e}")))?;
        let mut hasher = Sha256::new();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::Agent(format!("stream: {e}")))?;
            hasher.update(&chunk);
            file.write_all(&chunk)
                .map_err(|e| AppError::Agent(format!("write: {e}")))?;
            downloaded += chunk.len() as u64;

            if let Some(total) = total {
                let pct = downloaded * 100 / total;
                if pct % 10 == 0 {
                    eprintln!(
                        "[embedder] downloading {final_name}... {pct}% ({}/{})",
                        format_bytes(downloaded),
                        format_bytes(total)
                    );
                }
            }
        }

        drop(file);
        std::fs::rename(&tmp_path, &final_path)
            .map_err(|e| AppError::Agent(format!("rename: {e}")))?;

        Ok(format!("{:x}", hasher.finalize()))
    }
}

// ─── Global Singleton ───────────────────────────────────────────────────────

/// Initialize the global bge-m3 embedder. Call once at app startup (sync version).
/// If model files exist AND ONNX Runtime is available, initializes immediately.
/// Uses catch_unwind to prevent app crash if ORT dylib is missing.
pub fn init_global_embedder() -> Result<(), AppError> {
    let mgr = ModelManager::new();

    // Check tunaflow cache first, then seCall cache
    let model_dir = if mgr.is_downloaded() {
        Some(mgr.model_dir().to_path_buf())
    } else {
        let sp = secall_model_path();
        if sp.join("model.onnx").exists() && sp.join("model.onnx_data").exists() && sp.join("tokenizer.json").exists() {
            Some(sp)
        } else {
            None
        }
    };

    if let Some(dir) = model_dir {
        // Catch panic from ORT dylib loading (load-dynamic feature may panic if libonnxruntime not found)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            BgeM3Embedder::new(&dir)
        }));

        match result {
            Ok(Ok(embedder)) => {
                eprintln!("[embedder] bge-m3 global embedder initialized");
                GLOBAL_EMBEDDER.get_or_init(|| Ok(Arc::new(embedder)));
            }
            Ok(Err(e)) => {
                eprintln!("[embedder] bge-m3 init failed: {e}");
                GLOBAL_EMBEDDER.get_or_init(|| Err(e.to_string()));
            }
            Err(_) => {
                eprintln!("[embedder] bge-m3 init panicked — ONNX Runtime library not found. Install via: brew install onnxruntime");
                eprintln!("[embedder] falling back to rawq for embeddings");
                GLOBAL_EMBEDDER.get_or_init(|| Err("ORT dylib not found".to_string()));
            }
        }
    } else {
        eprintln!("[embedder] bge-m3 model not found — download will start in background...");
    }
    Ok(())
}

/// Async initialization: downloads model if needed, then initializes embedder.
/// Call from a tokio task at startup.
pub async fn init_global_embedder_async() -> Result<(), AppError> {
    // Already initialized successfully?
    if get_embedder().is_some() {
        return Ok(());
    }

    let mgr = ModelManager::new();
    let model_dir = mgr.ensure_downloaded().await?;

    // Initialize in blocking context with panic protection
    let result = tokio::task::spawn_blocking(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            BgeM3Embedder::new(&model_dir)
        }))
    })
    .await
    .map_err(|e| AppError::Agent(format!("spawn_blocking: {e}")))?;

    match result {
        Ok(Ok(embedder)) => {
            eprintln!("[embedder] bge-m3 global embedder initialized (after download)");
            GLOBAL_EMBEDDER.get_or_init(|| Ok(Arc::new(embedder)));
        }
        Ok(Err(e)) => {
            eprintln!("[embedder] bge-m3 init failed after download: {e}");
            GLOBAL_EMBEDDER.get_or_init(|| Err(e.to_string()));
        }
        Err(panic_info) => {
            let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "unknown panic".to_string()
            };
            eprintln!("[embedder] bge-m3 init panicked: {msg}");
            GLOBAL_EMBEDDER.get_or_init(|| Err(format!("ORT panic: {msg}")));
        }
    }
    Ok(())
}

/// Get the global bge-m3 embedder (None if not initialized or model missing).
pub fn get_embedder() -> Option<Arc<BgeM3Embedder>> {
    GLOBAL_EMBEDDER
        .get()
        .and_then(|r| r.as_ref().ok())
        .cloned()
}

/// Embed text using the global bge-m3 embedder.
/// Falls back to rawq if bge-m3 is unavailable.
pub fn embed_text(text: &str, _is_query: bool) -> Result<Vec<f32>, AppError> {
    if text.trim().is_empty() {
        return Err(AppError::Agent("empty text".into()));
    }

    // Truncate to ~512 tokens worth (~2000 chars for mixed CJK/English)
    let truncated = if text.len() > 2000 {
        let end = text
            .char_indices()
            .take_while(|&(i, _)| i <= 2000)
            .last()
            .map_or(0, |(i, _)| i);
        &text[..end]
    } else {
        text
    };

    if let Some(embedder) = get_embedder() {
        return embedder.embed(truncated);
    }

    // Fallback: rawq (384dim — only works if vec_chunks is still 384dim)
    crate::agents::rawq::embed_text(truncated, _is_query).map_err(|e| AppError::Agent(e.to_string()))
}

/// Check if bge-m3 is available (model downloaded + embedder initialized).
pub fn is_available() -> bool {
    get_embedder().is_some()
}

/// Get embedding dimension for current embedder.
#[allow(dead_code)]
pub fn current_dim() -> usize {
    get_embedder()
        .map(|e| e.dimensions())
        .unwrap_or(crate::agents::rawq::EMBED_DIM)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

pub fn default_model_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache")
        .join("tunaflow")
        .join("models")
        .join("bge-m3-onnx")
}

fn secall_model_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".cache")
        .join("secall")
        .join("models")
        .join("bge-m3-onnx")
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1}GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else if bytes >= 1024 * 1024 {
        format!("{:.0}MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.0}KB", bytes as f64 / 1024.0)
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model_path() {
        let path = default_model_path();
        assert!(path.to_str().unwrap().contains("bge-m3-onnx"));
    }

    #[test]
    fn test_secall_model_path() {
        let path = secall_model_path();
        assert!(path.to_str().unwrap().contains("secall"));
    }

    #[test]
    fn test_model_manager_not_downloaded() {
        let mgr = ModelManager {
            model_dir: PathBuf::from("/nonexistent/path"),
        };
        assert!(!mgr.is_downloaded());
    }

    #[test]
    fn test_version_info_serde() {
        let v = VersionInfo {
            model: "BAAI/bge-m3".to_string(),
            downloaded_at: "2026-04-11T12:00:00Z".to_string(),
            sha256_model: "abc123".to_string(),
            sha256_tokenizer: "def456".to_string(),
            source_revision: "main".to_string(),
        };
        let json = serde_json::to_string(&v).unwrap();
        let v2: VersionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(v.model, v2.model);
        assert_eq!(v.sha256_model, v2.sha256_model);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(1024), "1KB");
        assert_eq!(format_bytes(1024 * 1024), "1MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 + 100_000_000), "1.1GB");
    }

    #[test]
    fn test_embed_text_empty() {
        let result = embed_text("", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_current_dim_fallback() {
        // Without global embedder, should fallback to rawq dim
        assert!(current_dim() > 0);
    }

    #[test]
    fn test_embedder_real() {
        // Runtime-guarded: model files are ~2GB and only cached on dev machines.
        // CI skips silently; local dev validates full inference pipeline.
        let model_dir = default_model_path();
        if !model_dir.join("model.onnx").exists() {
            eprintln!("skipping: model not downloaded");
            return;
        }
        let embedder = BgeM3Embedder::new(&model_dir).expect("BgeM3Embedder::new");
        assert_eq!(embedder.dimensions(), 1024);

        let embedding = embedder.embed("hello world").expect("embed");
        assert_eq!(embedding.len(), 1024);

        // Check L2 normalized
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.01,
            "L2 norm should be ~1.0, got {norm}"
        );
    }

    #[test]
    fn test_batch_embed_real() {
        // Runtime-guarded: see test_embedder_real above.
        let model_dir = default_model_path();
        if !model_dir.join("model.onnx").exists() {
            eprintln!("skipping: model not downloaded");
            return;
        }
        let embedder = BgeM3Embedder::new(&model_dir).expect("BgeM3Embedder::new");
        let results = embedder
            .embed_batch(&["hello world", "안녕하세요", "code review"])
            .expect("embed_batch");
        assert_eq!(results.len(), 3);
        for emb in &results {
            assert_eq!(emb.len(), 1024);
        }
    }
}
