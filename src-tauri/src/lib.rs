mod controller;
mod host;
mod injector;
mod media;
mod models;
mod network;
mod payload;
mod preview;
mod settings;

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use controller::CodexController;
use media::MediaLibrary;
use models::{
    AppSnapshot, ApplyRequest, DownloadRequest, ImportResult, SettingsPatch, SkippedImport,
};
use network::download_remote_media;
use payload::{build_active_payload, ActivePayload};
use preview::MediaServer;
use settings::SettingsStore;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

const SNAPSHOT_EVENT: &str = "background:snapshot-changed";

fn data_directory() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("CodexBackgroundStudio")
}

struct StudioState {
    data_directory: PathBuf,
    settings: Mutex<SettingsStore>,
    library: Mutex<MediaLibrary>,
    media_server: Mutex<MediaServer>,
    controller: Arc<Mutex<CodexController>>,
    tray: Mutex<Option<host::TrayUi>>,
    quitting: AtomicBool,
    slideshow_busy: AtomicBool,
}

fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>, String> {
    value.lock().map_err(|_| "应用状态锁已损坏。".to_string())
}

impl StudioState {
    fn load() -> Result<Self, String> {
        let data_directory = data_directory();
        let settings = SettingsStore::load(&data_directory)?;
        let library = MediaLibrary::load(&data_directory)?;
        let media_server = MediaServer::start(&library)?;
        let controller = CodexController::load(&data_directory);
        Ok(Self {
            data_directory: data_directory.clone(),
            settings: Mutex::new(settings),
            library: Mutex::new(library),
            media_server: Mutex::new(media_server),
            controller: Arc::new(Mutex::new(controller)),
            tray: Mutex::new(None),
            quitting: AtomicBool::new(false),
            slideshow_busy: AtomicBool::new(false),
        })
    }

    fn snapshot(&self) -> Result<AppSnapshot, String> {
        let settings = lock(&self.settings)?.value();
        let library = lock(&self.library)?;
        let media_server = lock(&self.media_server)?;
        let items = library
            .items()
            .into_iter()
            .map(|mut item| {
                item.preview_url = Some(format!(
                    "{}?v={}",
                    media_server.url_for(&item.id),
                    item.sha256.chars().take(12).collect::<String>()
                ));
                item
            })
            .collect();
        Ok(AppSnapshot {
            settings,
            library: items,
            runtime: lock(&self.controller)?.status(),
            data_directory: self.data_directory.to_string_lossy().into_owned(),
        })
    }

    fn sync_preview(&self) -> Result<(), String> {
        let library = lock(&self.library)?;
        lock(&self.media_server)?.sync(&library);
        Ok(())
    }

    fn integrate_import(&self, result: &ImportResult) -> Result<(), String> {
        if result.added.is_empty() {
            return Ok(());
        }
        let mut store = lock(&self.settings)?;
        let mut settings = store.value();
        let new_ids = result
            .added
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        if settings.active_media_id.is_none() {
            settings.active_media_id = new_ids.first().cloned();
        }
        for id in new_ids {
            if !settings.playlist_ids.contains(&id) {
                settings.playlist_ids.push(id);
            }
        }
        store.save(settings)?;
        drop(store);
        self.sync_preview()
    }

    fn emit_snapshot(&self, app: &AppHandle) -> Result<AppSnapshot, String> {
        let snapshot = self.snapshot()?;
        app.emit(SNAPSHOT_EVENT, &snapshot)
            .map_err(|error| error.to_string())?;
        if let Ok(tray) = lock(&self.tray) {
            if let Some(tray) = tray.as_ref() {
                host::update_tray(app, tray);
            }
        }
        Ok(snapshot)
    }

    fn active_payload(&self) -> Result<ActivePayload, String> {
        let settings = lock(&self.settings)?.value();
        let id = settings
            .active_media_id
            .as_deref()
            .ok_or_else(|| "请先从媒体库选择一张图片或一个视频。".to_string())?;
        let library = lock(&self.library)?;
        let item = library
            .get_by_id(id)
            .ok_or_else(|| "请先从媒体库选择一张图片或一个视频。".to_string())?;
        let path = library.path_for(&item)?;
        build_active_payload(&item, &path, &settings.display)
    }
}

