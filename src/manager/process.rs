use crate::config::Process;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};
use tokio::fs;
use tokio::process::*;

pub async fn get_modified_timestamp(path: &PathBuf) -> u64 {
    match fs::metadata(path).await {
        Err(_) => return 0,
        Ok(m) => match m.modified() {
            Err(_) => return 0,
            Ok(d) => {
                return d
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs()
            }
        },
    }
}
