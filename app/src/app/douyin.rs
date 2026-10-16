use crate::app::App;
use nanatype_douyin::{AccountState, ConnectionState, Event, SessionEvent};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub account: AccountState,
    pub connection: ConnectionState,
    pub warning: Option<String>,
}
impl App {
    pub(super) fn douyin_event(&mut self, session: SessionEvent) {
        if session.epoch != self.douyin.epoch() {
            return;
        }
        match session.event {
            Event::Chat { room, event } => self._chat_assets.douyin(session.epoch, room, event),
            Event::Account(account) => self
                .ui
                .douyin
                .update(|state| std::sync::Arc::make_mut(state).account = account),
            Event::Connection(connection) => self
                .ui
                .douyin
                .update(|state| std::sync::Arc::make_mut(state).connection = connection),
            Event::Warning(code) => self
                .ui
                .douyin
                .update(|state| std::sync::Arc::make_mut(state).warning = Some(code.into())),
        }
    }
    pub(super) fn douyin_logout(&mut self) {
        self.edit(|s| s.integrations.douyin.enabled = false);
        self.ui.douyin_cookie.set(String::new());
        self.douyin.logout();
    }
    pub(super) fn douyin_import(&mut self, cookie: String) {
        self.edit(|s| s.integrations.douyin.enabled = false);
        self.ui.douyin_cookie.set(String::new());
        self.douyin.import_cookie(cookie);
    }
    pub(super) fn douyin_configure(&mut self) {
        let settings = &self.settings.integrations.douyin;
        self.douyin.set(nanatype_douyin::Config {
            enabled: settings.enabled,
            room: settings.room.clone(),
        });
    }
    pub(super) fn douyin_refresh(&mut self) {
        let epoch = self.douyin.epoch();
        self.ui
            .douyin
            .update(|s| std::sync::Arc::make_mut(s).warning = None);
        self.edit(|s| s.integrations.douyin.enabled = true);
        if self.douyin.epoch() == epoch {
            self.douyin.refresh();
        }
    }
}