async fn apply_live_if_active(state: &StudioState) -> Result<(), String> {
    if lock(&state.controller)?.status().phase != "active" {
        return Ok(());
    }
    let payload = match state.active_payload() {
        Ok(payload) => payload,
        Err(error) if error.contains("请先从媒体库选择") => return Ok(()),
        Err(error) => return Err(error),
    };
    let controller = Arc::clone(&state.controller);
    tauri::async_runtime::spawn_blocking(move || {
        lock(&controller)?.apply(payload.script, payload.revision, false)
    })
    .await
    .map_err(|error| error.to_string())??;
    Ok(())
}

async fn refresh_dynamic_item(state: &StudioState, id: &str) -> Result<(), String> {
    let (source_url, temporary_directory) = {
        let library = lock(&state.library)?;
        let item = library
            .get_by_id(id)
            .ok_or_else(|| "媒体项目不存在。".to_string())?;
        if item.origin != models::MediaOrigin::Api {
            return Err("该媒体不是随机 API 来源。".to_string());
        }
        (
            item.source_url
                .ok_or_else(|| "随机 API 条目缺少来源地址。".to_string())?,
            library.temporary_directory.clone(),
        )
    };
    let download = download_remote_media(&source_url, &temporary_directory).await?;
    lock(&state.library)?.refresh_with_download(id, download)?;
    state.sync_preview()
}

async fn advance_slideshow(app: AppHandle) -> Result<(), String> {
    let state = app.state::<StudioState>();
    if state.slideshow_busy.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    let result = async {
        let settings = lock(&state.settings)?.value();
        if !settings.slideshow.enabled {
            return Ok(());
        }
        let candidates = {
            let library = lock(&state.library)?;
            let mut candidates = settings
                .playlist_ids
                .iter()
                .filter(|id| library.get_by_id(id).is_some())
                .cloned()
                .collect::<Vec<_>>();
            if candidates.is_empty() {
                candidates = library.items().iter().map(|item| item.id.clone()).collect();
            }
            candidates
        };
        if candidates.is_empty() {
            return Ok(());
        }
        if candidates.len() == 1 {
            let is_dynamic = lock(&state.library)?
                .get_by_id(&candidates[0])
                .map(|item| item.origin == models::MediaOrigin::Api)
                .unwrap_or(false);
            if !is_dynamic {
                return Ok(());
            }
        }
        let next_id = match settings.slideshow.order {
            models::SlideshowOrder::Sequential => {
                let current = settings
                    .active_media_id
                    .as_ref()
                    .and_then(|active| candidates.iter().position(|id| id == active));
                candidates[(current.map(|index| index + 1).unwrap_or(0)) % candidates.len()].clone()
            }
            models::SlideshowOrder::Random => {
                let choices = if candidates.len() > 1 {
                    candidates
                        .iter()
                        .filter(|id| Some(id.as_str()) != settings.active_media_id.as_deref())
                        .collect::<Vec<_>>()
                } else {
                    candidates.iter().collect()
                };
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_nanos())
                    .unwrap_or_default();
                (*choices[(seed % choices.len() as u128) as usize]).clone()
            }
        };
        let is_dynamic = lock(&state.library)?
            .get_by_id(&next_id)
            .map(|item| item.origin == models::MediaOrigin::Api)
            .unwrap_or(false);
        if is_dynamic {
            // 网络抖动时沿用这个 API 条目的现有缓存，轮播仍继续。
            let _ = refresh_dynamic_item(&state, &next_id).await;
        }
        {
            let mut store = lock(&state.settings)?;
            let mut updated = store.value();
            updated.active_media_id = Some(next_id);
            store.save(updated)?;
        }
        apply_live_if_active(&state).await?;
        state.emit_snapshot(&app)?;
        Ok(())
    }
    .await;
    state.slideshow_busy.store(false, Ordering::SeqCst);
    result
}

fn start_slideshow_scheduler(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_tick = Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let state = app.state::<StudioState>();
            let (enabled, interval, active) = {
                let settings = lock(&state.settings)
                    .map(|store| store.value())
                    .unwrap_or_default();
                let active = lock(&state.controller)
                    .map(|controller| controller.status().phase == "active")
                    .unwrap_or(false);
                (
                    settings.slideshow.enabled,
                    settings.slideshow.interval_seconds.max(5),
                    active,
                )
            };
            if !enabled || !active {
                last_tick = Instant::now();
                continue;
            }
            if last_tick.elapsed().as_secs() < interval {
                continue;
            }
            last_tick = Instant::now();
            let _ = advance_slideshow(app.clone()).await;
        }
    });
}

