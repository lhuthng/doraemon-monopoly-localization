#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
const EMBEDDED_PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bin"));

#[cfg(not(windows))]
fn main() {
    eprintln!("Doraemon patcher GUI is Windows-only. Use patch-build on this platform.");
}

#[cfg(windows)]
mod windows_app {
    extern crate native_windows_gui as nwg;

    use doraemon_game_patch::{
        install::{self, ApplyOptions},
        payload::{self, Payload},
    };
    use std::{cell::RefCell, path::PathBuf, rc::Rc};

    #[derive(Default)]
    struct Ui {
        window: nwg::Window,
        heading: nwg::Label,
        coverage: nwg::Label,
        game_label: nwg::Label,
        game_path: nwg::TextInput,
        game_browse: nwg::Button,
        no_disc: nwg::CheckBox,
        cue_label: nwg::Label,
        cue_path: nwg::TextInput,
        cue_browse: nwg::Button,
        apply: nwg::Button,
        restore: nwg::Button,
        status: nwg::Label,
        folder_dialog: nwg::FileDialog,
        cue_dialog: nwg::FileDialog,
    }

    impl Ui {
        fn build(payload: &Payload) -> Result<Self, nwg::NwgError> {
            let mut ui = Self::default();
            nwg::Window::builder()
                .size((650, 420))
                .position((300, 200))
                .title("Doraemon Monopoly Localization Patcher")
                .build(&mut ui.window)?;
            nwg::Label::builder()
                .text(&format!("{} localization", payload.language.label()))
                .position((24, 20))
                .size((590, 34))
                .parent(&ui.window)
                .build(&mut ui.heading)?;
            let coverage = match payload.language {
                payload::Language::English => {
                    "Full dialogue translation · user interface approximately 90% localized."
                }
                payload::Language::Vietnamese => {
                    "Full Vietnamese dialogue translation · currently uses the English UI graphics."
                }
            };
            nwg::Label::builder()
                .text(coverage)
                .position((24, 58))
                .size((590, 34))
                .parent(&ui.window)
                .build(&mut ui.coverage)?;
            nwg::Label::builder()
                .text("Game folder containing Doraemon.exe")
                .position((24, 105))
                .size((400, 22))
                .parent(&ui.window)
                .build(&mut ui.game_label)?;
            nwg::TextInput::builder()
                .position((24, 130))
                .size((500, 28))
                .parent(&ui.window)
                .build(&mut ui.game_path)?;
            nwg::Button::builder()
                .text("Browse…")
                .position((535, 130))
                .size((85, 28))
                .parent(&ui.window)
                .build(&mut ui.game_browse)?;
            nwg::CheckBox::builder()
                .text("No-disc mode (also bypass the Setup registry check)")
                .check_state(nwg::CheckBoxState::Checked)
                .position((24, 180))
                .size((500, 26))
                .parent(&ui.window)
                .build(&mut ui.no_disc)?;
            nwg::Label::builder().text("Optional original DORAEMON.cue (music is silent when no WAV or CUE is available)").position((24,218)).size((600,22)).parent(&ui.window).build(&mut ui.cue_label)?;
            nwg::TextInput::builder()
                .position((24, 243))
                .size((500, 28))
                .parent(&ui.window)
                .build(&mut ui.cue_path)?;
            nwg::Button::builder()
                .text("Choose CUE…")
                .position((535, 243))
                .size((85, 28))
                .parent(&ui.window)
                .build(&mut ui.cue_browse)?;
            nwg::Button::builder()
                .text("Validate and patch")
                .position((24, 300))
                .size((155, 34))
                .parent(&ui.window)
                .build(&mut ui.apply)?;
            nwg::Button::builder()
                .text("Restore backup")
                .position((190, 300))
                .size((135, 34))
                .parent(&ui.window)
                .build(&mut ui.restore)?;
            nwg::Label::builder()
                .text("Select the game folder. No files are changed until validation succeeds.")
                .position((24, 350))
                .size((596, 48))
                .parent(&ui.window)
                .build(&mut ui.status)?;
            nwg::FileDialog::builder()
                .action(nwg::FileDialogAction::OpenDirectory)
                .title("Select Doraemon game folder")
                .build(&mut ui.folder_dialog)?;
            nwg::FileDialog::builder()
                .action(nwg::FileDialogAction::Open)
                .title("Select DORAEMON.cue")
                .filters("CUE sheet (*.cue)|*.cue")
                .build(&mut ui.cue_dialog)?;
            Ok(ui)
        }
    }

