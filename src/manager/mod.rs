mod process;
use std::time::Duration;

use crate::{
    config::{Config, Process},
    manager::process::get_child_stdout,
};
use process::build_process_from_config;
use tokio::sync::mpsc::{channel, Receiver, Sender};

pub enum ManagerEvent {
    CreateProcess(Process),
    ProcessExited(i32, Process),
    ProcessStdout(String, Process),
}

pub struct ProcessManager {
    sender: Sender<ManagerEvent>,
    receiver: Receiver<ManagerEvent>,
}

impl ProcessManager {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = channel(capacity);
        ProcessManager { sender, receiver }
    }

    pub async fn init_process(&mut self, config: Config) {
        for process in config.run.values() {
            self.send(ManagerEvent::CreateProcess(process.clone()))
                .await
        }
    }

    pub async fn send(&mut self, payload: ManagerEvent) {
        let _ = self.sender.send(payload).await;
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.receiver.recv().await {
            let sender = self.sender.clone();
            match event {
                ManagerEvent::CreateProcess(process_cfg) => {
                    tokio::spawn(async move {
                        let child_res = build_process_from_config(process_cfg.clone());
                        match child_res {
                            Err(e) => {
                                println!("Failed to create process {}: {:?}", process_cfg.name, e);
                                tokio::time::sleep(Duration::from_secs(1)).await;
                                let _ = sender
                                    .send(ManagerEvent::ProcessExited(0, process_cfg))
                                    .await;
                            }
                            Ok(mut child) => {
                                println!("Created instance of {} process", process_cfg.name);
                                loop {
                                    for line in get_child_stdout(&mut child).await {
                                        let _ = sender
                                            .send(ManagerEvent::ProcessStdout(
                                                line,
                                                process_cfg.clone(),
                                            ))
                                            .await;
                                    }
                                    if let Ok(status) = child.try_wait() {
                                        if let Some(exit_code) = status {
                                            let _ = sender
                                                .send(ManagerEvent::ProcessExited(
                                                    exit_code.code().unwrap_or(0),
                                                    process_cfg.clone(),
                                                ))
                                                .await;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                ManagerEvent::ProcessExited(exit_code, process_cfg) => {
                    println!(
                        "Process {} exited with exit code {}",
                        process_cfg.name, exit_code
                    );
                }
                ManagerEvent::ProcessStdout(msg, process_cfg) => {
                    println!("{} | {}", process_cfg.name, msg);
                }
            }
        }
    }
}
