use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VersionData {
  pub url: String,
  pub size: u64,
  pub checksum: String,
  pub file_type: String,
}