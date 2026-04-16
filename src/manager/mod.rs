mod process;
use std::time::Duration;

use crate::config::{Config, Process, ProcessBehavior};
use process::build_process_from_config;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, BufReader},
    sync::mpsc::{channel, Receiver, Sender},
};

pub enum ManagerEvent {
    CreateProcess(Process),
    ProcessExited(i32, Process),
    ProcessStdout(String, Process),
    ProcessStderr(String, Process),
    StopRunning,
}

pub struct ProcessManager {
    sender: Sender<ManagerEvent>,
    receiver: Receiver<ManagerEvent>,
    padding_width: usize,
    padding_enabled: bool,
}

impl ProcessManager {
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = channel(capacity);
        ProcessManager {
            sender,
            receiver,
            padding_width: 0,
            padding_enabled: true,
        }
    }

    pub async fn init_process(&mut self, config: &Config) {
        for process in config.run.values() {
            self.send(ManagerEvent::CreateProcess(process.clone()))
                .await
        }
        self.padding_width = config.run.values().map(|v| v.name.len()).max().unwrap_or(0);
        self.padding_enabled = config.align;
    }

    pub async fn send(&mut self, payload: ManagerEvent) {
        let _ = self.sender.send(payload).await;
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.receiver.recv().await {
            match event {
                ManagerEvent::CreateProcess(p) => self.handle_create(p).await,
                ManagerEvent::ProcessExited(code, p) => self.handle_exit(code, p).await,
                ManagerEvent::ProcessStdout(msg, p) => self.print_log(&p.name, msg),
                ManagerEvent::ProcessStderr(msg, p) => self.print_log(&p.name, msg),
                ManagerEvent::StopRunning => break,
            }
        }
    }

    async fn handle_create(&self, process_cfg: Process) {
        let sender = self.sender.clone();
        match build_process_from_config(process_cfg.clone()) {
            Err(e) => {
                eprintln!("Failed to create process {}: {:?}", process_cfg.name, e);
                let _ = sender
                    .send(ManagerEvent::ProcessExited(0, process_cfg))
                    .await;
            }
            Ok(mut child) => {
                self.print_log(&process_cfg.name, "Created instance".to_string());

                if let Some(out) = child.stdout.take() {
                    self.spawn_log_reader(
                        sender.clone(),
                        process_cfg.clone(),
                        BufReader::new(out),
                        true,
                    );
                }

                if let Some(err) = child.stderr.take() {
                    self.spawn_log_reader(
                        sender.clone(),
                        process_cfg.clone(),
                        BufReader::new(err),
                        false,
                    );
                }

                tokio::spawn(async move {
                    if let Ok(status) = child.wait().await {
                        let code = status.code().unwrap_or(0);
                        let _ = sender
                            .send(ManagerEvent::ProcessExited(code, process_cfg))
                            .await;
                    }
                });
            }
        }
    }

    fn spawn_log_reader<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
        &self,
        sender: Sender<ManagerEvent>,
        process: Process,
        reader: R,
        is_stdout: bool,
    ) {
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let event = if is_stdout {
                    ManagerEvent::ProcessStdout(line, process.clone())
                } else {
                    ManagerEvent::ProcessStderr(line, process.clone())
                };
                let _ = sender.send(event).await;
            }
        });
    }

    async fn handle_exit(&self, exit_code: i32, process_cfg: Process) {
        self.print_log(&process_cfg.name, format!("Exited with code {}", exit_code));

        let behavior = if exit_code == 0 {
            process_cfg.on_exit.clone()
        } else {
            process_cfg.on_error.clone()
        };

        match behavior {
            ProcessBehavior::Exit => {
                self.print_log(&process_cfg.name, "Stopping process manager...".to_string());
                let _ = self.sender.send(ManagerEvent::StopRunning).await;
            }
            ProcessBehavior::Restart => {
                self.print_log(&process_cfg.name, "Restarting...".to_string());
                tokio::time::sleep(Duration::from_secs(process_cfg.restart_delay)).await;
                let _ = self
                    .sender
                    .send(ManagerEvent::CreateProcess(process_cfg))
                    .await;
            }
            ProcessBehavior::Ignore => {}
        }
    }

    fn print_log(&self, name: &str, msg: String) {
        if self.padding_enabled {
            println!("{:<width$} | {}", name, msg, width = self.padding_width);
        } else {
            println!("{} | {}", name, msg);
        }
    }
}
