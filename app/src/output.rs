//! Spout2 (Windows) and Syphon (macOS) output of the dialogue and history windows.
//!
//! NanaUI paints each window's own scene a second time into an offscreen
//! texture. On Windows that texture is exported as a DX12 shared handle and
//! handed to a Spout sender. On macOS the same-device texture is blitted into
//! a Syphon server on the Metal queue. Nothing is read back to the CPU, and
//! output keeps flowing while the window is hidden.

#[cfg(target_os = "macos")]
mod syphon;

use std::collections::HashMap;

use nana_ui::{
    WindowOutputAlpha, WindowOutputConfig, WindowOutputExport, WindowOutputExtent,
    WindowOutputFrame, WindowOutputStatus,
};
use nanatype_i18n::Locale;
use nanatype_settings::{Settings, SpoutOutput};

use crate::ui::{I18N, LinkState};
use crate::windows::WindowKind;

/// What a Spout sender's failure is called, one key per kind of failure.
pub const SPOUT_ERROR_KEYS: [&str; 6] = [
    "output-spout-unsupported",
    "spout-error-backend",
    "spout-error-interop",
    "spout-error-format",
    "spout-error-publish",
    "spout-error-name",
];

/// Syphon failures. Kept beside the Spout keys so every locale is checked.
pub const SYPHON_ERROR_KEYS: [&str; 2] = ["output-syphon-missing", "output-syphon-publish"];

#[cfg(windows)]
fn spout_error_key(e: &nanalive_spout::SpoutOutputError) -> &'static str {
    use nanalive_spout::SpoutOutputError as E;
    match e {
        E::UnsupportedPlatform => SPOUT_ERROR_KEYS[0],
        E::BackendUnavailable => SPOUT_ERROR_KEYS[1],
        E::DeviceInteropUnavailable => SPOUT_ERROR_KEYS[2],
        E::SurfaceFormatUnsupported
        | E::InvalidFrameDimensions { .. }
        | E::BufferTooSmall { .. } => SPOUT_ERROR_KEYS[3],
        E::PublishFailed => SPOUT_ERROR_KEYS[4],
        E::InvalidSenderName => SPOUT_ERROR_KEYS[5],
    }
}

pub fn settings_for(kind: WindowKind, s: &Settings) -> Option<&SpoutOutput> {
    match kind {
        WindowKind::Display => Some(&s.output.display),
        WindowKind::PeerDisplay => Some(&s.output.peer_display),
        WindowKind::History => Some(&s.output.history),
        _ => None,
    }
}

pub const KINDS: [WindowKind; 3] = [
    WindowKind::Display,
    WindowKind::PeerDisplay,
    WindowKind::History,
];

pub fn name_conflicts(kind: WindowKind, s: &Settings) -> bool {
    let Some(output) = settings_for(kind, s) else {
        return false;
    };
    KINDS
        .into_iter()
        .filter(|other| *other != kind)
        .filter_map(|other| settings_for(other, s))
        .any(|other| {
            output.enabled
                && other.enabled
                && !output.name.trim().is_empty()
                && output.name.trim() == other.name.trim()
        })
}

pub fn config(kind: WindowKind, s: &Settings) -> Option<WindowOutputConfig> {
    if !(cfg!(windows) || cfg!(target_os = "macos")) {
        return None;
    }
    if name_conflicts(kind, s) {
        return None;
    }
    let out = settings_for(kind, s).filter(|o| o.enabled && !o.name.trim().is_empty())?;
    let extent = match out.size {
        Some([width, height]) => WindowOutputExtent::Fixed { width, height },
        None => WindowOutputExtent::MatchWindow,
    };
    // Windows shares a DX12 texture. macOS blits the same-device texture
    // into Syphon; NanaUI has no Metal export token.
    let export = if cfg!(windows) {
        WindowOutputExport::Native
    } else {
        WindowOutputExport::SameDevice
    };
    Some(
        WindowOutputConfig::default()
            .with_extent(extent)
            .with_alpha(if out.straight_alpha {
                WindowOutputAlpha::Straight
            } else {
                WindowOutputAlpha::Premultiplied
            })
            .with_export(export)
            .while_hidden(true),
    )
}

#[cfg(windows)]
struct Sender {
    name: String,
    luid: i64,
    sender: nanalive_spout::GpuDx11SharedSender,
}

