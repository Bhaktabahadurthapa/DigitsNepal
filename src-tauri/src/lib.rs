//! DigitsNepal — Nepali-English writing and translation assistant.
//!
//! Layout:
//! * `translation` — pure translation engine (providers, prompts, validation)
//! * `accessibility` — selected-text read/replace through macOS AX APIs
//! * `shortcuts`, `clipboard`, `security`, `storage` — native integration
//! * `commands` — the IPC surface exposed to the webview

pub mod accessibility;
pub mod clipboard;
pub mod commands;
pub mod coworker;
pub mod security;
pub mod shortcuts;
pub mod state;
pub mod storage;
pub mod translation;

use state::{AppState, PendingCapture};
use std::sync::Mutex;
use storage::history::HistoryStore;
use storage::settings::{Settings, SettingsStore};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use translation::types::TranslationMode;

const TRAY_ID: &str = "digitsnepal-tray";

pub fn show_main(app: &AppHandle, tab: Option<&str>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
    if let Some(t) = tab {
        let _ = app.emit_to("main", "navigate", t);
    }
}

/// Apply window-level preferences to every DigitsNepal window.
pub fn apply_window_prefs(app: &AppHandle, s: &Settings) {
    for label in ["main", "popover"] {
        if let Some(w) = app.get_webview_window(label) {
            if let Err(e) = w.set_content_protected(s.hide_from_capture) {
                log::warn!("could not set content protection: {e}");
            }
        }
    }
}

fn set_pending(app: &AppHandle, mut p: PendingCapture) {
    let state = app.state::<AppState>();
    let mut seq = state.seq.lock().unwrap();
    *seq += 1;
    p.seq = *seq;
    p.clipboard_fallback = state.settings.lock().unwrap().clipboard_fallback;
    *state.pending.lock().unwrap() = p.clone();
    drop(seq);
    if let Some(w) = app.get_webview_window("popover") {
        let _ = w.center();
        let _ = w.show();
        let _ = w.set_focus();
    }
    let _ = app.emit_to("popover", "popover:capture", p);
}

/// Shortcut pressed: capture the selection *before* our window takes focus.
fn on_shortcut(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.settings.lock().unwrap().paused {
        return;
    }
    match accessibility::capture() {
        Ok(cap) => {
            let text = cap.text.clone();
            *state.capture.lock().unwrap() = Some(cap);
            set_pending(
                app,
                PendingCapture {
                    status: "ok".into(),
                    text: Some(text),
                    can_replace: true,
                    ..Default::default()
                },
            );
        }
        Err(e) => {
            *state.capture.lock().unwrap() = None;
            let code = serde_json::to_value(&e)
                .ok()
                .and_then(|v| v["code"].as_str().map(String::from))
                .unwrap_or_default();
            set_pending(
                app,
                PendingCapture {
                    status: code,
                    message: Some(e.to_string()),
                    ..Default::default()
                },
            );
        }
    }
}

fn translate_clipboard(app: &AppHandle) {
    let state = app.state::<AppState>();
    *state.capture.lock().unwrap() = None;
    match clipboard::read_text(app) {
        Ok(t) if !t.trim().is_empty() => set_pending(
            app,
            PendingCapture {
                status: "clipboard".into(),
                text: Some(t),
                ..Default::default()
            },
        ),
        _ => set_pending(
            app,
            PendingCapture {
                status: "no_selection".into(),
                message: Some("The clipboard has no text.".into()),
                ..Default::default()
            },
        ),
    }
}

/// (Re)register the global shortcut according to settings. Returns an error
/// message if the OS or another app already owns the combination.
pub fn register_shortcut(app: &AppHandle, s: &Settings) -> Result<(), String> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    if !s.shortcut_enabled || s.paused {
        return Ok(());
    }
    gs.on_shortcut(s.shortcut.as_str(), |app, _sc, ev| {
        if ev.state() == ShortcutState::Pressed {
            on_shortcut(app);
        }
    })
    .map_err(|e| {
        format!(
            "Could not register {} — it may already be used by another app ({e}).",
            s.shortcut
        )
    })
}

fn build_tray_menu(app: &AppHandle, s: &Settings) -> tauri::Result<Menu<tauri::Wry>> {
    let open = MenuItem::with_id(app, "open", "Open DigitsNepal", true, None::<&str>)?;
    let clip = MenuItem::with_id(app, "clipboard", "Translate Clipboard Text", true, None::<&str>)?;
    let has_undo = app.state::<AppState>().last_replacement.lock().unwrap().is_some();
    let undo = MenuItem::with_id(app, "undo", "Undo Last Replacement", has_undo, None::<&str>)?;
    let modes: Vec<CheckMenuItem<tauri::Wry>> = TranslationMode::ALL
        .iter()
        .map(|m| {
            CheckMenuItem::with_id(
                app,
                format!("mode:{}", m.id()),
                m.label(),
                true,
                *m == s.default_mode,
                None::<&str>,
            )
        })
        .collect::<Result<_, _>>()?;
    let mode_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = modes.iter().map(|m| m as _).collect();
    let mode_menu = Submenu::with_items(app, "Translation Mode", true, &mode_refs)?;
    let hide = CheckMenuItem::with_id(
        app,
        "hide_capture",
        "Hide from Screen Sharing",
        true,
        s.hide_from_capture,
        None::<&str>,
    )?;
    let pause = MenuItem::with_id(
        app,
        "pause",
        if s.paused {
            "Resume Shortcut & Auto-Translate"
        } else {
            "Pause Shortcut & Auto-Translate"
        },
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit DigitsNepal", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    Menu::with_items(
        app,
        &[
            &open,
            &clip,
            &undo,
            &sep()?,
            &mode_menu,
            &hide,
            &pause,
            &sep()?,
            &settings,
            &sep()?,
            &quit,
        ],
    )
}

