use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::version_data::VersionData;
use crate::auto_update::AutoUpdateData;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Package {
  pub package_name: String,
  pub display_name: String,
  pub aliases: Vec<String>,
  pub exec_name: String,
  pub portable: Option<bool>,
  pub creator: String,
  pub description: String,
  pub latest_version: String,
  pub threads: u64,
  pub iswitches: Vec<String>,
  pub uswitches: Vec<String>,
  pub autoupdate: AutoUpdateData,
  #[serde(flatten)]
  pub versions: HashMap<String, VersionData>,
}



