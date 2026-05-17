use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::{env, thread};

use eframe::egui;
use eframe::egui::{CentralPanel, ProgressBar, Ui};
use eframe::{CreationContext, Frame};

use crate::remote::{RemoteResource, RemoteResources};

// ── Shared download progress ──

pub struct DownloadState {
    inner: Arc<Mutex<DownloadInner>>,
}

struct DownloadInner {
    fraction: f32,
    status: String,
    finished: bool,
}

impl Clone for DownloadState {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DownloadState {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(DownloadInner {
                fraction: 0.0,
                status: "等待中".into(),
                finished: false,
            })),
        }
    }

    pub fn update(&self, downloaded: u64, total: u64) {
        if let Ok(mut s) = self.inner.lock() {
            s.fraction = if total > 0 {
                (downloaded as f32 / total as f32).min(1.0)
            } else {
                0.0
            };
            s.status = "下载中".into();
        }
    }

    pub fn set_finished(&self, success: bool) {
        if let Ok(mut s) = self.inner.lock() {
            s.finished = true;
            s.status = if success {
                "完成".into()
            } else {
                "失败".into()
            };
            if success {
                s.fraction = 1.0;
            }
        }
    }

    pub fn snapshot(&self) -> (f32, String, bool) {
        if let Ok(s) = self.inner.lock() {
            (s.fraction, s.status.clone(), s.finished)
        } else {
            (0.0, "未知".into(), false)
        }
    }
}

// ── Language category (from filesystem) ──

struct LangCategory {
    name: String,
    env_name: String,
    path: PathBuf,
}

impl LangCategory {
    fn installed_versions(&self) -> Vec<String> {
        let mut versions = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.path) {
            for entry in entries.flatten() {
                if entry.file_type().map_or(false, |t| t.is_dir()) {
                    versions.push(entry.file_name().to_string_lossy().to_string());
                }
            }
        }
        versions.sort();
        versions
    }
}

// ── Main Application ──

pub struct EnvManager {
    base_path: PathBuf,
    categories: Vec<LangCategory>,
    selected: usize,
    selected_version: String,
    resource_url: String,
    resources: RemoteResources,
    downloads: HashMap<String, DownloadState>,
    show_settings: bool,
    error_msg: Option<String>,
}

impl EnvManager {
    pub fn new(_cc: &CreationContext<'_>) -> Self {
        let base_path = env::var("ENV_PATH")
            .ok()
            .and_then(|p| {
                let pb = PathBuf::from(&p);
                if pb.is_dir() { Some(pb) } else { None }
            })
            .unwrap_or_else(|| env::current_dir().unwrap());

        let resource_url = env::var("RESOURCE_URL")
            .unwrap_or_else(|_| "https://raw.githubusercontent.com/Crazy-ccc/env_manager/master/resource/resource.json".into());

        let categories = Self::load_categories(&base_path);
        let resources = Self::load_resources(&resource_url);

        Self {
            base_path,
            categories,
            selected: 0,
            selected_version: String::new(),
            resource_url,
            resources,
            downloads: HashMap::new(),
            show_settings: false,
            error_msg: None,
        }
    }

    fn supported_lang() -> HashMap<&'static str, &'static str> {
        let mut m = HashMap::new();
        m.insert("java", "JAVA_HOME");
        m.insert("scala", "SCALA_HOME");
        m.insert("go", "GO_HOME");
        m
    }

