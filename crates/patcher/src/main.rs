#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
const ENGLISH_PARTS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/english-parts.bin"));
#[cfg(windows)]
const VIETNAMESE_PARTS: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/vietnamese-parts.bin"));
#[cfg(windows)]
const ENGLISH_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/english-payload.bin"));
#[cfg(windows)]
const VIETNAMESE_PAYLOAD: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/vietnamese-payload.bin"));
#[cfg(windows)]
const ENGLISH_ICON: &[u8] = include_bytes!("../../../content/assets/icons/english.ico");
#[cfg(windows)]
const VIETNAMESE_ICON: &[u8] = include_bytes!("../../../content/assets/icons/vietnamese.ico");

#[cfg(not(windows))]
fn main() {
    eprintln!("Doraemon patcher GUI is Windows-only. Use patch-build on this platform.");
}

#[cfg(windows)]
fn decode_parts_blob(blob: &[u8]) -> Option<Vec<Vec<u8>>> {
    if blob.len() < 7 || &blob[..5] != b"DPART" {
        return None;
    }
    let count = u16::from_le_bytes(blob[5..7].try_into().ok()?) as usize;
    let toc_size = 7 + count * 8;
    if blob.len() < toc_size {
        return None;
    }
    let mut parts = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 7 + i * 8;
        let start = u32::from_le_bytes(blob[offset..offset + 4].try_into().ok()?) as usize;
        let len = u32::from_le_bytes(blob[offset + 4..offset + 8].try_into().ok()?) as usize;
        if start + len > blob.len() {
            return None;
        }
        parts.push(blob[start..start + len].to_vec());
    }
    Some(parts)
}

#[cfg(windows)]
fn load_language(
    lang_name: &str,
    parts_blob: &[u8],
    monolithic: &[u8],
) -> Option<doraemon_game_patch::payload::Payload> {
    if parts_blob.len() < 5 {
        eprintln!("DIAG {lang_name}: parts_blob too small ({})", parts_blob.len());
    } else if &parts_blob[..5] != b"DPART" {
        eprintln!("DIAG {lang_name}: parts_blob has wrong magic ({:?}) — expecting DPART", &parts_blob[..5]);
    } else if let Some(part_bytes) = decode_parts_blob(parts_blob) {
        let mut parts = Vec::new();
        let mut decoded = 0u32;
        let mut errors = 0u32;
        for bytes in &part_bytes {
            if !bytes.is_empty() {
                match doraemon_game_patch::payload::decode_part(bytes) {
                    Ok(part) => { decoded += 1; parts.push(part); }
                    Err(e) => { errors += 1; eprintln!("DIAG {lang_name}: decode_part failed ({e})"); }
                }
            }
        }
        if decoded > 0 {
            eprintln!("DIAG {lang_name}: multipart OK — {decoded} parts decoded, {errors} skipped");
            match doraemon_game_patch::merge_parts(&parts) {
                Ok(payload) => return Some(payload),
                Err(error) => eprintln!("DIAG {lang_name}: multipart merge failed ({error})"),
            }
        }
        eprintln!("DIAG {lang_name}: multipart failed: {decoded} decoded, {errors} errors");
    }
    if !monolithic.is_empty() {
        eprintln!("DIAG {lang_name}: trying monolithic payload ({} bytes)", monolithic.len());
        match doraemon_game_patch::payload::decode(monolithic) {
            Ok(payload) => { eprintln!("DIAG {lang_name}: monolithic OK"); return Some(payload); }
            Err(e) => { eprintln!("DIAG {lang_name}: monolithic decode failed ({e})"); }
        }
    } else {
        eprintln!("DIAG {lang_name}: monolithic payload is empty");
    }
    eprintln!("DIAG {lang_name}: NO PAYLOAD AVAILABLE");
    None
}

#[cfg(windows)]
mod windows_app {
    use slint::{SharedString, Timer, TimerMode, VecModel};
    use doraemon_game_patch::{
        cue,
        install::{self, ApplyOptions, TaskProgress, TaskState},
        music,
        payload::{self, Payload},
    };
    use std::{
        cell::{Cell, RefCell},
        fs::OpenOptions,
        io::Write,
        panic::{self, AssertUnwindSafe},
        path::PathBuf,
        rc::Rc,
        sync::mpsc,
        thread,
        time::Duration,
    };

