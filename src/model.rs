use std::sync::{Arc, Mutex};

use rust_bert::{
    pipelines::{
        conversation::{ConversationConfig, ConversationModel},
        sentiment::{SentimentConfig, SentimentModel},
        summarization::{SummarizationConfig, SummarizationModel},
    },
    RustBertError,
};
use serenity::prelude::TypeMapKey;

pub struct Model {
    pub summarization: Mutex<SummarizationModel>,
    pub sentiment: Mutex<SentimentModel>,
    pub conversation: Mutex<ConversationModel>,
}

impl Model {
    pub async fn new() -> Result<Model, RustBertError> {
        let summarization_config = {
            let mut config = SummarizationConfig::default();
            config.length_penalty = 0.5;
            config
        };

        tokio::task::spawn_blocking(move || {
            Ok(Model {
                summarization: Mutex::new(SummarizationModel::new(summarization_config)?),
                sentiment: Mutex::new(SentimentModel::new(SentimentConfig::default())?),
                conversation: Mutex::new(ConversationModel::new(ConversationConfig {
                    max_length: 3000,
                    ..ConversationConfig::default()
                })?),
            })
        })
        .await
        .unwrap()
    }
}

impl TypeMapKey for Model {
    type Value = Arc<Model>;
}