#[derive(Default)]
pub struct Spout {
    #[cfg(windows)]
    senders: HashMap<WindowKind, Sender>,
    #[cfg(target_os = "macos")]
    syphon: syphon::Outputs,
    #[cfg(not(any(windows, target_os = "macos")))]
    _unused: HashMap<WindowKind, ()>,
    /// Last link state per window, for the settings page.
    pub status: HashMap<WindowKind, LinkState>,
}

fn t(locale: Locale, key: &str) -> String {
    I18N.t_in(locale, key)
}

impl Spout {
    /// Drop senders whose output was turned off or renamed.
    pub fn sync(&mut self, s: &Settings, locale: Locale, peer_active: bool) {
        for kind in KINDS {
            let conflict = name_conflicts(kind, s);
            let wanted = settings_for(kind, s)
                .filter(|o| o.enabled && (kind != WindowKind::PeerDisplay || peer_active));
            #[cfg(windows)]
            if self
                .senders
                .get(&kind)
                .is_some_and(|x| wanted.is_none_or(|o| o.name != x.name) || conflict)
            {
                self.senders.remove(&kind);
            }
            let state = match wanted {
                None => LinkState::Off,
                Some(_) if !(cfg!(windows) || cfg!(target_os = "macos")) => {
                    LinkState::Error(t(locale, "output-spout-unsupported"))
                }
                Some(_) if conflict => LinkState::Error(t(locale, "output-name-conflict")),
                Some(_) => match self.status.get(&kind) {
                    // Just switched on: no frame has gone out yet.
                    None | Some(LinkState::Off) => LinkState::Connecting,
                    Some(state) => state.clone(),
                },
            };
            self.status.insert(kind, state);
        }
        #[cfg(target_os = "macos")]
        self.syphon.sync(
            &KINDS
                .into_iter()
                .filter_map(|kind| {
                    let out = settings_for(kind, s)?;
                    (out.enabled
                        && !out.name.trim().is_empty()
                        && !name_conflicts(kind, s)
                        && (kind != WindowKind::PeerDisplay || peer_active))
                        .then(|| (kind, out.name.clone()))
                })
                .collect::<Vec<_>>(),
        );
    }

    pub fn status_changed(
        &mut self,
        kind: WindowKind,
        status: &WindowOutputStatus,
        locale: Locale,
    ) {
        // A retired pool needs nothing here: the sender reopens its handles
        // on the next frame.
        if let Some(state) = crate::ui::link_text::spout(locale, status) {
            // NanaUI reports Active for the texture itself. A failed Syphon
            // publish has to stay on the badge.
            #[cfg(target_os = "macos")]
            let state = if let LinkState::Connected(_) = &state
                && let Some(key) = self.syphon.failure(kind)
            {
                crate::ui::link_text::spout_send(locale, key)
            } else {
                state
            };
            self.status.insert(kind, state);
        }
    }

