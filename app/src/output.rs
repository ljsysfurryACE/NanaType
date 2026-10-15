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
    resolve(kind, s, true).config()
}

#[derive(Clone, Copy)]
pub(crate) enum ResolvedOutput<'a> {
    Off,
    Invalid(&'static str),
    Ready(&'a SpoutOutput),
}

/// Configuration, sender lifetime and frame delivery share this decision.
/// Enabled names remain reserved even while the partner is disconnected.
pub(crate) fn resolve(kind: WindowKind, s: &Settings, peer_active: bool) -> ResolvedOutput<'_> {
    let Some(out) = settings_for(kind, s)
        .filter(|o| o.enabled && (kind != WindowKind::PeerDisplay || peer_active))
    else {
        return ResolvedOutput::Off;
    };
    if !(cfg!(windows) || cfg!(target_os = "macos")) {
        ResolvedOutput::Invalid("output-spout-unsupported")
    } else if out.name.trim().is_empty() {
        ResolvedOutput::Invalid("spout-error-name")
    } else if name_conflicts(kind, s) {
        ResolvedOutput::Invalid("output-name-conflict")
    } else {
        ResolvedOutput::Ready(out)
    }
}

impl<'a> ResolvedOutput<'a> {
    pub(crate) fn output(self) -> Option<&'a SpoutOutput> {
        match self {
            Self::Ready(out) => Some(out),
            _ => None,
        }
    }

    pub(crate) fn config(self) -> Option<WindowOutputConfig> {
        let out = self.output()?;
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
}

#[cfg(windows)]
struct Sender {
    name: String,
    luid: i64,
    sender: nanalive_spout::GpuDx11SharedSender,
}

#[derive(Default)]
pub struct Spout {
    /// Qualified sender identities, also used to reset status after a rename
    /// or after correcting an invalid configuration.
    names: HashMap<WindowKind, String>,
    #[cfg(windows)]
    senders: HashMap<WindowKind, Sender>,
    #[cfg(target_os = "macos")]
    syphon: syphon::Outputs,
    /// Last link state per window, for the settings page.
    pub status: HashMap<WindowKind, LinkState>,
}

fn t(locale: Locale, key: &str) -> String {
    I18N.t_in(locale, key)
}

impl Spout {
    /// Drop senders whose output was turned off or renamed.
    pub fn sync(&mut self, s: &Settings, locale: Locale, peer_active: bool) {
        #[cfg(target_os = "macos")]
        let mut live = Vec::new();
        for kind in KINDS {
            let resolved = resolve(kind, s, peer_active);
            let wanted = resolved.output();
            #[cfg(windows)]
            if self
                .senders
                .get(&kind)
                .is_some_and(|x| wanted.is_none_or(|o| o.name != x.name))
            {
                self.senders.remove(&kind);
            }
            let renamed = wanted.is_some_and(|o| self.names.get(&kind) != Some(&o.name));
            let state = match resolved {
                ResolvedOutput::Off => LinkState::Off,
                ResolvedOutput::Invalid(key) => LinkState::Error(t(locale, key)),
                ResolvedOutput::Ready(_) if renamed => LinkState::Connecting,
                ResolvedOutput::Ready(_) => match self.status.get(&kind) {
                    // Just switched on: no frame has gone out yet.
                    None | Some(LinkState::Off) => LinkState::Connecting,
                    Some(state) => state.clone(),
                },
            };
            if let Some(out) = wanted {
                if renamed {
                    self.names.insert(kind, out.name.clone());
                }
                #[cfg(target_os = "macos")]
                live.push((kind, out.name.clone()));
            } else {
                self.names.remove(&kind);
            }
            self.status.insert(kind, state);
        }
        #[cfg(target_os = "macos")]
        self.syphon.sync(&live);
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
    fn invalid_names_and_renames_share_configuration_and_sender_lifetime() {
        if !(cfg!(windows) || cfg!(target_os = "macos")) {
            return;
        }
        let mut settings = Settings::default();
        settings.output.display.enabled = true;
        settings.output.display.name = "   ".into();
        let mut outputs = Spout::default();
        outputs.sync(&settings, Locale::ZhCn, false);
        assert!(config(WindowKind::Display, &settings).is_none());
        assert!(outputs.names.is_empty());
        assert_eq!(
            outputs.status[&WindowKind::Display],
            LinkState::Error(t(Locale::ZhCn, "spout-error-name"))
        );
        settings.output.display.name = "Renamed".into();
        outputs.sync(&settings, Locale::ZhCn, false);
        assert!(config(WindowKind::Display, &settings).unwrap().while_hidden);
        assert_eq!(outputs.status[&WindowKind::Display], LinkState::Connecting);
        outputs
            .status
            .insert(WindowKind::Display, LinkState::Connected("ready".into()));
        outputs.sync(&settings, Locale::ZhCn, false);
        assert!(matches!(
            outputs.status[&WindowKind::Display],
            LinkState::Connected(_)
        ));
        settings.output.display.name = "Renamed again".into();
        outputs.sync(&settings, Locale::ZhCn, false);
        assert_eq!(outputs.status[&WindowKind::Display], LinkState::Connecting);
        assert_eq!(outputs.names[&WindowKind::Display], "Renamed again");
        settings.output.peer_display.enabled = true;
        assert!(
            resolve(WindowKind::PeerDisplay, &settings, false)
                .config()
                .is_none()
        );
        assert!(
            resolve(WindowKind::PeerDisplay, &settings, true)
                .config()
                .is_some()
        );
        settings.output.peer_display.name = settings.output.display.name.clone();
        outputs.sync(&settings, Locale::ZhCn, true);
        assert!(outputs.names.is_empty());
        assert!(
            resolve(WindowKind::Display, &settings, true)
                .output()
                .is_none()
        );
        settings.output.peer_display.enabled = false;
        outputs.sync(&settings, Locale::ZhCn, false);
        assert_eq!(outputs.status[&WindowKind::Display], LinkState::Connecting);
    }

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