pub fn refresh_tray(app: &AppHandle) {
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_tray_menu(app, &s)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn update_settings_from_native(app: &AppHandle, f: impl FnOnce(&mut Settings)) {
    let state = app.state::<AppState>();
    let next = {
        let mut s = state.settings.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = state.settings_store.save(&next);
    let _ = register_shortcut(app, &next);
    apply_window_prefs(app, &next);
    refresh_tray(app);
    let _ = app.emit("settings:changed", &next);
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => show_main(app, None),
        "clipboard" => translate_clipboard(app),
        "settings" => show_main(app, Some("settings")),
        "pause" => update_settings_from_native(app, |s| s.paused = !s.paused),
        "hide_capture" => update_settings_from_native(app, |s| s.hide_from_capture = !s.hide_from_capture),
        "quit" => app.exit(0),
        "undo" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let result = commands::undo_replacement(app.state::<AppState>()).await;
                if let Err(e) = result {
                    // Show the reason in the popover rather than failing silently.
                    set_pending(
                        &app,
                        PendingCapture {
                            status: "no_selection".into(),
                            message: Some(e.message),
                            ..Default::default()
                        },
                    );
                }
                refresh_tray(&app);
            });
        }
        other => {
            if let Some(m) = other.strip_prefix("mode:").and_then(TranslationMode::from_id) {
                update_settings_from_native(app, |s| s.default_mode = m);
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            let data = app.path().app_data_dir()?;
            let store = SettingsStore::new(&dir);
            let settings = store.load();
            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                settings_store: store,
                history: Mutex::new(HistoryStore::open(&data)),
                capture: Mutex::new(None),
                last_replacement: Mutex::new(None),
                pending: Mutex::new(PendingCapture::default()),
                seq: Mutex::new(0),
                secrets: Mutex::new(Default::default()),
            });

            #[cfg(feature = "coworker")]
            {
                use coworker::{
                    ledger::Ledger,
                    service::{Config, Coworker},
                };
                let ledger = std::sync::Arc::new(Ledger::open(&data.join("coworker").join("coworker.db"))?);
                let emitter = app.handle().clone();
                let sink: coworker::service::Sink = std::sync::Arc::new(move |v| {
                    let _ = emitter.emit("coworker:event", v);
                });
                let cw = Coworker::new(
                    ledger,
                    Config {
                        state_dir: data.join("coworker").join("engine"),
                        workspace_dir: data.join("coworker").join("workspace"),
                        backup_dir: data.join("coworker").join("backups"),
                        runner: None,
                        web: Default::default(),
                        tool_note: None,
                    },
                    sink,
                );
                cw.set_translator(std::sync::Arc::new(commands::coworker::AppTranslator(
                    app.handle().clone(),
                )));
                app.manage(commands::coworker::CoworkerState(cw));
            }

            let handle = app.handle().clone();
            apply_window_prefs(&handle, &settings);
            if let Err(e) = register_shortcut(&handle, &settings) {
                log::warn!("{e}");
                let _ = handle.emit("shortcut:error", e);
            }

            let menu = build_tray_menu(&handle, &settings)?;
            let mut tray = TrayIconBuilder::with_id(TRAY_ID)
                .tooltip("DigitsNepal")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()));
            if let Ok(icon) = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png")) {
                tray = tray.icon(icon).icon_as_template(true);
            } else if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the main window keeps DigitsNepal in the menu bar.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate,
            commands::detect_language,
            commands::dictionary_gloss,
            commands::list_modes,
            commands::list_models,
            commands::provider_status,
            commands::list_providers,
            commands::set_api_key,
            commands::delete_api_key,
            commands::get_settings,
            commands::update_settings,
            commands::history_search,
            commands::history_save,
            commands::history_favorite,
            commands::history_delete,
            commands::history_clear,
            commands::history_export,
            commands::copy_text,
            commands::read_clipboard_text,
            commands::get_pending_capture,
            commands::replace_selection,
            commands::undo_replacement,
            commands::dismiss_popover,
            commands::accessibility_status,
            commands::request_accessibility,
            commands::open_main,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_status,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_add_grant,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_revoke_grant,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_sessions,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_messages,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_new_session,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_send,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_decide,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_answer,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_cancel,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_activity,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_pending,
            commands::coworker::coworker_specialists,
            commands::coworker::coworker_security,
            commands::coworker::coworker_task_preview,
            commands::coworker::coworker_task_create,
            commands::coworker::coworker_tasks,
            commands::coworker::coworker_task,
            commands::coworker::coworker_task_control,
            commands::coworker::coworker_task_metrics,
            commands::coworker::coworker_set_security,
            commands::coworker::coworker_standing,
            commands::coworker::coworker_revoke_standing,
            commands::coworker::coworker_stop_all,
            commands::coworker::coworker_mcp_servers,
            commands::coworker::coworker_mcp_add,
            commands::coworker::coworker_mcp_enable,
            commands::coworker::coworker_mcp_remove,
            commands::coworker::coworker_automations,
            commands::coworker::coworker_automation_delete,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_resume,
            #[cfg(feature = "coworker")]
            commands::coworker::coworker_usage,
        ])
        .build(tauri::generate_context!())
        .expect("error while building DigitsNepal")
        .run(|app, event| {
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app, None);
            }
        });
}
