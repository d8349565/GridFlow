use crate::{engine::Engine, model::*};
use std::{path::PathBuf, sync::Arc};
use tauri::{Manager, State};

type Core<'a> = State<'a, Arc<Engine>>;
fn join_error(error: impl std::fmt::Display) -> AppError {
    AppError::new("WORKER", error.to_string())
}
#[tauri::command]
fn workspace(core: Core<'_>) -> Result<Workspace> {
    core.snapshot()
}
#[tauri::command]
fn save_draft(core: Core<'_>, rules: RuleSet, active_sheet_id: Option<String>) -> Result<()> {
    core.save_draft(rules, active_sheet_id)
}
#[tauri::command]
async fn import_files(core: Core<'_>, paths: Vec<String>) -> Result<Vec<ImportItem>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.import_files(paths))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn load_sample_files(core: Core<'_>) -> Result<Vec<ImportItem>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.load_sample_files())
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn preview_sheet(
    core: Core<'_>,
    sheet_id: String,
    offset: usize,
    limit: usize,
    header_row: Option<usize>,
) -> Result<DataPage> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.preview_sheet(&sheet_id, offset, limit, header_row)
    })
    .await
    .map_err(join_error)?
}
#[tauri::command]
async fn preview_appearance(core: Core<'_>, sheet_id: String, offset: usize, limit: usize) -> Result<SheetAppearance> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.preview_appearance(&sheet_id,offset,limit)).await.map_err(join_error)?
}
#[tauri::command]
async fn available_columns(core: Core<'_>, rules: RuleSet) -> Result<Vec<Column>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.available_columns(&rules))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn input_columns(core: Core<'_>, rules: RuleSet) -> Result<Vec<Column>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.input_columns(&rules))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn suggestions(core: Core<'_>, rules: RuleSet) -> Result<Vec<TypeSuggestion>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.suggestions(&rules))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn preview_rules(core: Core<'_>, rules: RuleSet) -> Result<Dataset> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.preview_rules(rules))
        .await
        .map_err(join_error)?
}
#[tauri::command]
fn submit_task(core: Core<'_>, rules: RuleSet) -> Result<Task> {
    core.submit(rules)
}
#[tauri::command]
fn retry_task(core: Core<'_>, task_id: String) -> Result<Task> {
    core.retry(&task_id)
}
#[tauri::command]
fn cancel_task(core: Core<'_>, task_id: String) -> Result<()> {
    core.cancel(&task_id)
}
#[tauri::command]
fn tasks(core: Core<'_>) -> Result<Vec<Task>> {
    core.guard()?;
    Ok(core.snapshot()?.tasks)
}
#[tauri::command]
async fn query_dataset(core: Core<'_>, query: DatasetQuery) -> Result<DataPage> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.query(query))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn distinct_values(
    core: Core<'_>,
    query: DatasetQuery,
    key: String,
    offset: usize,
) -> Result<Vec<String>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.distinct_values(query, key, offset))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn aggregate(core: Core<'_>, spec: PivotSpec) -> Result<Analysis> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.aggregate(spec, Some(2000)))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn issues(core: Core<'_>, dataset_id: String, offset: usize) -> Result<Vec<Issue>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.issues(&dataset_id, offset))
        .await
        .map_err(join_error)?
}
#[tauri::command]
fn save_scheme(core: Core<'_>, name: String, rules: RuleSet) -> Result<Scheme> {
    core.save_scheme(name, rules)
}
#[tauri::command]
fn rename_scheme(core: Core<'_>, scheme_id: String, name: String) -> Result<Scheme> {
    core.rename_scheme(&scheme_id, name)
}
#[tauri::command]
fn apply_scheme(core: Core<'_>, scheme_id: String, rules: RuleSet) -> Result<RuleSet> {
    core.apply_scheme(&scheme_id, rules)
}
#[tauri::command]
async fn import_scheme(core: Core<'_>, path: String) -> Result<Scheme> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.import_scheme(path))
        .await
        .map_err(join_error)?
}
#[tauri::command]
fn export_scheme(core: Core<'_>, scheme_id: String, path: String) -> Result<()> {
    core.export_scheme(scheme_id, path)
}
#[tauri::command]
async fn import_results(core: Core<'_>, paths: Vec<String>) -> Result<Vec<Dataset>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.import_results(paths))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn export_dataset(core: Core<'_>, query: DatasetQuery, path: String) -> Result<String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.export_dataset(query, path))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn export_analysis(core: Core<'_>, spec: PivotSpec, path: String) -> Result<String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.export_analysis(spec, path))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn export_workbook(core: Core<'_>, queries: Vec<DatasetQuery>, path: String) -> Result<String> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.export_workbook(queries, path)).await.map_err(join_error)?
}
#[tauri::command]
async fn export_batch(
    core: Core<'_>,
    ids: Vec<String>,
    directory: String,
    zip_path: Option<String>,
    format: String,
) -> Result<Vec<String>> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        core.export_batch(ids, directory, zip_path, format)
    })
    .await
    .map_err(join_error)?
}
#[tauri::command]
fn save_image(core: Core<'_>, path: String, bytes: Vec<u8>, dataset_id: Option<String>) -> Result<String> {
    if let Some(id) = &dataset_id { core.dataset(id)?; }
    let saved = core.save_image(path, bytes)?;
    if let Some(id) = dataset_id { core.record_export(&id, &PathBuf::from(&saved))?; }
    Ok(saved)
}
#[tauri::command]
async fn delete_items(core: Core<'_>, kind: String, ids: Vec<String>) -> Result<()> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.delete_items(&kind, &ids))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn unlock(core: Core<'_>, password: String) -> Result<()> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.unlock(&password))
        .await
        .map_err(join_error)?
}
#[tauri::command]
async fn set_password(core: Core<'_>, password: String, current_password: String) -> Result<()> {
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || core.set_password(&password, &current_password))
        .await
        .map_err(join_error)?
}
#[tauri::command]
fn lock(core: Core<'_>) -> Result<()> {
    core.lock()
}
#[tauri::command]
fn disk_usage(core: Core<'_>) -> Result<u64> {
    core.disk_usage()
}
#[tauri::command]
fn open_path(core: Core<'_>, path: String, select: bool) -> Result<()> {
    if path.starts_with("http://") || path.starts_with("https://") {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let mut cmd = std::process::Command::new("rundll32.exe");
            cmd.creation_flags(0x08000000);
            cmd.arg("url.dll,FileProtocolHandler");
            cmd.arg(&path);
            cmd.spawn().map_err(|e| AppError::new("SPAWN", e.to_string()))?;
        }
        #[cfg(not(windows))]
        {
            let _ = std::process::Command::new("xdg-open").arg(&path).spawn();
        }
        return Ok(());
    }
    core.guard()?;
    let path = PathBuf::from(path);
    let ws = core.snapshot()?;
    if !(path.starts_with(&core.root)
        || ws
            .datasets
            .iter()
            .flat_map(|d| &d.exports)
            .any(|p| PathBuf::from(p) == path))
    {
        return Err(AppError::new(
            "PATH",
            "只能打开工作台的数据目录或已导出的文件",
        ));
    }
    if !path.exists() {
        return Err(AppError::new("NOT_FOUND", "文件已被移动或删除"));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new("explorer.exe");
        cmd.creation_flags(0x08000000);
        if select && path.is_file() {
            cmd.raw_arg(format!("/select,\"{}\"", path.display()));
        } else {
            cmd.arg(path);
        }
        cmd.spawn()?;
    }
    Ok(())
}
pub fn run() {
    #[cfg(windows)]
    if tauri::webview_version().is_err() {
        native_message("缺少 WebView2 运行库。请运行单独提供的微软签名 MicrosoftEdgeWebview2RuntimeInstallerX64.exe，然后重新启动工作台。");
        return;
    }
    let exe = std::env::current_exe().expect("executable path");
    // 开发数据限定在仓库的 data 目录；发布版把 data 放在程序旁。
    let portable = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("data")
    } else {
        exe.parent().unwrap().join("data")
    };
    let test = portable.join(".write-check");
    let can_write = std::fs::create_dir_all(&portable)
        .and_then(|_| std::fs::write(&test, b"test"))
        .is_ok();
    let _ = std::fs::remove_file(test);
    let root = if let Some(override_root) = std::env::var_os("GRIDFLOW_DATA_DIR").or_else(|| std::env::var_os("EXCEL_WORKBENCH_DATA_DIR")) {
        PathBuf::from(override_root)
    } else if can_write {
        portable
    } else {
        let base = PathBuf::from(
            std::env::var_os("LOCALAPPDATA")
                .unwrap_or_else(|| std::env::temp_dir().into_os_string()),
        );
        let modern = base.join("GridFlow").join("data");
        let legacy = base.join("ExcelWorkbenchV2").join("data");
        if legacy.exists() && !modern.exists() {
            legacy
        } else {
            modern
        }
    };
    // The single-instance lock belongs to one workspace, rather than every portable copy.
    use std::hash::{Hash, Hasher};
    let mut workspace_hash = std::collections::hash_map::DefaultHasher::new();
    let _ = std::fs::create_dir_all(&root);
    std::fs::canonicalize(&root).unwrap_or_else(|_| root.clone()).to_string_lossy().to_lowercase().hash(&mut workspace_hash);
    let mut context = tauri::generate_context!();
    context.config_mut().identifier.push_str(&format!(".workspace-{:016x}",workspace_hash.finish()));
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            app.manage(Engine::open(&root)?);
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let scale = monitor.scale_factor();
                    let width = (monitor.size().width as f64 / scale - 70.).clamp(1024., 1440.);
                    let height = (monitor.size().height as f64 / scale - 100.).clamp(640., 900.);
                    let _ = window.set_size(tauri::LogicalSize::new(width, height));
                    let _ = window.center();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            workspace,
            save_draft,
            import_files,
            load_sample_files,
            preview_sheet,
            preview_appearance,
            available_columns,
            input_columns,
            suggestions,
            preview_rules,
            submit_task,
            retry_task,
            cancel_task,
            tasks,
            query_dataset,
            distinct_values,
            aggregate,
            issues,
            save_scheme,
            rename_scheme,
            apply_scheme,
            import_scheme,
            export_scheme,
            import_results,
            export_dataset,
            export_analysis,
            export_workbook,
            export_batch,
            save_image,
            delete_items,
            unlock,
            set_password,
            lock,
            disk_usage,
            open_path
        ])
        .run(context)
        .unwrap_or_else(|error| {
            native_message(&format!(
                "无法启动汇格 · GridFlow：{error}\n请检查数据目录权限和磁盘空间。"
            ))
        });
}
fn native_message(message: &str) {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn MessageBoxW(
                window: *mut std::ffi::c_void,
                text: *const u16,
                title: *const u16,
                flags: u32,
            ) -> i32;
        }
        let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        let title: Vec<u16> = "汇格 · GridFlow".encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
    #[cfg(not(windows))]
    eprintln!("{message}");
}
