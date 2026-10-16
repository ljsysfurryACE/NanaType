# NanaType promo video (Remotion)

40 s · 1920×1080 · 30 fps. Skins are re-created in React/CSS from `crates/theme/builtin/*.toml`;
the 18-skin grid uses real offscreen renders from the app itself.

```bash
npm install
npm run gen                         # skins TOML -> src/skins.generated.json
node scripts/gen-audio.mjs          # original BGM + sfx -> public/audio/*.wav (no third-party material)
# real app renders used by the skin grid (public/real, git-ignored):
(cd .. && cargo run -p nanatype --example snapshots -- promo/public/real all zh-CN)
npm run studio                      # preview
npm run capture:chat                # real NanaUI reply/@ interaction -> public/chat-parity/
npm run render                      # out/nanatype-promo.mp4
```

Music and sound effects are synthesised by `scripts/gen-audio.mjs`, so there is nothing to license.

## B 站封面

```bash
npm run cover                       # out/nanatype-bilibili-cover.png, 1920×1200
npm run cover:16x9                  # out/nanatype-bilibili-cover-16x9.png, 1920×1080
npm run cover:mobile                # out/nanatype-bilibili-cover-mobile.png, 384×240
npm run cover:4x3                   # out/nanatype-bilibili-cover-4x3.png, 1440×1080
npm run cover:mobile:4x3            # out/nanatype-bilibili-cover-mobile-4x3.png, 288×216
```

在 Studio 中选择 `BilibiliCover` 或 `BilibiliCover16x9` 预览。
`src/Cover.tsx` 使用光环皮肤的渐变、圆角和姓名栏样式，放大打字框左侧；
姓名栏展示现有 logo 和产品名，正文以大字展示产品定位及功能关键词。
产品名、定位和关键词居中排列在 1920×1080 设计画布的 x=280…1640 内，
在最窄的居中 4:3 裁剪范围 x=240…1680 中仍保留 40px 余量。
`BilibiliCover4x3` 直接模拟 16:9 版本的居中裁剪，手机预览也由 Remotion 直接渲染。

## 抖音封面

```bash
npm run cover:douyin                # out/nanatype-douyin-cover.png, 1080×1440（3:4）
npm run cover:douyin:mobile         # out/nanatype-douyin-cover-mobile.png, 324×432
```

在 Studio 中选择 `DouyinCover` 预览。`src/Cover.tsx` 的独立竖版布局沿用 promo 的
冰蓝配色、光环皮肤渐变姓名栏、产品 logo 和粉色光标；主标题分成“虚拟主播 / 打字机”两行，
下方展示四个功能关键词，并使用片头文案“你打的每一句，都是台词”。

## 弹幕交互实录

弹幕段的功能界面直接来自应用的 NanaUI 渲染器，包含实际按钮激活、编辑器输入、Core 回复归属和历史同步。主播操作面板使用紧凑列表，历史记录与展示框保留皮肤的气泡和引用效果。
Remotion 只组合标题、步骤和背景。默认录制使用实时镜像；不向 B 站发送消息，也不宣称通知了观众。

```bash
npm run capture:chat
npm run render:chat                 # out/chat-parity.mp4, 5 s standalone verification
# Additional profiles, from the repository root:
cargo run -p nanatype --example chat_parity -- target/chat-source nanatype.halo zh-CN source
cargo run -p nanatype --example chat_parity -- target/chat-queue nanatype.halo zh-CN queue
cargo run -p nanatype --example chat_parity -- target/chat-skins all zh-CN stills
cargo run -p nanatype --example chat_parity -- target/chat-skin-audit all zh-CN audit
# Refresh just the compact panel frames after a panel-only change:
cargo run -p nanatype --example chat_parity -- promo/public/chat-parity nanatype.halo zh-CN capture-panel
```

录制使用固定时间步，覆盖仅附带引用的回复后发送、再 @ 另一位观众、图片延迟补载、光标保持和撤销/重做断言。回复不自动插入 @，也不修改已有草稿与光标。
每次输出 panel/display/history 连续帧（capture 模式）或关键帧，另附输入窗口截图。capture-panel 只刷新 panel 连续帧，沿用已有的展示框与历史帧。
audit 在完整交互后加入长昵称、粉丝牌、多行富文本与长引用，检查各皮肤默认宽度及 300px 窄窗的历史与紧凑面板。输出实际截图、可访问性节点与 spacing-*.json，断言短记录顶部对齐、实际消息内容边界间距为 8–20px、文字及操作不横向溢出、历史不混入主播按钮。记录超出窗口时仍跟随最新消息滚动。
生成帧已忽略，重新克隆后须先运行 `capture:chat`。