#[tauri::command]
fn get_snapshot(state: State<'_, StudioState>) -> Result<AppSnapshot, String> {
    state.snapshot()
}

#[tauri::command]
async fn choose_media_files(
    app: AppHandle,
    state: State<'_, StudioState>,
) -> Result<ImportResult, String> {
    let paths = app
        .dialog()
        .file()
        .set_title("选择背景图片或视频")
        .add_filter(
            "图片和视频",
            &[
                "png", "jpg", "jpeg", "webp", "gif", "avif", "mp4", "webm", "ogv", "mov",
            ],
        )
        .blocking_pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| path.into_path().ok())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Ok(ImportResult::default());
    }
    let result = lock(&state.library)?.import_files(&paths);
    state.integrate_import(&result)?;
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)?;
    Ok(result)
}

#[tauri::command]
async fn choose_media_folder(
    app: AppHandle,
    state: State<'_, StudioState>,
) -> Result<ImportResult, String> {
    let Some(folder) = app
        .dialog()
        .file()
        .set_title("导入背景文件夹")
        .blocking_pick_folder()
        .and_then(|path| path.into_path().ok())
    else {
        return Ok(ImportResult::default());
    };
    let paths = lock(&state.library)?.discover_folder(&folder)?;
    let result = lock(&state.library)?.import_files(&paths);
    state.integrate_import(&result)?;
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)?;
    Ok(result)
}

#[tauri::command]
async fn add_remote_media(
    app: AppHandle,
    state: State<'_, StudioState>,
    request: DownloadRequest,
) -> Result<ImportResult, String> {
    if request.url.len() > 4096 {
        return Err("网络地址无效。".to_string());
    }
    let temporary_directory = lock(&state.library)?.temporary_directory.clone();
    let download = match download_remote_media(&request.url, &temporary_directory).await {
        Ok(download) => download,
        Err(error) => {
            return Ok(ImportResult {
                added: Vec::new(),
                skipped: vec![SkippedImport {
                    path: request.url,
                    reason: error,
                }],
            });
        }
    };
    let result = lock(&state.library)?.import_download(&request.url, request.dynamic, download);
    state.integrate_import(&result)?;
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)?;
    Ok(result)
}

#[tauri::command]
async fn refresh_media(
    app: AppHandle,
    state: State<'_, StudioState>,
    id: String,
) -> Result<AppSnapshot, String> {
    refresh_dynamic_item(&state, &id).await?;
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)
}

#[tauri::command]
async fn remove_media(
    app: AppHandle,
    state: State<'_, StudioState>,
    id: String,
) -> Result<AppSnapshot, String> {
    lock(&state.library)?.remove(&id)?;
    {
        let mut store = lock(&state.settings)?;
        let mut settings = store.value();
        settings.playlist_ids.retain(|candidate| candidate != &id);
        if settings.active_media_id.as_deref() == Some(id.as_str()) {
            settings.active_media_id = settings.playlist_ids.first().cloned().or_else(|| {
                lock(&state.library)
                    .ok()?
                    .items()
                    .first()
                    .map(|item| item.id.clone())
            });
        }
        store.save(settings)?;
    }
    state.sync_preview()?;
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)
}

#[tauri::command]
async fn set_active_media(
    app: AppHandle,
    state: State<'_, StudioState>,
    id: String,
) -> Result<AppSnapshot, String> {
    if lock(&state.library)?.get_by_id(&id).is_none() {
        return Err("媒体项目不存在。".to_string());
    }
    {
        let mut store = lock(&state.settings)?;
        let mut settings = store.value();
        settings.active_media_id = Some(id.clone());
        if !settings.playlist_ids.contains(&id) {
            settings.playlist_ids.push(id);
        }
        store.save(settings)?;
    }
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)
}

#[tauri::command]
async fn update_settings(
    app: AppHandle,
    state: State<'_, StudioState>,
    patch: SettingsPatch,
) -> Result<AppSnapshot, String> {
    let behavior = lock(&state.settings)?.patch(patch)?.behavior;
    host::sync_autostart(behavior.auto_start_with_windows, behavior.start_minimized)?;
    apply_live_if_active(&state).await?;
    state.emit_snapshot(&app)
}

