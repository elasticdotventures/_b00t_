use snafu::prelude::*;

// 🤓 Modular error types following snafu patterns for structured error management
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum GrokError {
    #[snafu(display("Qdrant connection failed: {source}"))]
    QdrantConnection {
        source: qdrant_client::QdrantError,
    },

    #[snafu(display("Qdrant operation failed: {source}"))]
    QdrantOperation {
        source: qdrant_client::QdrantError,
    },

    #[snafu(display("Embedding generation failed: {source}"))]
    EmbeddingGeneration {
        source: async_openai::error::OpenAIError,
    },

    #[snafu(display("Ollama API configuration invalid: {message}"))]
    OllamaConfig { 
        message: String 
    },

    #[snafu(display("Environment variable '{variable}' not set or invalid"))]
    EnvironmentVariable { 
        variable: String 
    },

    #[snafu(display("Python semantic chunking failed: {message}"))]
    SemanticChunking { 
        message: String 
    },

    #[snafu(display("Content chunking failed: {message}"))]
    ContentChunking { 
        message: String 
    },

    #[snafu(display("Serialization failed: {source}"))]
    Serialization {
        source: serde_json::Error,
    },

    #[snafu(display("UUID parsing failed: {source}"))]
    UuidParsing {
        source: uuid::Error,
    },

    #[snafu(display("Date/time parsing failed: {source}"))]
    DateTimeParsing {
        source: chrono::format::ParseError,
    },

    #[snafu(display("Client not initialized - call initialize() first"))]
    ClientNotInitialized,

    #[snafu(display("Invalid vector dimensions: expected {expected}, got {actual}"))]
    VectorDimensions {
        expected: usize,
        actual: usize,
    },

    #[snafu(display("Content too large: {size} bytes exceeds limit of {limit} bytes"))]
    ContentTooLarge {
        size: usize,
        limit: usize,
    },

    #[snafu(display("Collection '{collection}' operation failed: {message}"))]
    CollectionOperation {
        collection: String,
        message: String,
    },

    #[snafu(display("Invalid query parameters: {message}"))]
    InvalidQuery {
        message: String,
    },

    #[snafu(display("IO operation failed: {source}"))]
    Io {
        source: std::io::Error,
    },

    #[snafu(display("HTTP request failed: {source}"))]
    Http {
        source: reqwest::Error,
    },

    #[snafu(display("Timeout occurred after {seconds} seconds during {operation}"))]
    Timeout {
        seconds: u64,
        operation: String,
    },
}

// Convenience type alias
pub type Result<T, E = GrokError> = std::result::Result<T, E>;

impl GrokError {
    /// Check if error is recoverable and worth retrying
    pub fn is_recoverable(&self) -> bool {
        match self {
            GrokError::QdrantConnection { .. } => true,
            GrokError::EmbeddingGeneration { .. } => true,
            GrokError::Http { .. } => true,
            GrokError::Timeout { .. } => true,
            _ => false,
        }
    }