    /// Hand a frame to the window's Spout sender. When sending failed,
    /// returns what to say ([`SPOUT_ERROR_KEYS`]) and the sender's own
    /// (English) words for the log.
    #[allow(unused_variables)]
    pub fn frame(
        &mut self,
        kind: WindowKind,
        name: &str,
        frame: &WindowOutputFrame,
        gpu: &nana_ui::GpuContext,
    ) -> Option<(&'static str, String)> {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            let mut token = frame.take_native()?;
            let luid = token.adapter_luid();
            let stale = self
                .senders
                .get(&kind)
                .is_none_or(|s| s.name != name || s.luid != luid);
            if stale {
                self.senders.remove(&kind);
                match nanalive_spout::GpuDx11SharedSender::new(name, luid) {
                    Ok(sender) => {
                        self.senders.insert(
                            kind,
                            Sender {
                                name: name.to_owned(),
                                luid,
                                sender,
                            },
                        );
                    }
                    Err(e) => return Some((spout_error_key(&e), e.to_string())),
                }
            }
            let sender = self.senders.get_mut(&kind)?;
            let textures: Vec<*mut std::ffi::c_void> = (0..nana_ui::NATIVE_EXPORT_SLOTS)
                .filter_map(|slot| token.texture_handle(slot).map(|h| h.as_raw_handle()))
                .collect();
            let [width, height] = token.extent();
            let shared = nanalive_spout::SharedTextureFrame {
                pool_generation: token.pool_generation(),
                adapter_luid: luid,
                width,
                height,
                textures: &textures,
                fence: token.fence_handle().as_raw_handle(),
                slot: token.slot(),
                ready_value: token.ready_value(),
                release_value: token.release_value(),
            };
            token.accept_release();
            // SAFETY: the handles stay open while `token`'s pool lives, which
            // outlasts this call; the sender signals release itself.
            let result = unsafe {
                sender
                    .sender
                    .send(&shared, nanalive_spout::GpuDx11SharedOptions::default())
            };
            result.err().map(|e| (spout_error_key(&e), e.to_string()))
        }
        #[cfg(target_os = "macos")]
        return self.syphon.publish(kind, name, gpu, frame);
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (frame, gpu);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_outputs_keep_their_size_and_alpha() {
        if !(cfg!(windows) || cfg!(target_os = "macos")) {
            return;
        }
        let mut s = Settings::default();
        s.output.display.enabled = true;
        s.output.display.size = Some([1280, 720]);
        s.output.peer_display.enabled = true;
        s.output.peer_display.size = Some([640, 360]);
        s.output.peer_display.straight_alpha = true;
        let own = config(WindowKind::Display, &s).unwrap();
        let peer = config(WindowKind::PeerDisplay, &s).unwrap();
        assert_eq!(
            own.extent,
            WindowOutputExtent::Fixed {
                width: 1280,
                height: 720
            }
        );
        assert_eq!(
            peer.extent,
            WindowOutputExtent::Fixed {
                width: 640,
                height: 360
            }
        );
        assert_eq!(own.alpha, WindowOutputAlpha::Premultiplied);
        assert_eq!(peer.alpha, WindowOutputAlpha::Straight);
        assert!(own.while_hidden && peer.while_hidden);
        s.output.peer_display.enabled = false;
        assert!(config(WindowKind::PeerDisplay, &s).is_none());
        assert_eq!(config(WindowKind::Display, &s), Some(own));
        assert!(config(WindowKind::History, &s).is_none());
    }

    #[test]
    fn partner_output_stops_on_leave_and_can_start_again() {
        if !(cfg!(windows) || cfg!(target_os = "macos")) {
            return;
        }
        let mut s = Settings::default();
        s.output.display.enabled = true;
        s.output.peer_display.enabled = true;
        let mut outputs = Spout::default();
        let locale = Locale::ZhCn;
        outputs.sync(&s, locale, false);
        assert!(matches!(
            outputs.status[&WindowKind::PeerDisplay],
            LinkState::Off
        ));
        outputs.sync(&s, locale, true);
        assert!(matches!(
            outputs.status[&WindowKind::PeerDisplay],
            LinkState::Connecting
        ));
        outputs
            .status
            .insert(WindowKind::Display, LinkState::Connected("self".into()));
        outputs.status.insert(
            WindowKind::PeerDisplay,
            LinkState::Connected("partner".into()),
        );
        outputs.sync(&s, locale, false);
        assert!(matches!(
            outputs.status[&WindowKind::PeerDisplay],
            LinkState::Off
        ));
        assert!(matches!(
            outputs.status[&WindowKind::Display],
            LinkState::Connected(_)
        ));
        outputs.sync(&s, locale, true);
        assert!(matches!(
            outputs.status[&WindowKind::PeerDisplay],
            LinkState::Connecting
        ));
    }

    #[test]
    fn duplicate_enabled_names_are_rejected() {
        let mut s = Settings::default();
        s.output.display.enabled = true;
        s.output.peer_display.enabled = true;
        s.output.peer_display.name = s.output.display.name.clone();
        assert!(name_conflicts(WindowKind::Display, &s));
        assert!(name_conflicts(WindowKind::PeerDisplay, &s));
        assert!(!name_conflicts(WindowKind::History, &s));
        assert!(config(WindowKind::Display, &s).is_none());
        assert!(config(WindowKind::PeerDisplay, &s).is_none());
        let mut outputs = Spout::default();
        outputs.sync(&s, Locale::ZhCn, true);
        assert!(matches!(
            outputs.status[&WindowKind::PeerDisplay],
            LinkState::Error(_)
        ));
        s.output.peer_display.enabled = false;
        assert!(!name_conflicts(WindowKind::Display, &s));
    }
}
