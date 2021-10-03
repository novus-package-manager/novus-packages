use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AutoUpdateData {
  pub download_page: String,
  pub download_url: String,
  pub regex: String,
}