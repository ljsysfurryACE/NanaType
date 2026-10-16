# Bilibili interface provenance

The interface descriptions and signing rules below come from existing community
projects. NanaType handles integration, porting and adaptation.

- Original protocol documentation source:
  [SocialSisterYi/bilibili-API-collect](https://github.com/SocialSisterYi/bilibili-API-collect),
  by SocialSisterYi and the project's contributors. An accessible mirror is
  [bilibili-plugins/bilibili-api-collect](https://github.com/bilibili-plugins/bilibili-api-collect).
  The original project is currently unavailable; the mirror preserves its
  CC-BY-NC-4.0 licence (full text: `LICENSE-API-COLLECT`).
  Relevant documents: [WBI signing](https://github.com/bilibili-plugins/bilibili-api-collect/blob/master/docs/misc/sign/wbi.md),
  [live WebSocket protocol](https://github.com/bilibili-plugins/bilibili-api-collect/blob/master/docs/live/message_stream.md),
  [live danmaku](https://github.com/bilibili-plugins/bilibili-api-collect/blob/master/docs/live/danmaku.md),
  and [QR login](https://github.com/bilibili-plugins/bilibili-api-collect/blob/master/docs/login/login_action/QR.md).

The protocol comments in `src/live_ws.rs` are adapted summaries of the upstream
documentation. `src/wbi.rs` inherits the WBI
permutation table and signing rules, with a Rust implementation adapted for
NanaType. These inherited materials retain attribution and CC-BY-NC-4.0;
they are not relicensed as MIT. Local HTTP, credential storage, session management,
delivery queues and UI integration are maintained by NanaType.

CC-BY-NC-4.0 permits sharing and adaptation of covered material for NonCommercial
purposes with attribution and an indication of changes. Commercial reuse of that
material requires separate permission from the relevant rights holders. Its
disclaimer of warranties and limitation of liability remain in the full licence:
<https://creativecommons.org/licenses/by-nc/4.0/>.
