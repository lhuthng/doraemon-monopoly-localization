#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
const ENGLISH_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/english-payload.bin"));
#[cfg(windows)]
const VIETNAMESE_PAYLOAD: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/vietnamese-payload.bin"));
#[cfg(windows)]
const ENGLISH_ICON: &[u8] = include_bytes!("../../../assets/icons/english.ico");
#[cfg(windows)]
const VIETNAMESE_ICON: &[u8] = include_bytes!("../../../assets/icons/vietnamese.ico");

#[cfg(not(windows))]
fn main() {
    eprintln!("Doraemon patcher GUI is Windows-only. Use patch-build on this platform.");
}

#[cfg(windows)]
mod windows_app {
    extern crate native_windows_gui as nwg;

    use doraemon_game_patch::{
        cue,
        install::{self, ApplyOptions, TaskProgress, TaskState},
        music,
        payload::{self, Payload},
    };
    use std::{
        cell::Cell,
        fs::OpenOptions,
        io::Write,
        panic::{self, AssertUnwindSafe},
        path::PathBuf,
        rc::Rc,
        sync::mpsc,
        thread,
        time::Duration,
    };

    const WS_CHILD: u32 = 0x40000000;
    const WS_VISIBLE: u32 = 0x10000000;
    const BS_GROUPBOX: u32 = 0x00000007;
    const WM_SETFONT: u32 = 0x0030;

    #[derive(Default)]
    struct Ui {
        window: nwg::Window,
        title_bar: nwg::Label,
        subtitle: nwg::Label,
        title_font: nwg::Font,
        group_font: nwg::Font,
        options_group: nwg::ControlHandle,
        game_label: nwg::Label,
        language: nwg::ComboBox<String>,
        no_disc: nwg::CheckBox,
        no_reg: nwg::CheckBox,
        local_audio: nwg::CheckBox,
        modern_volume: nwg::CheckBox,

        music: nwg::Label,
        refresh_music: nwg::Button,
        actions_group: nwg::ControlHandle,
        apply: nwg::Button,
        restore: nwg::Button,
        wrapper: nwg::Button,
        play: nwg::Button,
        progress: nwg::ProgressBar,
        log_group: nwg::ControlHandle,
        log: nwg::RichTextBox,
        exit: nwg::Button,
        timer: nwg::AnimationTimer,
    }

    impl Drop for Ui {
        fn drop(&mut self) {
            self.options_group.destroy();
            self.actions_group.destroy();
            self.log_group.destroy();
        }
    }

    enum UiEvent {
        Progress(TaskProgress),
        Finished(Result<install::ApplyReport, String>),
        Restored(Result<Vec<String>, String>),
        Wrapper(Result<Vec<String>, String>),
    }