    fn load_categories(base: &PathBuf) -> Vec<LangCategory> {
        let lang_map = Self::supported_lang();
        let mut cats = Vec::new();
        if let Ok(entries) = fs::read_dir(base) {
            for entry in entries.flatten() {
                if entry.file_type().map_or(true, |t| t.is_file()) {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().trim().to_string();
                if let Some(&env_name) = lang_map.get(name.as_str()) {
                    cats.push(LangCategory {
                        env_name: env_name.to_string(),
                        path: entry.path(),
                        name,
                    });
                }
            }
        }
        cats
    }

    fn load_resources(url: &str) -> RemoteResources {
        RemoteResource::fetch_remote_json(url).unwrap_or_default()
    }

    fn current_category(&self) -> Option<&LangCategory> {
        self.categories.get(self.selected)
    }

    fn current_key(&self) -> &str {
        self.current_category()
            .map(|c| c.name.as_str())
            .unwrap_or("")
    }

    fn current_resources(&self) -> &[RemoteResource] {
        let key = self.current_key();
        self.resources.get(key).map_or(&[], |v| v.as_slice())
    }

    /// Remote resources that are NOT yet installed locally
    fn downloadable_resources(&self) -> Vec<RemoteResource> {
        let installed = self
            .current_category()
            .map(|c| c.installed_versions())
            .unwrap_or_default();

        self.current_resources()
            .iter()
            .filter(|r| !installed.contains(&r.name))
            .cloned()
            .collect()
    }

    fn refresh_resources(&mut self) {
        let resources = Self::load_resources(&self.resource_url);
        if resources.is_empty() {
            self.error_msg = Some("从远程加载资源列表失败".into());
        } else {
            self.resources = resources;
            self.error_msg = None;
        }
    }

    // ── UI: main page ──

    fn ui_lang_tabs(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            for (i, cat) in self.categories.iter().enumerate() {
                let clicked = ui
                    .add(egui::RadioButton::new(self.selected == i, &cat.name))
                    .clicked();
                if clicked {
                    self.selected = i;
                }
            }
        });
    }

    fn ui_installed_versions(&mut self, ui: &mut Ui) {
        let Some(cat) = self.current_category() else {
            return;
        };
        let env_name = cat.env_name.clone();
        let cat_path = cat.path.clone();
        let versions = cat.installed_versions();
        // cat borrow ends here — env_name and cat_path are owned copies

        if versions.is_empty() {
            ui.label("暂无已安装的 SDK 版本");
            return;
        }

        ui.label(format!("已安装的 {} 版本：", env_name));

        for v in &versions {
            let is_current = Self::check_env_var(&env_name, &cat_path, v);
            let selected = is_current;

            let label = if is_current {
                format!("{} (当前)", v)
            } else {
                v.clone()
            };

            if ui.radio(selected, &label).clicked() {
                self.selected_version = v.clone();
                Self::set_env(&env_name, &cat_path, v);
                self.error_msg = Some(format!("已将 {} 设为 {}", env_name, v));
            }
        }
    }

    fn check_env_var(env_name: &str, cat_path: &PathBuf, version: &str) -> bool {
        env::var(env_name)
            .is_ok_and(|val| cat_path.join(version).to_string_lossy().as_ref() == val.trim())
    }

    fn set_env(env_name: &str, cat_path: &PathBuf, version: &str) {
        let path = cat_path.join(version);
        unsafe {
            env::set_var(env_name, &path);
        }
        let _ = Command::new("setx").arg(env_name).arg(&path).spawn();
    }

    // ── UI: downloads window ──

    fn ui_downloadable_list(&mut self, ui: &mut Ui) {
        let items = self.downloadable_resources();

        if items.is_empty() {
            ui.label("所有版本已安装");
            return;
        }

        for res in &items {
            let key = res.name.clone();

            ui.horizontal(|ui| {
                ui.label(&res.name);

                if let Some(state) = self.downloads.get(&key) {
                    let (frac, status, finished) = state.snapshot();
                    if finished {
                        ui.label(status);
                        if ui.button("清除").clicked() {
                            self.downloads.remove(&key);
                        }
                    } else {
                        ui.add(
                            ProgressBar::new(frac)
                                .text(format!("{} ({:.0}%)", status, frac * 100.0))
                                .desired_width(200.0),
                        );
                    }
                } else {
                    if ui.button("下载").clicked() {
                        let res = res.clone();
                        let state = DownloadState::new();
                        let thread_state = state.clone();
                        self.downloads.insert(key.clone(), state);
                        let cat_name = self.current_key().to_string();
                        let download_dir = self.base_path.join("download");
                        let extract_dir = self.base_path.join(&cat_name);
                        thread::spawn(move || {
                            let result = res.download_to_with_progress(
                                &download_dir,
                                &extract_dir,
                                |d, t| {
                                    thread_state.update(d, t);
                                },
                            );
                            thread_state.set_finished(result.is_ok());
                        });
                    }
                }
            });
        }
    }
}

impl eframe::App for EnvManager {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        CentralPanel::default().show_inside(ui, |ui| {
            // ── Language tabs ──
            if !self.categories.is_empty() {
                self.ui_lang_tabs(ui);
            }
            ui.separator();

            // ── Installed SDK versions ──
            self.ui_installed_versions(ui);

            // ── Error / status message ──
            if let Some(msg) = &self.error_msg {
                ui.label(egui::RichText::new(msg).color(egui::Color32::GREEN));
            }

            ui.separator();

            // ── More button ──
            if ui.button("获取更多sdk").clicked() {
                self.show_settings = !self.show_settings;
            }
        });

        // ── Downloads / settings window ──
        let mut open = self.show_settings;
        egui::Window::new("下载 SDK")
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.label("可下载的 SDK 版本（未安装）：");
                self.ui_downloadable_list(ui);

                ui.separator();
                if ui.button("刷新资源列表").clicked() {
                    self.refresh_resources();
                }
            });
        self.show_settings = open;
    }
}