    /// Get error category for logging/metrics
    pub fn category(&self) -> &'static str {
        match self {
            GrokError::QdrantConnection { .. } | GrokError::QdrantOperation { .. } => "database",
            GrokError::EmbeddingGeneration { .. } | GrokError::OllamaConfig { .. } => "embedding",
            GrokError::SemanticChunking { .. } | GrokError::ContentChunking { .. } => "chunking",
            GrokError::EnvironmentVariable { .. } => "configuration",
            GrokError::Serialization { .. } => "serialization",
            GrokError::ClientNotInitialized => "initialization",
            GrokError::VectorDimensions { .. } | GrokError::InvalidQuery { .. } => "validation",
            GrokError::ContentTooLarge { .. } => "limits",
            GrokError::CollectionOperation { .. } => "database",
            GrokError::Io { .. } | GrokError::Http { .. } => "network",
            GrokError::Timeout { .. } => "timeout",
            GrokError::UuidParsing { .. } | GrokError::DateTimeParsing { .. } => "parsing",
        }
    }

    /// Get user-friendly error message
    pub fn user_message(&self) -> String {
        match self {
            GrokError::QdrantConnection { .. } => 
                "Unable to connect to vector database. Check server status and network connectivity.".to_string(),
            GrokError::QdrantOperation { .. } => 
                "Database operation failed. Please try again.".to_string(),
            GrokError::EmbeddingGeneration { .. } => 
                "Failed to generate text embeddings. Check Ollama server status.".to_string(),
            GrokError::OllamaConfig { .. } => 
                "Ollama configuration is invalid. Check OLLAMA_API_URL environment variable.".to_string(),
            GrokError::EnvironmentVariable { variable } => 
                format!("Missing required configuration: {}. Please check your .env file.", variable),
            GrokError::SemanticChunking { .. } | GrokError::ContentChunking { .. } => 
                "Failed to process content into chunks. Content may be malformed.".to_string(),
            GrokError::ClientNotInitialized => 
                "Service not ready. Please wait for initialization to complete.".to_string(),
            GrokError::VectorDimensions { .. } => 
                "Vector dimension mismatch. This may indicate a model configuration issue.".to_string(),
            GrokError::ContentTooLarge { limit, .. } => 
                format!("Content is too large. Maximum size is {} bytes.", limit),
            GrokError::InvalidQuery { .. } => 
                "Query parameters are invalid. Please check your request.".to_string(),
            GrokError::Timeout { seconds, operation } => 
                format!("Operation '{}' timed out after {} seconds. Please try again.", operation, seconds),
            _ => "An unexpected error occurred. Please contact support.".to_string(),
        }
    }
}

// 🤓 Implementing From traits for seamless error propagation with ?
impl From<qdrant_client::QdrantError> for GrokError {
    fn from(err: qdrant_client::QdrantError) -> Self {
        // Differentiate between connection and operation errors
        let error_str = err.to_string().to_lowercase();
        if error_str.contains("connection") || error_str.contains("connect") || error_str.contains("network") {
            GrokError::QdrantConnection { source: err }
        } else {
            GrokError::QdrantOperation { source: err }
        }
    }
}

impl From<async_openai::error::OpenAIError> for GrokError {
    fn from(err: async_openai::error::OpenAIError) -> Self {
        GrokError::EmbeddingGeneration { source: err }
    }
}

impl From<serde_json::Error> for GrokError {
    fn from(err: serde_json::Error) -> Self {
        GrokError::Serialization { source: err }
    }
}

impl From<uuid::Error> for GrokError {
    fn from(err: uuid::Error) -> Self {
        GrokError::UuidParsing { source: err }
    }
}

impl From<chrono::format::ParseError> for GrokError {
    fn from(err: chrono::format::ParseError) -> Self {
        GrokError::DateTimeParsing { source: err }
    }
}

impl From<std::io::Error> for GrokError {
    fn from(err: std::io::Error) -> Self {
        GrokError::Io { source: err }
    }
}

impl From<reqwest::Error> for GrokError {
    fn from(err: reqwest::Error) -> Self {
        GrokError::Http { source: err }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categories() {
        let env_error = GrokError::EnvironmentVariable { variable: "TEST".to_string() };
        assert_eq!(env_error.category(), "configuration");
        assert!(!env_error.is_recoverable());
        
        let timeout_error = GrokError::Timeout { seconds: 30, operation: "test".to_string() };
        assert_eq!(timeout_error.category(), "timeout");
        assert!(timeout_error.is_recoverable());
    }

    #[test]
    fn test_user_messages() {
        let client_error = GrokError::ClientNotInitialized;
        let user_msg = client_error.user_message();
        assert!(user_msg.contains("Service not ready"));
        
        let env_error = GrokError::EnvironmentVariable { variable: "QDRANT_URL".to_string() };
        let env_msg = env_error.user_message();
        assert!(env_msg.contains("QDRANT_URL"));
    }

    #[test]
    fn test_error_conversion() {
        let json_error = serde_json::from_str::<serde_json::Value>("invalid json");
        assert!(json_error.is_err());
        
        let grok_error: GrokError = json_error.unwrap_err().into();
        assert!(matches!(grok_error, GrokError::Serialization { .. }));
        assert_eq!(grok_error.category(), "serialization");
    }
}