    fn cue_files(game: &std::path::Path) -> Vec<PathBuf> {
        let mut cues: Vec<_> = std::fs::read_dir(game)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("cue"))
            })
            .collect();
        cues.sort();
        cues
    }

    fn find_cue(game: &std::path::Path) -> Option<PathBuf> {
        cue_files(game)
            .into_iter()
            .find(|path| cue::valid_cue(path))
    }

    fn music_text(game: &std::path::Path) -> String {
        if music::valid(&game.join("Music.dat")) {
            "♪ Local music is ready: Music.dat found.".into()
        } else if cue::valid_wav(&game.join("DoraemonMusic.wav")) {
            "♪ DoraemonMusic.wav found. I’ll compress it into Music.dat when you apply.".into()
        } else if let Some(path) = find_cue(game) {
            format!(
                "♪ Disc music found: {}. I'll prepare it when you apply.",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        } else if let Some(path) = cue_files(game).into_iter().next() {
            format!(
                "♫ I found {}, but its matching BIN is missing or incomplete. The game will be quiet for now.",
                path.file_name().unwrap_or_default().to_string_lossy()
            )
        } else {
            "♫ No Music.dat, WAV, or CUE/BIN here yet. Leave local music off to keep the original CD playback."
                .into()
        }
    }

    fn append_log(ui: &Ui, state: TaskState, message: &str) {
        let marker = match state {
            TaskState::Working => "●",
            TaskState::Done => "✓",
            TaskState::Skipped => "–",
            TaskState::Failed => "✕",
        };
        let color = match state {
            TaskState::Working => [49, 91, 148],
            TaskState::Done => [35, 116, 75],
            TaskState::Skipped => [104, 100, 90],
            TaskState::Failed => [173, 54, 54],
        };
        let start = ui.log.len();
        ui.log.appendln(&format!("{marker} {message}"));
        let end = ui.log.len();
        ui.log.set_selection(start..end);
        ui.log.set_char_format(&nwg::CharFormat {
            text_color: Some(color),
            ..Default::default()
        });
        ui.log.set_selection(end..end);
    }

    fn apply_game_icon(game: &std::path::Path, icon: &[u8]) -> Result<(), String> {
        use std::{ffi::OsStr, iter, os::windows::ffi::OsStrExt};
        extern "system" {
            fn BeginUpdateResourceW(
                path: *const u16,
                delete_existing: i32,
            ) -> *mut core::ffi::c_void;
            fn UpdateResourceW(
                handle: *mut core::ffi::c_void,
                kind: *const u16,
                name: *const u16,
                language: u16,
                data: *const core::ffi::c_void,
                len: u32,
            ) -> i32;
            fn EndUpdateResourceW(handle: *mut core::ffi::c_void, discard: i32) -> i32;
        }
        if icon.len() < 22 || &icon[0..4] != [0, 0, 1, 0] {
            return Err("embedded game icon is invalid".into());
        }
        let size = u32::from_le_bytes(icon[14..18].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(icon[18..22].try_into().unwrap()) as usize;
        let image = icon
            .get(offset..offset + size)
            .ok_or("embedded game icon is truncated")?;
        let mut group = vec![0, 0, 1, 0, 1, 0, 32, 32, 0, 0, 1, 0, 32, 0];
        group.extend_from_slice(&(image.len() as u32).to_le_bytes());
        group.extend_from_slice(&2u16.to_le_bytes()); // existing 32x32 icon resource ID
        let path: Vec<u16> = OsStr::new(&game.join("Doraemon.exe"))
            .encode_wide()
            .chain(iter::once(0))
            .collect();
        let handle = unsafe { BeginUpdateResourceW(path.as_ptr(), 0) };
        if handle.is_null() {
            return Err(format!(
                "open executable resources: {}",
                std::io::Error::last_os_error()
            ));
        }
        let image_ok = unsafe {
            UpdateResourceW(
                handle,
                3usize as *const u16,
                2usize as *const u16,
                0x409,
                image.as_ptr().cast(),
                image.len() as u32,
            )
        } != 0;
        let group_ok = image_ok
            && unsafe {
                UpdateResourceW(
                    handle,
                    14usize as *const u16,
                    101usize as *const u16,
                    0x409,
                    group.as_ptr().cast(),
                    group.len() as u32,
                )
            } != 0;
        let committed = unsafe { EndUpdateResourceW(handle, if group_ok { 0 } else { 1 }) } != 0;
        if group_ok && committed {
            Ok(())
        } else {
            Err(format!(
                "update executable icon: {}",
                std::io::Error::last_os_error()
            ))
        }
    }

    fn write_diagnostic(game: &std::path::Path, state: TaskState, message: &str) {
        let state = match state {
            TaskState::Working => "WORKING",
            TaskState::Done => "DONE",
            TaskState::Skipped => "SKIPPED",
            TaskState::Failed => "FAILED",
        };
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(game.join("Doraemon-Patcher-diagnostic.log"))
        {
            let _ = writeln!(file, "[{state}] {message}");
            let _ = file.flush();
            let _ = file.sync_all();
        }
    }

    fn make_group_box(
        parent: &nwg::Window,
        text: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        font: &nwg::Font,
    ) -> Result<nwg::ControlHandle, nwg::NwgError> {
        use winapi::shared::minwindef::{LPARAM, TRUE, WPARAM};
        use winapi::um::winuser::SendMessageW;

        let handle = nwg::ControlBase::build_hwnd()
            .class_name("BUTTON")
            .forced_flags(WS_CHILD)
            .flags(WS_VISIBLE | BS_GROUPBOX)
            .text(text)
            .size((w, h))
            .position((x, y))
            .parent(Some(parent.handle.into()))
            .build()?;

        if let Some(hwnd) = handle.hwnd() {
            unsafe {
                SendMessageW(hwnd, WM_SETFONT, font.handle as WPARAM, TRUE as LPARAM);
            }
        }
        Ok(handle)
    }

    fn prompt_run_config(game: &std::path::Path, window: &nwg::Window) {
        let config_names = ["cnc-ddraw config.exe", "ddrawcfg.exe"];
        let config_path = config_names.iter().find_map(|name| {
            let path = game.join(name);
            if path.exists() {
                Some(path)
            } else {
                None
            }
        });
        let Some(config_path) = config_path else {
            return;
        };
        let params = nwg::MessageParams {
            title: "Graphics Wrapper",
            content: "The graphics wrapper has been installed.\n\nWould you like to open the configuration tool now?\n(Recommended for first-time use on Crossover or Wine.)",
            buttons: nwg::MessageButtons::YesNo,
            icons: nwg::MessageIcons::Question,
        };
        if nwg::modal_message(window, &params) == nwg::MessageChoice::Yes {
            let _ = std::process::Command::new(&config_path).spawn();
        }
    }

    impl Ui {
        fn build(game: &std::path::Path, languages: Vec<String>) -> Result<Self, nwg::NwgError> {
            let mut ui = Self::default();

            nwg::Window::builder()
                .size((640, 664))
                .position((300, 150))
                .title("Doraemon Monopoly Patcher")
                .flags(
                    nwg::WindowFlags::WINDOW
                        | nwg::WindowFlags::MINIMIZE_BOX
                        | nwg::WindowFlags::VISIBLE,
                )
                .build(&mut ui.window)?;

            // -- Fonts --

            nwg::Font::builder()
                .family("Tahoma")
                .size(16)
                .weight(700)
                .build(&mut ui.title_font)?;

            nwg::Font::builder()
                .family("Tahoma")
                .weight(700)
                .build(&mut ui.group_font)?;

            // -- Title banner --

            nwg::Label::builder()
                .text("Doraemon Monopoly Patcher")
                .position((16, 12))
                .size((608, 28))
                .parent(&ui.window)
                .build(&mut ui.title_bar)?;
            ui.title_bar.set_font(Some(&ui.title_font));

            nwg::Label::builder()
                .text("Choose a language, then pick the compatibility extras you want.")
                .position((16, 42))
                .size((608, 18))
                .parent(&ui.window)
                .build(&mut ui.subtitle)?;

            // -- Options group box --

            ui.options_group = make_group_box(
                &ui.window,
                " Patch options ",
                12,
                66,
                616,
                214,
                &ui.group_font,
            )?;

            nwg::Label::builder()
                .text(&format!("Game folder: {}", game.display()))
                .position((24, 88))
                .size((592, 18))
                .parent(&ui.window)
                .build(&mut ui.game_label)?;

            nwg::ComboBox::builder()
                .collection(languages)
                .selected_index(Some(0))
                .position((24, 112))
                .size((180, 26))
                .parent(&ui.window)
                .build(&mut ui.language)?;

            nwg::CheckBox::builder()
                .text("Play without the original disc")
                .check_state(nwg::CheckBoxState::Checked)
                .position((24, 145))
                .size((420, 20))
                .parent(&ui.window)
                .build(&mut ui.no_disc)?;

            nwg::CheckBox::builder()
                .text("Skip the original Setup registry check")
                .check_state(nwg::CheckBoxState::Checked)
                .position((24, 169))
                .size((420, 20))
                .parent(&ui.window)
                .build(&mut ui.no_reg)?;

            nwg::CheckBox::builder()
                .text("Use local background music when available")
                .check_state(
                    if music::valid(&game.join("Music.dat"))
                        || find_cue(game).is_some()
                        || cue::valid_wav(&game.join("DoraemonMusic.wav"))
                    {
                        nwg::CheckBoxState::Checked
                    } else {
                        nwg::CheckBoxState::Unchecked
                    },
                )
                .position((24, 193))
                .size((420, 20))
                .parent(&ui.window)
                .build(&mut ui.local_audio)?;

            nwg::CheckBox::builder()
                .text("Modern SFX volume control (Windows 7+ / CrossOver)")
                .check_state(nwg::CheckBoxState::Unchecked)
                .position((24, 217))
                .size((440, 20))
                .parent(&ui.window)
                .build(&mut ui.modern_volume)?;

            nwg::Label::builder()
                .text(&music_text(game))
                .position((24, 241))
                .size((420, 28))
                .parent(&ui.window)
                .build(&mut ui.music)?;

            nwg::Button::builder()
                .text("Refresh")
                .position((520, 246))
                .size((85, 24))
                .parent(&ui.window)
                .build(&mut ui.refresh_music)?;

            // -- Actions group box --

            ui.actions_group =
                make_group_box(&ui.window, " Actions ", 12, 290, 616, 56, &ui.group_font)?;

            nwg::Button::builder()
                .text("Apply patch")
                .position((24, 308))
                .size((125, 30))
                .parent(&ui.window)
                .build(&mut ui.apply)?;

            nwg::Button::builder()
                .text("Restore backup")
                .enabled(game.join("backup").is_dir())
                .position((160, 308))
                .size((130, 30))
                .parent(&ui.window)
                .build(&mut ui.restore)?;

            nwg::Button::builder()
                .text("Add graphics wrapper")
                .enabled(true)
                .position((301, 308))
                .size((165, 30))
                .parent(&ui.window)
                .build(&mut ui.wrapper)?;

            nwg::Button::builder()
                .text("Play")
                .enabled(game.join("Doraemon.exe").is_file())
                .position((477, 308))
                .size((128, 30))
                .parent(&ui.window)
                .build(&mut ui.play)?;

            // -- Progress bar --

            nwg::ProgressBar::builder()
                .range(0..100)
                .pos(0)
                .step(1)
                .size((616, 22))
                .position((12, 358))
                .parent(&ui.window)
                .build(&mut ui.progress)?;

            // -- Log group box --

            ui.log_group = make_group_box(&ui.window, " Log ", 12, 392, 616, 240, &ui.group_font)?;

            nwg::RichTextBox::builder()
                .text("")
                .readonly(true)
                .position((24, 414))
                .size((592, 208))
                .parent(&ui.window)
                .build(&mut ui.log)?;
            ui.log.set_background_color([250, 250, 248]);

            // -- Exit button --

            nwg::Button::builder()
                .text("Exit")
                .position((548, 634))
                .size((80, 24))
                .parent(&ui.window)
                .build(&mut ui.exit)?;

            nwg::AnimationTimer::builder()
                .parent(&ui.window)
                .interval(Duration::from_millis(120))
                .active(true)
                .build(&mut ui.timer)?;

            Ok(ui)
        }
    }

    pub fn run() -> Result<(), String> {
        nwg::init().map_err(|error| error.to_string())?;
        nwg::Font::set_global_family("Tahoma").map_err(|error| error.to_string())?;

        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let restore_mode = executable
            .file_name()
            .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Restore.exe"));
        let game = if restore_mode {
            executable
                .parent()
                .and_then(std::path::Path::parent)
                .ok_or("Restore.exe must be inside the backup folder")?
                .to_path_buf()
        } else {
            executable
                .parent()
                .ok_or("the patcher executable has no parent game folder")?
                .to_path_buf()
        };
        let english =
            if super::ENGLISH_PAYLOAD.is_empty() {
                None
            } else {
                Some(payload::decode(super::ENGLISH_PAYLOAD).map_err(|error| {
                    format!("This patcher has an invalid English payload: {error}")
                })?)
            };
        let vietnamese = if super::VIETNAMESE_PAYLOAD.is_empty() {
            None
        } else {
            Some(payload::decode(super::VIETNAMESE_PAYLOAD).map_err(|error| {
                format!("This patcher has an invalid Vietnamese payload: {error}")
            })?)
        };
        let mut languages = vec!["Unchanged".into()];
        let english_index = english.as_ref().map(|_| {
            let index = languages.len();
            languages.push("English".into());
            index
        });
        let vietnamese_index = vietnamese.as_ref().map(|_| {
            let index = languages.len();
            languages.push("Vietnamese".into());
            index
        });
        let wrapper_available = english
            .as_ref()
            .or(vietnamese.as_ref())
            .is_some_and(|payload| {
                payload
                    .bundled
                    .iter()
                    .any(|file| !file.name.eq_ignore_ascii_case("doraudio.dll"))
            });
        let ui = Rc::new(Ui::build(&game, languages).map_err(|error| error.to_string())?);
        append_log(
            &ui,
            TaskState::Skipped,
            "Ready. Applying a new choice restores the original files first.",
        );
        if restore_mode {
            ui.window.set_text("Restore Doraemon Monopoly");
            ui.title_bar.set_text("Restore Doraemon Monopoly");
            ui.subtitle
                .set_text("Restore the exact original files kept in this backup.");
            ui.apply.set_enabled(false);
            ui.wrapper.set_enabled(false);
            ui.play.set_enabled(game.join("Doraemon.exe").is_file());
            ui.no_disc.set_enabled(false);
            append_log(
                &ui,
                TaskState::Working,
                "Ready to restore the original game files.",
            );
        }
        let english = Rc::new(english);
        let vietnamese = Rc::new(vietnamese);
        let game = Rc::new(game);
        let busy = Rc::new(Cell::new(false));
        let events_ui = ui.clone();
        let events_game = game.clone();
        let events_payload = english.clone();
        let events_vietnamese = vietnamese.clone();
        let events_english_index = english_index;
        let events_vietnamese_index = vietnamese_index;
        let events_wrapper_available = wrapper_available;
        let events_busy = busy.clone();
        let (events_tx, events_rx) = mpsc::channel::<UiEvent>();
        let handler = nwg::full_bind_event_handler(
            &ui.window.handle,
            move |event, _data, handle| {
                let ui = &events_ui;
                if (event == nwg::Event::OnWindowClose)
                    || (event == nwg::Event::OnButtonClick && handle == ui.exit.handle)
                {
                    if events_busy.get() {
                        append_log(
                            &ui,
                            TaskState::Working,
                            "Please wait for the current task to finish.",
                        );
                    } else {
                        nwg::stop_thread_dispatch();
                    }
                } else if event == nwg::Event::OnTimerTick && handle == ui.timer.handle {
                    while let Ok(event) = events_rx.try_recv() {
                        match event {
                            UiEvent::Progress(update) => {
                                if let Some(pct) = update.progress {
                                    ui.progress.set_pos(pct as u32);
                                }
                                append_log(&ui, update.state, &update.message)
                            }
                            UiEvent::Finished(Ok(report)) => {
                                ui.progress.set_pos(100);
                                append_log(
                                    &ui,
                                    TaskState::Done,
                                    if report.changed.is_empty() {
                                        "Apply finished: everything requested was already in place."
                                    } else {
                                        "Apply finished successfully."
                                    },
                                );
                                append_log(&ui, TaskState::Done, &report.audio);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.music.set_text(&music_text(&events_game));
                                events_busy.set(false);
                                ui.apply.set_enabled(!restore_mode);
                                ui.wrapper
                                    .set_enabled(events_wrapper_available && !restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                                ui.refresh_music.set_enabled(!restore_mode);
                            }
                            UiEvent::Finished(Err(error)) => {
                                ui.progress.set_pos(0);
                                append_log(
                                    &ui,
                                    TaskState::Failed,
                                    &format!("Apply failed: {error}"),
                                );
                                events_busy.set(false);
                                ui.apply.set_enabled(!restore_mode);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.wrapper
                                    .set_enabled(events_wrapper_available && !restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                                ui.refresh_music.set_enabled(!restore_mode);
                            }
                            UiEvent::Restored(Ok(files)) => {
                                ui.progress.set_pos(100);
                                append_log(
                                    &ui,
                                    TaskState::Done,
                                    &format!("Restored and verified: {}.", files.join(", ")),
                                );
                                events_busy.set(false);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.apply.set_enabled(!restore_mode);
                                ui.wrapper
                                    .set_enabled(events_wrapper_available && !restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                                ui.music.set_text(&music_text(&events_game));
                                ui.refresh_music.set_enabled(!restore_mode);
                            }
                            UiEvent::Restored(Err(error)) => {
                                ui.progress.set_pos(0);
                                append_log(
                                    &ui,
                                    TaskState::Failed,
                                    &format!("Restore failed: {error}"),
                                );
                                events_busy.set(false);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.apply.set_enabled(!restore_mode);
                                ui.wrapper
                                    .set_enabled(events_wrapper_available && !restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                            }
                            UiEvent::Wrapper(Ok(files)) if files.is_empty() => {
                                ui.progress.set_pos(100);
                                append_log(
                                    &ui,
                                    TaskState::Skipped,
                                    "The graphics wrapper is already installed.",
                                );
                                events_busy.set(false);
                                ui.apply.set_enabled(!restore_mode);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.wrapper.set_enabled(!restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                            }
                            UiEvent::Wrapper(Ok(files)) => {
                                ui.progress.set_pos(100);
                                append_log(
                                    &ui,
                                    TaskState::Done,
                                    &format!("Graphics wrapper added: {} files.", files.len()),
                                );
                                events_busy.set(false);
                                ui.apply.set_enabled(!restore_mode);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.wrapper.set_enabled(!restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                                prompt_run_config(&events_game, &events_ui.window);
                            }
                            UiEvent::Wrapper(Err(error)) => {
                                ui.progress.set_pos(0);
                                append_log(
                                    &ui,
                                    TaskState::Failed,
                                    &format!("Graphics wrapper failed: {error}"),
                                );
                                events_busy.set(false);
                                ui.apply.set_enabled(!restore_mode);
                                ui.restore.set_enabled(events_game.join("backup").is_dir());
                                ui.wrapper.set_enabled(!restore_mode);
                                ui.play
                                    .set_enabled(events_game.join("Doraemon.exe").is_file());
                            }
                        }
                    }
                } else if event == nwg::Event::OnButtonClick && handle == ui.refresh_music.handle {
                    ui.music.set_text(&music_text(&events_game));
                } else if event == nwg::Event::OnButtonClick && handle == ui.apply.handle {
                    events_busy.set(true);
                    ui.apply.set_enabled(false);
                    ui.restore.set_enabled(false);
                    ui.wrapper.set_enabled(false);
                    ui.play.set_enabled(false);
                    ui.refresh_music.set_enabled(false);
                    append_log(&ui, TaskState::Working, "Starting Apply…");
                    let _ =
                        std::fs::remove_file(events_game.join("Doraemon-Patcher-diagnostic.log"));
                    write_diagnostic(&events_game, TaskState::Working, "Apply button pressed.");
                    let options = ApplyOptions {
                        no_disc: ui.no_disc.check_state() == nwg::CheckBoxState::Checked,
                        no_reg: ui.no_reg.check_state() == nwg::CheckBoxState::Checked,
                        local_audio: ui.local_audio.check_state() == nwg::CheckBoxState::Checked,
                        modern_volume: ui.modern_volume.check_state()
                            == nwg::CheckBoxState::Checked,
                        cue: find_cue(&events_game),
                    };
                    let game = (*events_game).clone();
                    let selection = ui.language.selection();
                    let payload = if selection == events_english_index {
                        events_payload
                            .as_ref()
                            .as_ref()
                            .expect("English is available")
                            .clone()
                    } else if selection == events_vietnamese_index {
                        events_vietnamese
                            .as_ref()
                            .as_ref()
                            .expect("Vietnamese is available")
                            .clone()
                    } else {
                        let mut payload = events_payload
                            .as_ref()
                            .as_ref()
                            .or(events_vietnamese.as_ref().as_ref())
                            .cloned()
                            .unwrap_or(Payload {
                                language: payload::Language::Custom,
                                profiles: Vec::new(),
                                strings: None,
                                bundled: Vec::new(),
                            });
                        payload.language = payload::Language::Custom;
                        payload.profiles.clear();
                        payload.strings = None;
                        payload
                    };
                    let icon = if selection == events_english_index {
                        Some(super::ENGLISH_ICON)
                    } else if selection == events_vietnamese_index {
                        Some(super::VIETNAMESE_ICON)
                    } else {
                        None
                    };
                    let executable = executable.clone();
                    let tx = events_tx.clone();
                    thread::spawn(move || {
                        let result = panic::catch_unwind(AssertUnwindSafe(|| {
                            let wants_changes = payload.language != payload::Language::Custom
                                || options.no_disc
                                || options.no_reg
                                || options.local_audio
                                || options.modern_volume;
                            let backup = game.join("backup");
                            if backup.is_dir() {
                                let message = if wants_changes {
                                    "Restoring the original files before applying your new choices…"
                                } else {
                                    "No patch choices selected; restoring the original files…"
                                };
                                let update = TaskProgress { state: TaskState::Working, message: message.into(), progress: Some(3) };
                                write_diagnostic(&game, update.state, &update.message);
                                let _ = tx.send(UiEvent::Progress(update));
                                let restored = install::restore(&backup)?;
                                let update = TaskProgress { state: TaskState::Done, message: format!("Original files restored: {}.", restored.join(", ")), progress: Some(10) };
                                write_diagnostic(&game, update.state, &update.message);
                                let _ = tx.send(UiEvent::Progress(update));
                                if !wants_changes {
                                    return Ok(install::ApplyReport { changed: Vec::new(), audio: "Nothing selected, so the game is back to its original files.".into() });
                                }
                            } else if !wants_changes {
                                return Ok(install::ApplyReport { changed: Vec::new(), audio: "Nothing is selected and no backup was found. The game is unchanged.".into() });
                            }
                            let mut report = install::apply_with_progress(
                                &game,
                                &payload,
                                &options,
                                &executable,
                                &mut |update| {
                                    write_diagnostic(&game, update.state, &update.message);
                                    let _ = tx.send(UiEvent::Progress(update));
                                },
                            )?;
                            if let Some(icon) = icon {
                                let update = TaskProgress { state: TaskState::Working, message: "Applying the selected game icon…".into(), progress: Some(98) };
                                let _ = tx.send(UiEvent::Progress(update));
                                apply_game_icon(&game, icon)?;
                                report.changed.push("Doraemon.exe icon".into());
                            }
                            Ok(report)
                        }))
                        .unwrap_or_else(|_| {
                            Err(
                                "The patch task stopped unexpectedly; no files were installed."
                                    .into(),
                            )
                        });
                        match &result {
                            Ok(_) => write_diagnostic(
                                &game,
                                TaskState::Done,
                                "Apply finished successfully.",
                            ),
                            Err(error) => write_diagnostic(
                                &game,
                                TaskState::Failed,
                                &format!("Apply failed: {error}"),
                            ),
                        }
                        let _ = tx.send(UiEvent::Finished(result));
                    });
                } else if event == nwg::Event::OnButtonClick && handle == ui.restore.handle {
                    events_busy.set(true);
                    ui.apply.set_enabled(false);
                    ui.restore.set_enabled(false);
                    ui.wrapper.set_enabled(false);
                    ui.play.set_enabled(false);
                    append_log(&ui, TaskState::Working, "Restoring original files…");
                    let backup = events_game.join("backup");
                    let tx = events_tx.clone();
                    thread::spawn(move || {
                        let result =
                            panic::catch_unwind(AssertUnwindSafe(|| install::restore(&backup)))
                                .unwrap_or_else(|_| {
                                    Err(
                                "The restore task stopped unexpectedly; no files were restored."
                                    .into(),
                            )
                                });
                        let _ = tx.send(UiEvent::Restored(result));
                    });
                } else if event == nwg::Event::OnButtonClick && handle == ui.wrapper.handle {
                    events_busy.set(true);
                    ui.apply.set_enabled(false);
                    ui.restore.set_enabled(false);
                    ui.wrapper.set_enabled(false);
                    ui.play.set_enabled(false);
                    append_log(&ui, TaskState::Working, "Adding the graphics wrapper…");
                    let game = (*events_game).clone();
                    let payload = events_payload
                        .as_ref()
                        .as_ref()
                        .or(events_vietnamese.as_ref().as_ref())
                        .cloned()
                        .unwrap_or(Payload {
                            language: payload::Language::Custom,
                            profiles: Vec::new(),
                            strings: None,
                            bundled: Vec::new(),
                        });
                    let tx = events_tx.clone();
                    thread::spawn(move || {
                        let result = panic::catch_unwind(AssertUnwindSafe(|| install::add_wrapper(&game, &payload)))
                            .unwrap_or_else(|_| Err("The graphics-wrapper task stopped unexpectedly; no files were added.".into()));
                        let _ = tx.send(UiEvent::Wrapper(result));
                    });
                } else if event == nwg::Event::OnButtonClick && handle == ui.play.handle {
                    let game_exe = events_game.join("Doraemon.exe");
                    match std::process::Command::new(&game_exe)
                        .current_dir(&*events_game)
                        .spawn()
                    {
                        Ok(_) => append_log(&ui, TaskState::Done, "Launched Doraemon.exe."),
                        Err(error) => append_log(
                            &ui,
                            TaskState::Failed,
                            &format!("Could not launch Doraemon.exe: {error}"),
                        ),
                    }
                }
            },
        );
        nwg::dispatch_thread_events();
        nwg::unbind_event_handler(&handler);
        Ok(())
    }
}

#[cfg(windows)]
fn main() {
    if let Err(error) = windows_app::run() {
        native_windows_gui::error_message("Doraemon patcher", &error);
    }
}