    pub fn run() -> Result<(), String> {
        nwg::init().map_err(|error| error.to_string())?;
        if let Ok(executable) = std::env::current_exe() {
            if executable
                .file_name()
                .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Restore.exe"))
            {
                let backup = executable
                    .parent()
                    .ok_or("Restore.exe has no backup folder")?;
                let restored = install::restore(backup)?;
                nwg::simple_message(
                    "Restore complete",
                    &format!("Restored and verified: {}", restored.join(", ")),
                );
                return Ok(());
            }
        }
        let payload = payload::decode(super::EMBEDDED_PAYLOAD).map_err(|error| {
            format!("This development build has no valid embedded payload: {error}")
        })?;
        nwg::Font::set_global_family("Segoe UI").map_err(|error| error.to_string())?;
        let ui = Rc::new(RefCell::new(
            Ui::build(&payload).map_err(|error| error.to_string())?,
        ));
        let payload = Rc::new(payload);
        let events_ui = ui.clone();
        let handler = nwg::full_bind_event_handler(
            &ui.borrow().window.handle,
            move |event, _data, handle| {
                let ui = events_ui.borrow_mut();
                if event == nwg::Event::OnWindowClose {
                    nwg::stop_thread_dispatch();
                } else if event == nwg::Event::OnButtonClick && handle == ui.game_browse.handle {
                    if ui.folder_dialog.run(Some(&ui.window)) {
                        if let Ok(path) = ui.folder_dialog.get_selected_item() {
                            ui.game_path.set_text(&path.to_string_lossy());
                        }
                    }
                } else if event == nwg::Event::OnButtonClick && handle == ui.cue_browse.handle {
                    if ui.cue_dialog.run(Some(&ui.window)) {
                        if let Ok(path) = ui.cue_dialog.get_selected_item() {
                            ui.cue_path.set_text(&path.to_string_lossy());
                        }
                    }
                } else if event == nwg::Event::OnButtonClick && handle == ui.apply.handle {
                    let game = PathBuf::from(ui.game_path.text());
                    let cue = ui.cue_path.text();
                    ui.status
                        .set_text("Validating all original files and preparing verified outputs…");
                    let options = ApplyOptions {
                        no_disc: ui.no_disc.check_state() == nwg::CheckBoxState::Checked,
                        cue: if cue.trim().is_empty() {
                            None
                        } else {
                            Some(PathBuf::from(cue))
                        },
                    };
                    match std::env::current_exe()
                        .map_err(|error| error.to_string())
                        .and_then(|exe| install::apply(&game, &payload, &options, &exe))
                    {
                        Ok(report) => {
                            let message = format!(
                                "Patched and verified: {}\n{}",
                                report.changed.join(", "),
                                report.audio
                            );
                            ui.status.set_text(&message);
                            nwg::simple_message("Patch complete", &message);
                        }
                        Err(error) => {
                            ui.status.set_text(&format!("Error: {error}"));
                            nwg::error_message("Patch failed", &error);
                        }
                    }
                } else if event == nwg::Event::OnButtonClick && handle == ui.restore.handle {
                    let game = PathBuf::from(ui.game_path.text());
                    match install::restore(&game.join("backup")) {
                        Ok(files) => {
                            let message = format!("Restored and verified: {}", files.join(", "));
                            ui.status.set_text(&message);
                            nwg::simple_message("Restore complete", &message);
                        }
                        Err(error) => {
                            ui.status.set_text(&format!("Error: {error}"));
                            nwg::error_message("Restore failed", &error);
                        }
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
