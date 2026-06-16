use crate::backup_runner::{run_backup, BackupStatus, LogMessage};
use crate::config::{load_config, save_config, BackupConfig};
use crate::ui::{entry_list, progress, settings, settings::EditState};
use eframe::egui;
use std::sync::mpsc;
use std::thread;

/// 主应用结构体
pub struct BackupApp {
    /// 备份配置
    config: BackupConfig,
    /// UI 编辑状态
    edit_state: EditState,
    /// 备份执行状态
    backup_status: BackupStatus,
    /// 日志消息列表
    logs: Vec<LogMessage>,
    /// 是否正在备份中
    is_running: bool,
    /// 接收后台日志消息的 channel receiver
    log_receiver: Option<mpsc::Receiver<LogMessage>>,
    /// 接收后台状态变更的 channel receiver
    status_receiver: Option<mpsc::Receiver<BackupStatus>>,
}

impl BackupApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config = load_config();
        tracing::info!("已加载配置 ({} 条备份条目)", config.entries.len());

        BackupApp {
            config,
            edit_state: EditState::default(),
            backup_status: BackupStatus::Idle,
            logs: Vec::new(),
            is_running: false,
            log_receiver: None,
            status_receiver: None,
        }
    }

    /// 检查后台消息并更新 UI
    fn poll_messages(&mut self) {
        // 接收日志消息
        if let Some(rx) = &self.log_receiver {
            while let Ok(msg) = rx.try_recv() {
                self.logs.push(msg);
                self.logs.truncate(500); // 限制日志数量
            }
        }

        // 接收状态变更
        let should_stop = if let Some(rx) = &self.status_receiver {
            let mut stop = false;
            loop {
                match rx.try_recv() {
                    Ok(status) => {
                        self.backup_status = status;
                        if self.backup_status.is_finished() {
                            stop = true;
                        }
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        // 线程已结束但没发 Done/Error → 可能 panic 了
                        // 但如果之前已经收到过完成状态，不要覆盖
                        if self.is_running && !self.backup_status.is_finished() {
                            self.backup_status = BackupStatus::Error(
                                "后台任务异常终止，请检查终端日志".into(),
                            );
                            stop = true;
                        }
                        break;
                    }
                }
            }
            stop
        } else {
            false
        };

        if should_stop {
            self.is_running = false;
            self.log_receiver = None;
            self.status_receiver = None;
        }
    }

    /// 开始备份
    fn start_backup(&mut self) {
        if self.is_running {
            return;
        }

        // 如果密码不为空，同步到配置
        if !self.edit_state.password_buffer.is_empty() {
            self.config.password = Some(self.edit_state.password_buffer.clone());
        }

        let config = self.config.clone();
        let (log_tx, log_rx) = mpsc::channel();
        let (status_tx, status_rx) = mpsc::channel();

        self.log_receiver = Some(log_rx);
        self.status_receiver = Some(status_rx);
        self.is_running = true;
        self.backup_status = BackupStatus::Collecting;

        // 在后台线程中执行备份
        thread::spawn(move || {
            run_backup(config, log_tx, status_tx);
        });
    }

    /// 保存配置
    fn save_config(&mut self) {
        // 同步密码到配置
        if !self.edit_state.password_buffer.is_empty() {
            self.config.password = Some(self.edit_state.password_buffer.clone());
        }
        match save_config(&self.config) {
            Ok(_) => {
                self.logs.push(LogMessage::ok("配置已保存"));
            }
            Err(e) => {
                self.logs.push(LogMessage::error(e));
            }
        }
    }

    /// 清空日志
    fn clear_logs(&mut self) {
        self.logs.clear();
    }
}

impl eframe::App for BackupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 轮询后台消息
        self.poll_messages();

        // 如果正在运行，持续刷新 UI
        if self.is_running {
            ctx.request_repaint();
        }

        // 顶部标题栏（固定）
        egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("备份工具")
                        .size(22.0)
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(
                        egui::Color32::GRAY,
                        format!("v{}", env!("CARGO_PKG_VERSION")),
                    );
                });
            });
            ui.add_space(4.0);
        });

        // 底部操作栏（固定）
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.separator();
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // 主操作按钮
                let backup_btn = egui::Button::new(
                    egui::RichText::new("开始备份").strong().color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(46, 125, 50))
                .rounding(egui::Rounding::same(6.0))
                .min_size(egui::vec2(120.0, 32.0));

                if ui
                    .add_enabled(!self.is_running, backup_btn)
                    .clicked()
                {
                    self.start_backup();
                }

                let save_btn = egui::Button::new(
                    egui::RichText::new("保存配置").strong(),
                )
                .rounding(egui::Rounding::same(6.0))
                .min_size(egui::vec2(110.0, 32.0));

                if ui.add(save_btn).clicked() {
                    self.save_config();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let clear_btn = egui::Button::new(
                        egui::RichText::new("清空日志"),
                    )
                    .rounding(egui::Rounding::same(6.0))
                    .min_size(egui::vec2(100.0, 32.0));

                    if ui.add(clear_btn).clicked() {
                        self.clear_logs();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // 中央内容区（可滚动）
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // 1. 备份条目列表
                entry_list::render_entry_list(
                    ui,
                    &mut self.config.entries,
                    &mut self.edit_state,
                );

                ui.add_space(10.0);

                // 2. 压缩设置
                settings::render_compression_settings(
                    ui,
                    &mut self.config,
                    &mut self.edit_state,
                );

                ui.add_space(10.0);

                // 3. 路径设置
                settings::render_path_settings(ui, &mut self.config);

                // 4 & 5. 日志/进度面板
                ui.add_space(10.0);
                progress::render_progress(ui, &self.logs, &self.backup_status);
            });
        });

        // 关闭前自动保存配置
        if ctx.input(|i| i.viewport().close_requested()) {
            let _ = save_config(&self.config);
        }
    }
}
