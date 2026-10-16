use crate::{app::App, msg::Msg, ui::settings::Page};
use nanatype_bilibili::{
    AccountState, ConnectionState, Delivery, DeliveryState, Event, SessionEvent, sending,
};
use nanatype_core::{Cmd, Event as CoreEvent};
use nanatype_model::Speaker;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

type EchoKey = (u64, u64, String);
#[derive(Default)]
pub(super) struct Echoes {
    sent: HashSet<EchoKey>,
    order: VecDeque<EchoKey>,
    rows: HashMap<EchoKey, u64>,
    super_chats: HashMap<(u64, String), u64>,
    deleted: HashSet<(u64, String)>,
    deleted_order: VecDeque<(u64, String)>,
}
impl Echoes {
    fn confirm(&mut self, key: EchoKey) -> Option<u64> {
        if key.2.is_empty() {
            return None;
        }
        if self.sent.insert(key.clone()) {
            self.order.push_back(key.clone());
        }
        while self.order.len() > 16384 {
            if let Some(old) = self.order.pop_front() {
                self.sent.remove(&old);
            }
        }
        self.rows.remove(&key)
    }
    fn suppress(&self, key: &EchoKey) -> bool {
        !key.2.is_empty() && (self.sent.contains(key) || self.rows.contains_key(key))
    }
    fn delete_super_chats(&mut self, room: u64, ids: &[String]) -> Vec<u64> {
        let mut rows = Vec::new();
        for id in ids {
            let key = (room, id.clone());
            if self.deleted.insert(key.clone()) {
                self.deleted_order.push_back(key.clone());
            }
            if let Some(row) = self.super_chats.remove(&key) {
                rows.push(row);
            }
        }
        while self.deleted_order.len() > 16384 {
            if let Some(key) = self.deleted_order.pop_front() {
                self.deleted.remove(&key);
            }
        }
        rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(id: &str) -> EchoKey {
        (1, 100, id.into())
    }
    #[test]
    fn receipts_filter_only_confirmed_echoes_in_either_arrival_order() {
        let mut echoes = Echoes::default();
        echoes.rows.insert(key("early"), 8);
        assert_eq!(echoes.confirm(key("early")), Some(8));
        assert!(echoes.suppress(&key("early")));
        assert_eq!(echoes.confirm(key("late")), None);
        assert!(echoes.suppress(&key("late")));
        assert!(!echoes.suppress(&key("other-client-same-text")));
        assert!(!echoes.suppress(&(1, 101, "early".into())));
        assert!(!echoes.suppress(&(2, 100, "early".into())));
        assert_eq!(echoes.confirm(key("")), None);
        assert!(!echoes.suppress(&key("")));
    }
    #[test]
    fn super_chat_deletion_survives_delayed_ingestion_and_room_changes() {
        let mut echoes = Echoes::default();
        echoes.super_chats.insert((100, "old".into()), 8);
        assert_eq!(
            echoes.delete_super_chats(100, &["old".into(), "early".into()]),
            vec![8]
        );
        assert!(echoes.deleted.contains(&(100, "early".into())));
        assert!(!echoes.deleted.contains(&(101, "early".into())));
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub account: AccountState,
    pub connection: ConnectionState,
    pub avatar: Option<String>,
    pub sending: bool,
    pub confirm: bool,
    pub deliveries: Vec<Delivery>,
}
impl State {
    pub fn can_send(&self) -> bool {
        matches!(self.account, AccountState::Ready(_))
            && matches!(self.connection, ConnectionState::Connected(_))
    }
    pub fn pending(&self) -> usize {
        self.deliveries
            .iter()
            .filter(|d| matches!(d.state, DeliveryState::Queued | DeliveryState::Sending))
            .count()
    }
}
impl App {
    pub(super) fn bili_state(&self, edit: impl FnOnce(&mut State)) {
        self.ui.bilibili.update(|state| edit(Arc::make_mut(state)));
    }
    pub(super) fn bilibili_disable_send(&mut self) {
        self.bilibili.set_sending(false);
        self.bili_state(|s| {
            s.sending = false;
            s.confirm = false;
            for d in &mut s.deliveries {
                d.blocked = false;
                if matches!(d.state, DeliveryState::Queued | DeliveryState::Sending) {
                    d.state = DeliveryState::Cancelled;
                }
            }
        });
    }
    pub(super) fn bilibili_reconnecting(&mut self) {
        self.bilibili_connection(ConnectionState::Connecting);
    }
    pub(super) fn bilibili_reset(&mut self) {
        self.bili_state(|s| {
            s.account = AccountState::SignedOut;
            s.avatar = None;
        });
        self.bilibili_connection(ConnectionState::NeedsLogin);
        self.sync_bilibili_speaker();
    }
    fn bilibili_connection(&mut self, connection: ConnectionState) {
        if !matches!(connection, ConnectionState::Connected(_)) {
            self.bilibili_disable_send();
        }
        let (label, link) =
            crate::ui::link_text::bilibili(self.ui.locale.get_untracked(), &connection, "");
        self.edit_status(|s| {
            s.bilibili = label;
            s.bilibili_link = link;
        });
        self.bili_state(|s| s.connection = connection);
    }
    pub(super) fn bilibili_login(&mut self) {
        self.bilibili_reset();
        self.ui.page.set(Page::Bilibili);
        self.open_settings();
        self.bilibili.login();
    }
    pub(super) fn bilibili_toggle(&mut self) {
        let state = self.ui.bilibili.get_untracked();
        if state.sending {
            self.bilibili_disable_send();
            return;
        }
        if state.can_send() {
            self.bili_state(|s| s.confirm = true);
        }
        self.ui.page.set(Page::Bilibili);
        self.open_settings();
    }
    pub(super) fn bilibili_confirm(&mut self, accept: bool) {
        let state = self.ui.bilibili.get_untracked();
        if !state.confirm {
            return;
        }
        let enable = accept && state.can_send();
        self.bili_state(|s| {
            s.confirm = false;
            s.sending = enable;
        });
        self.bilibili.set_sending(enable);
    }
    pub(super) fn bilibili_preflight(&mut self, cmd: &Cmd) -> bool {
        if !self.ui.bilibili.get_untracked().sending {
            return true;
        }
        let invalid = match cmd {
            Cmd::Send => {
                sending::plain_text(self.core.draft()).is_err() && !self.core.draft().is_blank()
            }
            Cmd::SendMarkup(markup) => {
                let doc = nanatype_markup::parse(markup).doc;
                !doc.is_blank() && sending::plain_text(&doc).is_err()
            }
            Cmd::DraftChanged(doc) => {
                sending::image_count(doc) > sending::image_count(self.core.draft())
            }
            _ => false,
        };
        if invalid {
            self.notice(self.ui.now_t("bili-images-unavailable"), true);
        }
        !invalid
    }
    pub(super) fn bilibili_submitted(&mut self, event: &CoreEvent) {
        if !self.ui.bilibili.get_untracked().sending {
            return;
        }
        if let CoreEvent::LineSubmitted(line) = event
            && let Ok(text) = sending::plain_text(&line.doc)
            && !self.bilibili.send(line.id, text)
        {
            self.notice(self.ui.now_t("bili-queue-full"), true);
        }
    }
    pub(super) fn sync_bilibili_speaker(&mut self) {
        let state = self.ui.bilibili.get_untracked();
        let speaker = match &state.account {
            AccountState::Ready(p) if self.settings.integrations.bilibili.sync_profile => Speaker {
                name: p.name.clone(),
                avatar: state.avatar.clone(),
                portrait: None,
            },
            _ => Speaker {
                name: self.settings.general.speaker_name.clone(),
                avatar: self.settings.general.avatar.clone(),
                portrait: None,
            },
        };
        self.ui.speaker.set(speaker.clone());
        let fx = self
            .core
            .handle(Cmd::SetSpeaker(Some(speaker)), crate::clock::now_ms());
        self.apply_effects(fx);
    }
    pub(super) fn bilibili_event(&mut self, session: SessionEvent) {
        if let Event::Delivery(delivery) = &session.event {
            let mut remove = Vec::new();
            for id in &delivery.receipts {
                let key = (session.epoch, delivery.room, id.clone());
                if let Some(row) = self.bili_echoes.confirm(key) {
                    remove.push(row);
                }
            }
            if !remove.is_empty() {
                self.run(Cmd::RemoveHistoryRows(remove));
            }
        }
        if session.epoch != self.bilibili.epoch() {
            return;
        }
        match session.event {
            Event::Account(account) => {
                if let AccountState::Ready(profile) = &account {
                    let asset = crate::services::chat_assets::cache_profile_avatar(
                        self.paths.blobs(),
                        &profile.uid.to_string(),
                        &profile.avatar_url,
                        {
                            let ctx = self.ctx.clone();
                            move |asset| ctx.dispatch(Msg::ChatAssetReady(asset))
                        },
                    );
                    self.bili_state(|s| s.avatar = asset.map(|a| a.to_string()));
                }
                if matches!(
                    account,
                    AccountState::Expired | AccountState::Failed(_) | AccountState::SignedOut
                ) {
                    self.bilibili_connection(ConnectionState::NeedsLogin);
                } else if !matches!(account, AccountState::Ready(_)) {
                    self.bilibili_disable_send();
                }
                self.bili_state(|s| s.account = account);
                self.sync_bilibili_speaker();
            }
            Event::Connection(connection) => {
                self.bilibili_connection(connection);
            }
            Event::Chat { room, event } => {
                if let nanatype_bilibili::ChatEvent::DeleteSuperChats { ids } = &event {
                    let keys = self.bili_echoes.delete_super_chats(room, ids);
                    self.run(Cmd::RemoveHistoryRows(keys));
                    self.chat.retain(|m| {
                        !(m.platform == crate::chat::Platform::Bilibili
                            && m.room == room.to_string()
                            && matches!(m.kind, crate::chat::ChatKind::SuperChat { .. })
                            && ids.contains(&m.id))
                    });
                    self.publish_chat();
                } else {
                    self._chat_assets.bilibili(session.epoch, room, event);
                }
            }
            Event::Delivery(delivery) => {
                self.bili_state(|s| {
                    if let Some(old) = s.deliveries.iter_mut().find(|d| d.line == delivery.line) {
                        *old = delivery.clone();
                    } else {
                        s.deliveries.push(delivery.clone());
                    }
                    while s.deliveries.len() > 200 {
                        if let Some(index) = s.deliveries.iter().position(|d| {
                            !d.blocked
                                && !matches!(
                                    d.state,
                                    DeliveryState::Queued | DeliveryState::Sending
                                )
                        }) {
                            s.deliveries.remove(index);
                        } else {
                            break;
                        }
                    }
                });
                if matches!(
                    delivery.state,
                    DeliveryState::Failed | DeliveryState::Uncertain
                ) {
                    self.notice(
                        format!(
                            "{}: {}",
                            self.ui.now_t("bili-delivery-failed"),
                            delivery.detail
                        ),
                        true,
                    );
                }
            }
            Event::Warning(message) => self.notice(message, true),
        }
    }
    pub(super) fn bilibili_history(&mut self, message: &crate::chat::ChatMessage) -> bool {
        let key = (
            message.epoch,
            message.room.parse().unwrap_or_default(),
            message.id.clone(),
        );
        if matches!(message.kind, crate::chat::ChatKind::Text) && self.bili_echoes.suppress(&key) {
            return false;
        }
        true
    }
    pub(super) fn bilibili_record_row(&mut self, message: &crate::chat::ChatMessage) {
        if let Some(row) = self.core.history().back() {
            if matches!(message.kind, crate::chat::ChatKind::SuperChat { .. }) {
                self.bili_echoes.super_chats.insert(
                    (message.room.parse().unwrap_or_default(), message.id.clone()),
                    row.key,
                );
            }
            self.bili_echoes.rows.insert(
                (
                    message.epoch,
                    message.room.parse().unwrap_or_default(),
                    message.id.clone(),
                ),
                row.key,
            );
        }
        let oldest = self
            .core
            .history()
            .front()
            .map(|r| r.key)
            .unwrap_or(u64::MAX);
        self.bili_echoes.rows.retain(|_, row| *row >= oldest);
        self.bili_echoes.super_chats.retain(|_, row| *row >= oldest);
    }
    pub(super) fn bilibili_message_deleted(&self, message: &crate::chat::ChatMessage) -> bool {
        matches!(message.kind, crate::chat::ChatKind::SuperChat { .. })
            && self
                .bili_echoes
                .deleted
                .contains(&(message.room.parse().unwrap_or_default(), message.id.clone()))
    }
}