    slint::include_modules!();

    fn cue_files(game: &std::path::Path) -> Vec<PathBuf> {
        let mut cues: Vec<_> = std::fs::read_dir(game).into_iter().flatten()
            .filter_map(Result::ok).map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case("cue")))
            .collect();
        cues.sort();
        cues
    }

    fn find_cue(game: &std::path::Path) -> Option<PathBuf> {
        cue_files(game).into_iter().find(|p| cue::valid_cue(p))
    }

    fn has_music(game: &std::path::Path) -> bool {
        music::valid(&game.join("BGM.dat"))
            || find_cue(game).is_some()
            || cue::valid_wav(&game.join("DoraemonMusic.wav"))
    }

    fn music_text(game: &std::path::Path) -> String {
        let mut message = if music::valid(&game.join("BGM.dat")) {
            "♪ Local music is ready: BGM.dat found.".into()
        } else if cue::valid_wav(&game.join("DoraemonMusic.wav")) {
            "♪ DoraemonMusic.wav found. I'll compress it into BGM.dat when you apply.".into()
        } else if let Some(path) = find_cue(game) {
            format!("♪ Disc music found: {}. I'll prepare it when you apply.",
                path.file_name().unwrap_or_default().to_string_lossy())
        } else if let Some(path) = cue_files(game).into_iter().next() {
            format!("♫ I found {}, but its matching BIN is missing or incomplete. The game will be quiet for now.",
                path.file_name().unwrap_or_default().to_string_lossy())
        } else {
            String::new()
        };
        let legacy: Vec<_> = ["Music.dat", "doraudio.dll"].into_iter()
            .filter(|n| game.join(n).exists()).collect();
        if !legacy.is_empty() {
            if !message.is_empty() { message.push_str("  "); }
            message.push_str(&format!("Unused legacy {} found; this patcher will leave {} untouched.",
                legacy.join(" and "), if legacy.len() == 1 { "it" } else { "them" }));
        }
        message
    }

    fn write_diagnostic(game: &std::path::Path, state: TaskState, message: &str) {
        let state = match state { TaskState::Working => "WORKING", TaskState::Done => "DONE", TaskState::Skipped => "SKIPPED", TaskState::Failed => "FAILED" };
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(game.join("Doraemon-Patcher-diagnostic.log")) {
            let _ = writeln!(file, "[{state}] {message}"); let _ = file.flush(); let _ = file.sync_all();
        }
    }

    fn apply_game_icon(game: &std::path::Path, icon: &[u8]) -> Result<(), String> {
        use std::{ffi::OsStr, iter, os::windows::ffi::OsStrExt};
        extern "system" {
            fn BeginUpdateResourceW(path: *const u16, delete_existing: i32) -> *mut core::ffi::c_void;
            fn UpdateResourceW(handle: *mut core::ffi::c_void, kind: *const u16, name: *const u16, language: u16, data: *const core::ffi::c_void, len: u32) -> i32;
            fn EndUpdateResourceW(handle: *mut core::ffi::c_void, discard: i32) -> i32;
        }
        if icon.len() < 22 || icon[0..4] != [0, 0, 1, 0] { return Err("embedded game icon is invalid".into()); }
        let count = u16::from_le_bytes(icon[4..6].try_into().unwrap()) as usize;
        if count == 0 || count > 3 || icon.len() < 6 + count * 16 { return Err("embedded game icon directory is invalid".into()); }
        let mut images = Vec::with_capacity(count);
        let mut group = icon[..6].to_vec();
        for index in 0..count {
            let entry = &icon[6 + index * 16..22 + index * 16];
            let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let image = icon.get(offset..offset + size).ok_or("embedded game icon image is truncated")?;
            let resource_id = index as u16 + 1;
            group.extend_from_slice(&entry[..12]);
            group.extend_from_slice(&resource_id.to_le_bytes());
            images.push((resource_id, image));
        }
        let path: Vec<u16> = OsStr::new(&game.join("Doraemon.exe")).encode_wide().chain(iter::once(0)).collect();
        let handle = unsafe { BeginUpdateResourceW(path.as_ptr(), 0) };
        if handle.is_null() { return Err(format!("open executable resources: {}", std::io::Error::last_os_error())); }
        let mut images_ok = true;
        for (resource_id, image) in images {
            images_ok = images_ok && unsafe { UpdateResourceW(handle, 3usize as *const u16, resource_id as usize as *const u16, 0x409, image.as_ptr().cast(), image.len() as u32) } != 0;
        }
        let group_ok = images_ok && unsafe { UpdateResourceW(handle, 14usize as *const u16, 101usize as *const u16, 0x409, group.as_ptr().cast(), group.len() as u32) } != 0;
        let committed = unsafe { EndUpdateResourceW(handle, if group_ok { 0 } else { 1 }) } != 0;
        if group_ok && committed { Ok(()) } else { Err(format!("update executable icon: {}", std::io::Error::last_os_error())) }
    }

    fn empty_payload() -> Payload {
        Payload { language: payload::Language::Custom, profiles: Vec::new(), strings: None, voice: None, bundled: Vec::new() }
    }

    struct PendingApply {
        game: PathBuf, payload: Payload, icon: Option<&'static [u8]>, options: ApplyOptions, executable: PathBuf,
    }

    enum PendingDialog {
        KeepAudio(Box<PendingApply>),
        WrapperConfig { config_path: PathBuf },
    }

    fn log_color(state: TaskState) -> ( &'static str, slint::Color ) {
        match state {
            TaskState::Working => ("[....]", slint::Color::from_rgb_u8(37, 99, 235)),
            TaskState::Done    => ("[ OK ]", slint::Color::from_rgb_u8(22, 163, 74)),
            TaskState::Skipped => ("[SKIP]", slint::Color::from_rgb_u8(234, 179, 8)),
            TaskState::Failed  => ("[FAIL]", slint::Color::from_rgb_u8(220, 38, 38)),
        }
    }

    fn append_log(model: &VecModel<LogRow>, state: TaskState, message: &str) {
        let (marker, color) = log_color(state);
        let clean = message.replace('…', "...").replace(['—', '–'], "-");
        model.push(LogRow { text: SharedString::from(format!("{marker} {clean}")), color });
    }

    fn enable_controls(ui: &PatcherUI, busy: bool, rm: bool, backup: bool, wrapper: bool, play: bool, game: &std::path::Path) {
        ui.set_options_enabled(!rm && !busy);
        ui.set_audio_enabled(!rm && !busy);
        ui.set_apply_enabled(!rm && !busy);
        ui.set_restore_enabled(backup && !busy);
        ui.set_wrapper_enabled(wrapper && !rm && !busy);
        ui.set_play_enabled(play && !busy);
        ui.set_refresh_enabled(!rm && !busy);
        ui.set_local_audio_enabled(has_music(game) && !rm && !busy);
        ui.set_reduce_bgm_enabled(has_music(game) && !rm && !busy);
        let compress = !rm && !busy && (ui.get_reduce_bgm() || ui.get_optimize_voice());
        ui.set_compress_enabled(compress);
    }

    struct AppContext {
        restore_mode: bool, has_backup: bool, wrapper_available: bool, can_play: bool,
    }

    pub fn run() -> Result<(), String> {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let restore_mode = executable.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("Restore.exe"));
        let game_path = if restore_mode {
            executable.parent().and_then(std::path::Path::parent).ok_or("Restore.exe must be inside the backup folder")?.to_path_buf()
        } else {
            executable.parent().ok_or("the patcher executable has no parent game folder")?.to_path_buf()
        };

        eprintln!("DIAG: ENGLISH_PARTS size={} ENGLISH_PAYLOAD size={}", super::ENGLISH_PARTS.len(), super::ENGLISH_PAYLOAD.len());
        eprintln!("DIAG: VIETNAMESE_PARTS size={} VIETNAMESE_PAYLOAD size={}", super::VIETNAMESE_PARTS.len(), super::VIETNAMESE_PAYLOAD.len());
        let english = super::load_language("English", super::ENGLISH_PARTS, super::ENGLISH_PAYLOAD);
        let vietnamese = super::load_language("Vietnamese", super::VIETNAMESE_PARTS, super::VIETNAMESE_PAYLOAD);

        let mut languages = vec![SharedString::from("<original>")];
        let english_index = english.as_ref().map(|_| { let i = languages.len() as i32; languages.push(SharedString::from("English")); i });
        let vietnamese_index = vietnamese.as_ref().map(|_| { let i = languages.len() as i32; languages.push(SharedString::from("Vietnamese")); i });

        let wrapper_available = english.as_ref().or(vietnamese.as_ref()).is_some_and(|p|
            p.bundled.iter().any(|f| !f.name.eq_ignore_ascii_case("doraudio.dll")));

        let ui = Rc::new(PatcherUI::new().map_err(|e| e.to_string())?);
        let log_model: Rc<VecModel<LogRow>> = Rc::new(VecModel::default());
        ui.set_log_model(log_model.clone().into());
        let quality = ["Original", "High", "Balanced", "Compact"].into_iter().map(SharedString::from).collect::<Vec<_>>();
        ui.set_quality_items(slint::VecModel::from_slice(&quality));
        ui.set_language_index(0);
        ui.set_quality_index(0);
        ui.set_language_items(slint::VecModel::from_slice(&languages));
        ui.set_restore_mode(restore_mode);
        ui.set_no_disc(true);
        ui.set_no_reg(true);

        let ctx = Rc::new(AppContext {
            restore_mode,
            has_backup: game_path.join("backup").is_dir(),
            wrapper_available,
            can_play: game_path.join("Doraemon.exe").is_file(),
        });
        let busy = Rc::new(Cell::new(false));
        ui.set_subtitle(SharedString::from(if restore_mode { "Restore the exact original files kept in this backup." } else { "Choose a language, then pick the compatibility extras you want." }));
        ui.set_music_hint(SharedString::from(music_text(&game_path)));
        enable_controls(&ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, &game_path);

        append_log(&log_model, TaskState::Skipped, "Ready. Applying a new choice restores the original files first.");
        if restore_mode {
            ui.set_subtitle(SharedString::from("Restore Doraemon Monopoly"));
            append_log(&log_model, TaskState::Working, "Ready to restore the original game files.");
        }
        if english.is_none() && vietnamese.is_none() {
            append_log(&log_model, TaskState::Failed, &format!(
                "No embedded language payloads found. English parts blob={}B, Vietnamese parts blob={}B. The patch data may not have been linked in.",
                super::ENGLISH_PARTS.len(), super::VIETNAMESE_PARTS.len()));
        }

        let english = Rc::new(english);
        let vietnamese = Rc::new(vietnamese);
        let game = Rc::new(game_path);
        let (tx, rx) = mpsc::channel::<UiEvent>();
        let pending_dialog: Rc<RefCell<Option<PendingDialog>>> = Rc::new(RefCell::new(None));

        // ---- Close / Exit ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let close_busy = busy.clone();
            let close_log = log_model.clone();
            ui.on_exit_app(move || {
                if close_busy.get() { append_log(&close_log, TaskState::Working, "Please wait for the current task to finish."); }
                else if let Some(ui) = ui_weak.upgrade() { let _ = ui.window().hide(); }
            });
        }
        {
            let req_busy = busy.clone();
            ui.window().on_close_requested(move || {
                if req_busy.get() { slint::CloseRequestResponse::KeepWindowShown } else { slint::CloseRequestResponse::HideWindow }
            });
        }

        // ---- Dialog answer ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let dlg_pending = pending_dialog.clone();
            let dlg_busy = busy.clone();
            let dlg_ctx = ctx.clone();
            let dlg_tx = tx.clone();
            let dlg_game = game.clone();
            ui.on_dialog_answer(move |yes| {
                let Some(dlg_ui) = ui_weak.upgrade() else { return };
                match dlg_pending.borrow_mut().take() {
                    Some(PendingDialog::KeepAudio(mut p)) => {
                        dlg_ui.set_dialog_visible(false);
                        p.options.keep_compressed_audio = yes;
                        spawn_apply(&dlg_tx, *p, false);
                    }
                    Some(PendingDialog::WrapperConfig { config_path }) => {
                        dlg_ui.set_dialog_visible(false);
                        if yes { let _ = std::process::Command::new(&config_path).spawn(); }
                        dlg_busy.set(false);
                        enable_controls(&dlg_ui, false, dlg_ctx.restore_mode, dlg_ctx.has_backup, dlg_ctx.wrapper_available, dlg_ctx.can_play, &dlg_game);
                    }
                    None => { dlg_ui.set_dialog_visible(false); }
                }
            });
        }

        // ---- Apply patch ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let g = game.clone();
            let en = english.clone();
            let vi = vietnamese.clone();
            let ei = english_index;
            let vii = vietnamese_index;
            let exe = executable.clone();
            let t = tx.clone();
            let pd = pending_dialog.clone();
            let b = busy.clone();
            let c = ctx.clone();
            let l = log_model.clone();
            ui.on_apply_patch(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                if b.get() || c.restore_mode { return; }
                b.set(true);
                enable_controls(&ui, true, c.restore_mode, c.has_backup, c.wrapper_available, c.can_play, &g);
                append_log(&l, TaskState::Working, "Starting Apply…");
                let _ = std::fs::remove_file(g.join("Doraemon-Patcher-diagnostic.log"));
                write_diagnostic(&g, TaskState::Working, "Apply patch button pressed.");

                let backup = g.join("backup");
                let compressed = if backup.is_dir() { install::compressed_audio_files(&backup, &g).ok() } else { None };
                let ask = compressed.as_ref().is_some_and(|c| !c.is_empty());
                let sel = ui.get_language_index();
                let options = ApplyOptions {
                    no_disc: ui.get_no_disc(), no_reg: ui.get_no_reg(), local_audio: ui.get_local_audio(),
                    modern_volume: ui.get_modern_volume(), primary_audio_8bit: ui.get_primary_8bit(),
                    cue: find_cue(&g), reduce_bgm: false, optimize_voice: false,
                    voice_compression: doraemon_game_patch::voice::Compression::Original, keep_compressed_audio: false,
                };
                let payload = if Some(sel) == ei { en.as_ref().as_ref().expect("English available").clone() }
                    else if Some(sel) == vii { vi.as_ref().as_ref().expect("Vietnamese available").clone() }
                    else {
                        let mut p = en.as_ref().as_ref().or(vi.as_ref().as_ref()).cloned().unwrap_or_else(empty_payload);
                        p.language = payload::Language::Custom; p.profiles.clear(); p.strings = None; p.voice = None; p
                    };
                let icon = if Some(sel) == ei { Some(super::ENGLISH_ICON) } else if Some(sel) == vii { Some(super::VIETNAMESE_ICON) } else { None };
                let pending = PendingApply { game: (*g).clone(), payload, icon, options, executable: exe.clone() };
                if ask {
                    let list = compressed.unwrap().join("\n- ");
                    pd.borrow_mut().replace(PendingDialog::KeepAudio(Box::new(pending)));
                    ui.set_dialog_title(SharedString::from("Keep compressed audio?"));
                    ui.set_dialog_text(SharedString::from(format!("The following audio file{} been modified since the last patch:\n- {list}\n\nKeep the current compressed audio?\n(Choosing Yes will skip restoring and regenerating these files.)",
                        if list.contains('\n') { "s have" } else { " has" })));
                    ui.set_dialog_visible(true);
                } else {
                    spawn_apply(&t, pending, false);
                }
            });
        }

        // ---- Compress audio ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let g = game.clone();
            let en = english.clone();
            let vi = vietnamese.clone();
            let exe = executable.clone();
            let t = tx.clone();
            let b = busy.clone();
            let c = ctx.clone();
            let l = log_model.clone();
            ui.on_apply_audio(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                if b.get() || c.restore_mode { return; }
                if !(ui.get_reduce_bgm() || ui.get_optimize_voice()) { return; }
                b.set(true);
                enable_controls(&ui, true, c.restore_mode, c.has_backup, c.wrapper_available, c.can_play, &g);
                append_log(&l, TaskState::Working, "Preparing audio…");
                write_diagnostic(&g, TaskState::Working, "Apply audio button pressed.");
                let options = ApplyOptions {
                    no_disc: false, no_reg: false, local_audio: false, modern_volume: false, primary_audio_8bit: false,
                    cue: find_cue(&g), reduce_bgm: ui.get_reduce_bgm(), optimize_voice: ui.get_optimize_voice(),
                    voice_compression: match ui.get_quality_index() { 1 => doraemon_game_patch::voice::Compression::High, 2 => doraemon_game_patch::voice::Compression::Balanced, 3 => doraemon_game_patch::voice::Compression::Compact, _ => doraemon_game_patch::voice::Compression::Original },
                    keep_compressed_audio: false,
                };
                let mut payload = en.as_ref().as_ref().or(vi.as_ref().as_ref()).cloned().unwrap_or_else(empty_payload);
                payload.language = payload::Language::Custom; payload.profiles.clear(); payload.strings = None; payload.voice = None;
                spawn_apply(&t, PendingApply { game: (*g).clone(), payload, icon: None, options, executable: exe.clone() }, true);
            });
        }

        // ---- Restore ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let g = game.clone();
            let t = tx.clone();
            let b = busy.clone();
            let c = ctx.clone();
            let l = log_model.clone();
            ui.on_restore_backup(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                if b.get() { return; }
                b.set(true);
                enable_controls(&ui, true, c.restore_mode, c.has_backup, c.wrapper_available, c.can_play, &g);
                append_log(&l, TaskState::Working, "Restoring original files…");
                let backup = g.join("backup");
                let tx = t.clone();
                thread::spawn(move || {
                    let result = panic::catch_unwind(AssertUnwindSafe(|| install::restore(&backup)))
                        .unwrap_or_else(|_| Err("The restore task stopped unexpectedly; no files were restored.".into()));
                    let _ = tx.send(UiEvent::Restored(result));
                });
            });
        }

        // ---- Add wrapper ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let g = game.clone();
            let en = english.clone();
            let vi = vietnamese.clone();
            let t = tx.clone();
            let b = busy.clone();
            let c = ctx.clone();
            let l = log_model.clone();
            ui.on_add_wrapper(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                if b.get() || c.restore_mode { return; }
                b.set(true);
                enable_controls(&ui, true, c.restore_mode, c.has_backup, c.wrapper_available, c.can_play, &g);
                append_log(&l, TaskState::Working, "Adding the graphics wrapper…");
                let payload = en.as_ref().as_ref().or(vi.as_ref().as_ref()).cloned().unwrap_or_else(empty_payload);
                let tx = t.clone();
                let game = (*g).clone();
                thread::spawn(move || {
                    let result = panic::catch_unwind(AssertUnwindSafe(|| install::add_wrapper(&game, &payload)))
                        .unwrap_or_else(|_| Err("The graphics-wrapper task stopped unexpectedly; no files were added.".into()));
                    let _ = tx.send(UiEvent::Wrapper(result));
                });
            });
        }

        // ---- Play ----
        {
            let g = game.clone();
            let l = log_model.clone();
            ui.on_play_game(move || {
                let exe = g.join("Doraemon.exe");
                match std::process::Command::new(&exe).current_dir(&*g).spawn() {
                    Ok(_) => append_log(&l, TaskState::Done, "Launched Doraemon.exe."),
                    Err(e) => append_log(&l, TaskState::Failed, &format!("Could not launch Doraemon.exe: {e}")),
                }
            });
        }

        // ---- Refresh music ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let g = game.clone();
            let b = busy.clone();
            let c = ctx.clone();
            ui.on_refresh_music(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                if b.get() || c.restore_mode { return; }
                ui.set_music_hint(SharedString::from(music_text(&g)));
                let ok = has_music(&g);
                ui.set_local_audio_enabled(ok);
                ui.set_reduce_bgm_enabled(ok);
            });
        }

        // ---- Audio options toggled ----
        {
            let ui_weak = Rc::downgrade(&ui);
            let b = busy.clone();
            let c = ctx.clone();
            ui.on_audio_options_changed(move || {
                let Some(ui) = ui_weak.upgrade() else { return };
                let compress = !c.restore_mode && !b.get() && (ui.get_reduce_bgm() || ui.get_optimize_voice());
                ui.set_compress_enabled(compress);
            });
        }

        // ---- Event pump ----
        let timer_ui = Rc::downgrade(&ui);
        let timer = Timer::default();
        let timer_rx = rx;
        let timer_pd = pending_dialog.clone();
        let timer_ctx = ctx.clone();
        let timer_g = game.clone();
        let timer_b = busy.clone();
        let timer_l = log_model.clone();
        timer.start(TimerMode::Repeated, Duration::from_millis(120), move || {
            if let Some(ui) = timer_ui.upgrade() {
                drain_events(&ui, &timer_rx, &timer_l, &timer_b, &timer_ctx, &timer_g, &timer_pd);
            }
        });

        let result = ui.run().map_err(|e| e.to_string());
        timer.stop();
        result
    }

    fn spawn_apply(
        tx: &mpsc::Sender<UiEvent>,
        mut pending: PendingApply, audio_only: bool,
    ) {
        if audio_only {
            pending.options.no_disc = false; pending.options.no_reg = false;
            pending.options.local_audio = false; pending.options.modern_volume = false;
            pending.options.primary_audio_8bit = false;
        }
        let PendingApply { game: g, payload, icon, options, executable } = pending;
        let wants = payload.language != payload::Language::Custom || options.no_disc || options.no_reg
            || options.local_audio || options.modern_volume || options.primary_audio_8bit
            || options.reduce_bgm || options.optimize_voice;
        let tx = tx.clone();
        let game = g.clone();
        let game_clone = game.clone();
        thread::spawn(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                let backup = game.join("backup");
                if backup.is_dir() && !audio_only {
                    let mut saved: Vec<(String, Vec<u8>)> = Vec::new();
                    if options.keep_compressed_audio {
                        for name in &["voice.dat", "BGM.dat"] {
                            let path = game.join(name);
                            if path.exists() { if let Ok(data) = std::fs::read(&path) { saved.push((name.to_string(), data)); } }
                        }
                    }
                    let msg = if options.keep_compressed_audio { "Restoring original files while keeping your compressed audio…" }
                        else if wants { "Restoring the original files before applying your new choices…" }
                        else { "No patch choices selected; restoring the original files…" };
                    let _ = tx.send(UiEvent::Progress(TaskProgress { state: TaskState::Working, message: msg.into(), progress: Some(3) }));
                    let restored = install::restore(&backup)?;
                    for (name, data) in &saved { let _ = std::fs::write(game.join(name), data); }
                    let _ = tx.send(UiEvent::Progress(TaskProgress { state: TaskState::Done, message: format!("Original files restored: {}.", restored.join(", ")), progress: Some(10) }));
                    if !wants { return Ok(install::ApplyReport { changed: Vec::new(), audio: "Nothing selected, so the game is back to its original files.".into() }); }
                } else if !wants {
                    return Ok(install::ApplyReport { changed: Vec::new(), audio: "Nothing is selected and no backup was found. The game is unchanged.".into() });
                }
                let mut report = install::apply_with_progress(&game, &payload, &options, &executable, &mut |u| { let _ = tx.send(UiEvent::Progress(u)); })?;
                if let Some(icon) = icon {
                    let _ = tx.send(UiEvent::Progress(TaskProgress { state: TaskState::Working, message: "Applying the selected game icon…".into(), progress: Some(98) }));
                    apply_game_icon(&game, icon)?;
                    report.changed.push("Doraemon.exe icon".into());
                }
                Ok(report)
            })).unwrap_or_else(|_| Err("The patch task stopped unexpectedly; no files were installed.".into()));
            match &result {
                Ok(_) => write_diagnostic(&game_clone, TaskState::Done, "Apply finished successfully."),
                Err(e) => write_diagnostic(&game_clone, TaskState::Failed, &format!("Apply failed: {e}")),
            }
            let _ = tx.send(UiEvent::Finished(result));
        });
    }

    fn wrapper_config_prompt(ui: &PatcherUI, pd: &Rc<RefCell<Option<PendingDialog>>>, game: &std::path::Path) {
        let config_path = ["cnc-ddraw config.exe", "ddrawcfg.exe"].iter()
            .find_map(|n| { let p = game.join(n); if p.exists() { Some(p) } else { None } });
        let Some(config_path) = config_path else { return; };
        pd.borrow_mut().replace(PendingDialog::WrapperConfig { config_path });
        ui.set_dialog_title(SharedString::from("Graphics Wrapper"));
        ui.set_dialog_text(SharedString::from("The graphics wrapper has been installed.\n\nWould you like to open the configuration tool now?\n(Recommended for first-time use on Crossover or Wine.)"));
        ui.set_dialog_visible(true);
    }

    fn drain_events(
        ui: &PatcherUI, rx: &mpsc::Receiver<UiEvent>, log_model: &VecModel<LogRow>,
        busy: &Rc<Cell<bool>>, ctx: &AppContext, game: &std::path::Path,
        pd: &Rc<RefCell<Option<PendingDialog>>>,
    ) {
        while let Ok(event) = rx.try_recv() {
            match event {
                UiEvent::Progress(u) => { if let Some(pct) = u.progress { ui.set_progress_value(pct as i32); } append_log(log_model, u.state, &u.message); }
                UiEvent::Finished(Ok(r)) => {
                    ui.set_progress_value(100);
                    append_log(log_model, TaskState::Done, if r.changed.is_empty() { "Apply finished: everything requested was already in place." } else { "Apply finished successfully." });
                    append_log(log_model, TaskState::Done, &r.audio);
                    ui.set_music_hint(SharedString::from(music_text(game)));
                    let ok = has_music(game);
                    ui.set_local_audio_enabled(ok); ui.set_reduce_bgm_enabled(ok);
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                }
                UiEvent::Finished(Err(e)) => {
                    ui.set_progress_value(0);
                    append_log(log_model, TaskState::Failed, &format!("Apply failed: {e}"));
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                }
                UiEvent::Restored(Ok(files)) => {
                    ui.set_progress_value(100);
                    append_log(log_model, TaskState::Done, &format!("Restored and verified: {}.", files.join(", ")));
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                }
                UiEvent::Restored(Err(e)) => {
                    ui.set_progress_value(0);
                    append_log(log_model, TaskState::Failed, &format!("Restore failed: {e}"));
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                }
                UiEvent::Wrapper(Ok(files)) if files.is_empty() => {
                    ui.set_progress_value(100);
                    append_log(log_model, TaskState::Skipped, "The graphics wrapper is already installed.");
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                }
                UiEvent::Wrapper(Ok(files)) => {
                    ui.set_progress_value(100);
                    append_log(log_model, TaskState::Done, &format!("Graphics wrapper added: {} files.", files.len()));
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                    wrapper_config_prompt(ui, pd, game);
                }
                UiEvent::Wrapper(Err(e)) => {
                    ui.set_progress_value(0);
                    append_log(log_model, TaskState::Failed, &format!("Graphics wrapper failed: {e}"));
                    busy.set(false);
                    enable_controls(ui, false, ctx.restore_mode, ctx.has_backup, ctx.wrapper_available, ctx.can_play, game);
                }
            }
        }
    }

    enum UiEvent {
        Progress(TaskProgress),
        Finished(Result<install::ApplyReport, String>),
        Restored(Result<Vec<String>, String>),
        Wrapper(Result<Vec<String>, String>),
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows_app::run() {
        use std::iter;
        use winapi::um::winuser::{MB_OK, MessageBoxW};
        let wide: Vec<u16> = error.encode_utf16().chain(iter::once(0)).collect();
        let title: Vec<u16> = "Doraemon patcher".encode_utf16().chain(iter::once(0)).collect();
        unsafe { MessageBoxW(std::ptr::null_mut(), wide.as_ptr(), title.as_ptr(), MB_OK); }
    }
}
