//! Action engine (SPEC 6). Every action runs on a worker thread and reports errors instead
//! of panicking; the caller logs them and flashes a red "!" on the button.

pub mod shell;
pub mod system;

use std::time::Duration;

use anyhow::bail;

use crate::app::core;
use crate::foreground::{self, ForegroundInfo};
use crate::input::send;
use crate::keys::KeyCombo;
use crate::profiles::model::{Action, Button, MediaOp, VolumeOp};

/// Where and why an action runs.
#[derive(Debug, Clone, Default)]
pub struct ActionCtx {
    /// Window that was in the foreground when the panel opened.
    pub target: Option<ForegroundInfo>,
    pub button_id: Option<String>,
    /// Started from the panel (focus is restored and modifiers released first).
    pub from_panel: bool,
}

/// Synthetic input into an elevated window is blocked by Windows (UIPI).
#[derive(Debug, thiserror::Error)]
#[error("the target window runs as administrator; Windows blocks input into it (UIPI)")]
pub struct UipiBlocked;

/// Returns the hint key to show once when an action failed because of UIPI.
pub fn uipi_hint_for(e: &anyhow::Error) -> Option<String> {
    e.downcast_ref::<UipiBlocked>()?;
    let core = core();
    let mut cfg = core.config.read().clone();
    if cfg.uipi_hint_shown {
        return None;
    }
    cfg.uipi_hint_shown = true;
    let _ = core.update_config(cfg);
    Some("uipiHint".into())
}

/// Runs the action of a button, handling per-button state (toggles).
pub fn run_button_action(button: &Button, ctx: &ActionCtx) -> anyhow::Result<()> {
    if ctx.from_panel {
        // SPEC 4.5: the panel is already hidden; give focus back and wait ~30 ms.
        if let Some(t) = &ctx.target {
            if !t.is_own_process() {
                foreground::restore_focus(t.hwnd());
            }
        }
        std::thread::sleep(Duration::from_millis(30));
    }
    match &button.action {
        Action::Toggle { a, b } => {
            let state = core().toggles.lock().get(&button.id).copied().unwrap_or(0);
            let next = if state == 0 { a } else { b };
            execute(next, ctx)?;
            core()
                .toggles
                .lock()
                .insert(button.id.clone(), 1 - state.min(1));
            Ok(())
        }
        Action::Plugin { .. } => crate::sdk_host::key_press(button),
        other => execute(other, ctx),
    }
}

/// Makes sure keystrokes go where they should: refuses elevated targets and releases
/// modifiers the user may still be holding from the trigger.
fn prepare_input(ctx: &ActionCtx) -> anyhow::Result<()> {
    let fg = foreground::current();
    if let Some(fg) = fg.as_ref().filter(|f| !f.is_own_process()) {
        if foreground::input_blocked_by_uipi(fg.pid) {
            return Err(UipiBlocked.into());
        }
    }
    if ctx.from_panel {
        send::release_held_modifiers(Duration::from_millis(400));
    }
    Ok(())
}

pub fn execute(action: &Action, ctx: &ActionCtx) -> anyhow::Result<()> {
    match action {
        Action::Hotkey { keys } => {
            prepare_input(ctx)?;
            send::send_combos(keys)
        }
        Action::TypeText { text } => {
            prepare_input(ctx)?;
            send::type_text(text)
        }
        Action::Launch {
            path,
            args,
            working_dir,
            admin,
        } => shell::launch(path, args, working_dir, *admin),
        Action::Open { target } => shell::open(target),
        Action::Command {
            command,
            shell: sh,
            hidden,
        } => shell::command(command, *sh, *hidden),
        Action::Media { op } => {
            let vk = match op {
                MediaOp::PlayPause => 0xB3,
                MediaOp::Next => 0xB0,
                MediaOp::Previous => 0xB1,
                MediaOp::Stop => 0xB2,
            };
            send::tap_vk(vk)
        }
        Action::NowPlaying {} => crate::media::play_pause(),
        Action::Volume { op, step, value } => {
            let step = (*step).clamp(1, 100) as f32 / 100.0;
            match op {
                VolumeOp::Up => crate::audio::change_volume(step).map(|_| ()),
                VolumeOp::Down => crate::audio::change_volume(-step).map(|_| ()),
                VolumeOp::Set => crate::audio::set_volume((*value).min(100) as f32 / 100.0),
                VolumeOp::MuteToggle => crate::audio::toggle_output_mute().map(|_| ()),
            }
        }
        Action::MicMute {} => crate::audio::toggle_mic_mute().map(|_| ()),
        Action::AppVolume { exe, op, step } => {
            let step = (*step).clamp(1, 100) as f32 / 100.0;
            match op {
                VolumeOp::Up => crate::audio::sessions::change_app_volume(exe, step),
                VolumeOp::Down => crate::audio::sessions::change_app_volume(exe, -step),
                VolumeOp::MuteToggle => crate::audio::sessions::toggle_app_mute(exe).map(|_| ()),
                VolumeOp::Set => bail!("setting an exact app volume is not supported"),
            }
        }
        Action::AudioDevice {
            flow, device_id, ..
        } => match device_id {
            Some(id) if !id.is_empty() => crate::audio::set_default_device(id),
            _ => crate::audio::cycle_device(*flow).map(|_| ()),
        },
        Action::Window { op } => {
            let target = ctx
                .target
                .clone()
                .or_else(|| core().last_foreground.lock().clone());
            let Some(t) = target else {
                bail!("no target window")
            };
            crate::windows_ops::apply(t.hwnd(), *op)
        }
        Action::System { op } => match op {
            crate::profiles::model::SystemOp::Lock => system::lock(),
            crate::profiles::model::SystemOp::Sleep => system::sleep(),
            crate::profiles::model::SystemOp::Snip => {
                prepare_input(ctx)?;
                send::send_combos(&[KeyCombo::new(false, true, false, true, "S")])
            }
        },
        Action::Obs { op, scene, source } => crate::obs::run(*op, scene, source),
        Action::Multi { actions } => {
            for (i, a) in actions.iter().enumerate() {
                if let Err(e) = execute(a, ctx) {
                    tracing::warn!("multi action cancelled at step {}: {e:#}", i + 1);
                    return Err(e.context(format!("step {} of the multi action", i + 1)));
                }
            }
            Ok(())
        }
        Action::Delay { ms } => {
            std::thread::sleep(Duration::from_millis((*ms).min(60_000) as u64));
            Ok(())
        }
        // A toggle nested in a multi action has no own state: it runs its first action.
        Action::Toggle { a, .. } => execute(a, ctx),
        Action::Folder { .. } => Ok(()),
        Action::SwitchProfile { profile_id } => {
            let core = core();
            if core.profiles.read().get(profile_id).is_none() {
                bail!("profile {profile_id} does not exist");
            }
            *core.manual_profile.lock() = Some(profile_id.clone());
            crate::profiles::active::refresh_active_profile();
            Ok(())
        }
        Action::Plugin { .. } => bail!("plugin actions run through their button"),
    }
}