#[tauri::command]
async fn apply_background(
    app: AppHandle,
    state: State<'_, StudioState>,
    request: Option<ApplyRequest>,
) -> Result<AppSnapshot, String> {
    let payload = state.active_payload()?;
    let restart_requested = request
        .and_then(|request| request.restart_existing)
        .unwrap_or(false);
    let run_apply = |restart_existing: bool, script: String, revision: String| {
        let controller = Arc::clone(&state.controller);
        tauri::async_runtime::spawn_blocking(move || {
            lock(&controller)?.apply(script, revision, restart_existing)
        })
    };
    let first = run_apply(
        restart_requested,
        payload.script.clone(),
        payload.revision.clone(),
    )
    .await
    .map_err(|error| error.to_string())?;
    if let Err(error) = first {
        if !restart_requested && error.contains("需要重启一次") {
            let confirmed = app
                .dialog()
                .message(
                    "未发送的输入可能丢失。背景管理器只会关闭经过官方 Store 包身份校验的 Codex 进程。",
                )
                .title("应用背景需要重启一次 Codex")
                .buttons(MessageDialogButtons::OkCancelCustom(
                    "重启并应用".to_string(),
                    "取消".to_string(),
                ))
                .blocking_show();
            if !confirmed {
                return state.emit_snapshot(&app);
            }
            let retry = run_apply(true, payload.script, payload.revision)
                .await
                .map_err(|error| error.to_string())?;
            if let Err(error) = retry {
                let _ = state.emit_snapshot(&app);
                return Err(error);
            }
        } else {
            let _ = state.emit_snapshot(&app);
            return Err(error);
        }
    }
    state.emit_snapshot(&app)
}

#[tauri::command]
async fn pause_background(
    app: AppHandle,
    state: State<'_, StudioState>,
) -> Result<AppSnapshot, String> {
    let controller = Arc::clone(&state.controller);
    tauri::async_runtime::spawn_blocking(move || lock(&controller)?.pause())
        .await
        .map_err(|error| error.to_string())??;
    state.emit_snapshot(&app)
}

#[tauri::command]
async fn restore_background(
    app: AppHandle,
    state: State<'_, StudioState>,
) -> Result<AppSnapshot, String> {
    let controller = Arc::clone(&state.controller);
    tauri::async_runtime::spawn_blocking(move || lock(&controller)?.restore())
        .await
        .map_err(|error| error.to_string())??;
    state.emit_snapshot(&app)
}

#[tauri::command]
fn open_data_directory(state: State<'_, StudioState>) -> Result<(), String> {
    host::open_data_directory(&state.data_directory)
}

#[tauri::command]
fn show_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口。".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_window(app.clone());
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = StudioState::load()
                .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            if let Ok(payload) = state.active_payload() {
                let _ = lock(&state.controller)
                    .and_then(|mut controller| {
                        controller
                            .reconnect_saved(payload.script, payload.revision)
                            .map(|_| ())
                    });
            }
            let settings = lock(&state.settings)
                .map_err(std::io::Error::other)?
                .value();
            host::sync_autostart(
                settings.behavior.auto_start_with_windows,
                settings.behavior.start_minimized,
            )
            .map_err(std::io::Error::other)?;
            let start_hidden = settings.behavior.start_minimized
                || std::env::args().any(|argument| argument == "--hidden");
            app.manage(state);
            let tray = host::setup_tray(app.handle()).map_err(std::io::Error::other)?;
            let managed = app.state::<StudioState>();
            *lock(&managed.tray).map_err(std::io::Error::other)? = Some(tray);
            if let Ok(tray) = lock(&managed.tray) {
                if let Some(tray) = tray.as_ref() {
                    host::update_tray(app.handle(), tray);
                }
            }
            if !start_hidden {
                show_window(app.handle().clone()).map_err(std::io::Error::other)?;
            }
            start_slideshow_scheduler(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle().clone();
                let state = app.state::<StudioState>();
                if state.quitting.load(Ordering::SeqCst) {
                    return;
                }
                let close_to_tray = lock(&state.settings)
                    .map(|settings| settings.value().behavior.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    let _ = window.hide();
                } else {
                    tauri::async_runtime::spawn(host::quit_and_restore(app));
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            choose_media_files,
            choose_media_folder,
            add_remote_media,
            refresh_media,
            remove_media,
            set_active_media,
            update_settings,
            apply_background,
            pause_background,
            restore_background,
            open_data_directory,
            show_window
        ])
        .run(tauri::generate_context!())
        .expect("运行 Codex Background Studio 失败");
}
