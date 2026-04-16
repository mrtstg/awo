use std::time::{Duration, SystemTime, SystemTimeError};

pub async fn wait_min_delay(
    start_date: SystemTime,
    delay: Duration,
) -> Result<(), SystemTimeError> {
    let elapsed = start_date.elapsed()?;
    if let Some(d) = delay.checked_sub(elapsed) {
        tokio::time::sleep(d).await;
    }
    Ok(())
}
