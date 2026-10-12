//! Spout2 output of the dialogue and history windows.
//!
//! NanaUI paints each window's own scene a second time into a shared DX12
//! texture (window output with native export); here every frame's token is
//! handed to a Spout sender that copies it on the GPU. Nothing is read back
//! to the CPU, and output keeps flowing while the window is hidden.

use std::collections::HashMap;

use nana_ui::{
    WindowOutputAlpha, WindowOutputConfig, WindowOutputExport, WindowOutputExtent,
    WindowOutputFrame, WindowOutputStatus,
};
use nanatype_i18n::Locale;
use nanatype_settings::{Settings, SpoutOutput};

use crate::ui::{I18N, LinkState};
use crate::windows::WindowKind;

pub fn settings_for(kind: WindowKind, s: &Settings) -> Option<&SpoutOutput> {
    match kind {
        WindowKind::Display => Some(&s.output.display),
        WindowKind::History => Some(&s.output.history),
        _ => None,
    }
}

/// The output NanaUI should produce for a window, if any.
pub fn config(kind: WindowKind, s: &Settings) -> Option<WindowOutputConfig> {
    if !cfg!(windows) {
        return None;
    }
    let out = settings_for(kind, s).filter(|o| o.enabled && !o.name.trim().is_empty())?;
    let extent = match out.size {
        Some([width, height]) => WindowOutputExtent::Fixed { width, height },
        None => WindowOutputExtent::MatchWindow,
    };
    Some(
        WindowOutputConfig::default()
            .with_extent(extent)
            .with_alpha(if out.straight_alpha {
                WindowOutputAlpha::Straight
            } else {
                WindowOutputAlpha::Premultiplied
            })
            .with_export(WindowOutputExport::Native)
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
    #[cfg(not(windows))]
    _unused: HashMap<WindowKind, ()>,
    /// Last link state per window, for the settings page.
    pub status: HashMap<WindowKind, LinkState>,
}

fn t(locale: Locale, key: &str) -> String {
    I18N.t_in(locale, key)
}

impl Spout {
    /// Drop senders whose output was turned off or renamed.
    pub fn sync(&mut self, s: &Settings, locale: Locale) {
        for kind in [WindowKind::Display, WindowKind::History] {
            let wanted = settings_for(kind, s).filter(|o| o.enabled);
            #[cfg(windows)]
            if self
                .senders
                .get(&kind)
                .is_some_and(|x| wanted.is_none_or(|o| o.name != x.name))
            {
                self.senders.remove(&kind);
            }
            let state = match wanted {
                None => LinkState::Off,
                Some(_) if !cfg!(windows) => {
                    LinkState::Error(t(locale, "output-spout-unsupported"))
                }
                Some(_) => match self.status.get(&kind) {
                    // Just switched on: no frame has gone out yet.
                    None | Some(LinkState::Off) => LinkState::Connecting,
                    Some(state) => state.clone(),
                },
            };
            self.status.insert(kind, state);
        }
    }

    pub fn status_changed(
        &mut self,
        kind: WindowKind,
        status: &WindowOutputStatus,
        locale: Locale,
    ) {
        let unavailable = |reason: &str| {
            LinkState::Error(I18N.tr_in(
                locale,
                "output-status-unavailable",
                &[("reason", reason.into())],
            ))
        };
        let state = match status {
            WindowOutputStatus::Active { extent, .. } => LinkState::Connected(format!(
                "{} · {}×{}",
                t(locale, "output-status-active"),
                extent[0],
                extent[1]
            )),
            WindowOutputStatus::NativeUnavailable { reason } => unavailable(reason),
            WindowOutputStatus::Failed { reason } => unavailable(reason),
            WindowOutputStatus::Stopped => LinkState::Off,
            WindowOutputStatus::NativeRetired { .. } => {
                #[cfg(windows)]
                if let Some(s) = self.senders.get_mut(&kind) {
                    // Handles change; the sender reopens on the next frame.
                    let _ = s;
                }
                return;
            }
            _ => return,
        };
        self.status.insert(kind, state);
    }

    /// Hand a frame to the window's Spout sender. Returns an error text when
    /// sending failed so the caller can surface it.
    #[allow(unused_variables)]
    pub fn frame(
        &mut self,
        kind: WindowKind,
        name: &str,
        frame: &WindowOutputFrame,
    ) -> Option<String> {
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
                    Err(e) => return Some(e.to_string()),
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
            result.err().map(|e| e.to_string())
        }
        #[cfg(not(windows))]
        {
            let _ = frame;
            None
        }
    }
}
