# NanaType

虚拟主播打字机：在直播中把你打的台词以 AVG / RPG 对话框的样式逐字呈现，并以 blivechat 风格保留历史记录。

- **输入框**：所见即所得——直接在与展示框完全一致的对话框里打字，顶部是富文本与特效工具栏；也可切换到标记源码编辑。
- **展示框**：透明的对话框窗口，打字机逐字揭示，支持逐字特效、停顿、翻页、震屏、立绘差分。
- **历史记录**：头像 + 名字 + 气泡的聊天列表，可随打字同步出现。
- **接入 OBS**：Spout2（GPU 共享纹理，零回读，窗口隐藏时也持续输出）或透明窗口捕获。
- **联动**：VTube Studio 热键、OSC、本地 WebSocket 广播。
- **皮肤**：内置 5 套（光环、夜色、羊皮纸、ADV 经典、RPG 窗口），可导入 / 导出 `.ntskin` 皮肤包。
- **多语言**：简体中文、English、日本語。

## 使用

1. 启动后输入框出现在屏幕下方，展示框与历史记录窗口为透明叠加层。
2. 在输入框的对话框里直接输入台词，按 **Enter** 发送（Shift+Enter 换行，可在设置里改为 Ctrl+Enter）。Ctrl+↑ / ↓ 调回发过的台词，Esc 隐藏。
3. 选中文字后用工具栏（或 Ctrl+B / I / U）加粗、上色、描边、注音、加动效、调节奏；不选中时，设置作用于接下来输入的文字。停顿、翻页、音效、表情等标记以细竖线显示在文字间，不占排版宽度，所以输入框与展示框的换行完全一致。
4. 工具栏的 `</>` 按钮切换到标记源码，两边编辑的是同一份台词。
5. 全局快捷键：**F8** 唤出输入框，**F9** 继续（越过停顿 / 跳过当前段）。
6. 托盘菜单可显示 / 隐藏各窗口、锁定叠加层、切换模式与皮肤、打开设置。

输入模式：

- **编辑后发送**：写完后发送，展示框按打字机节奏播放。
- **实时镜像**：展示框实时跟随输入，发送后台词进入历史。

### 接入 OBS

- **Spout2**：设置 › 输出 / OBS 打开 Spout2 输出。OBS 中安装 Spout2 插件，添加 *Spout2 Capture*，选择发送端名称（默认 `NanaType Display` / `NanaType History`），合成模式选 *Premultiplied Alpha*。
- **窗口捕获**：*Window Capture*，捕获方式选 *Windows 10 (1903 and up)*。
- macOS：使用 *macOS Screen Capture* 捕获对话框窗口（Spout2 仅 Windows）。

## 台词标记

台词以 BBCode 风格的标记保存：源码模式、快捷短语和模板都用它。完整说明见 [docs/markup.md](docs/markup.md)。

```text
[@auto=1500]大家好！[wait=400][b][color=#e86a92]今天[/color][/b]也来[wave]打字[/wave]啦～
[pause][shake amp=2]哇！[/shake][se=coin][expr=smile]
[ruby=かんじ]漢字[/ruby][sticker=cats/hello h=1.6]
```

## 皮肤

皮肤是一个含 `skin.toml` 的文件夹或 `.ntskin` 压缩包，可继承内置皮肤只写差异。格式见 [docs/skins.md](docs/skins.md)。

## 构建

需要 Rust 1.98+ 和 Windows 10 SDK（图标与版本资源）。NanaType 依赖相邻目录中的 NanaUI：

```
workspace/
  NanaType/
  sena-nana/NanaUI-app/      NanaUI（nanatype/integration 分支）
  sena-nana/nanalive-spout/  Spout2 发送端（gpu-dx11-shared 特性）
```

```bash
cargo run -p nanatype
```

```bash
cargo test --workspace --exclude nanatype
```

离屏渲染所有窗口的界面快照（不开桌面窗口）：

```bash
cargo run -p nanatype --example snapshots -- target/snapshots nanatype.night ja-JP
```

打包（需要 `nana-packager` 与 Inno Setup 6）：

```bash
pwsh scripts/package.ps1
```

开发辅助环境变量（仅调试构建或显式设置时生效）：

| 变量 | 作用 |
| --- | --- |
| `NANATYPE_DEBUG=<文件>` | 把窗口与输出事件追加写入该文件 |
| `NANATYPE_DEMO=1` | 启动 1.5 秒后播放示例台词（调试构建） |
| `NANATYPE_EXIT_AFTER=<毫秒>` | 到时正常退出（用于测量启动、刷出日志） |
| `NANATYPE_OPAQUE=1` | 叠加窗口改为不透明（调试构建） |

## 结构

| crate | 内容 |
| --- | --- |
| `crates/model` | 台词文档模型：文字段、注音、贴纸、标记 |
| `crates/markup` | 标记语言的解析与序列化、模板 |
| `crates/playback` | 揭示时间线、播放器状态机、实时镜像差分、队列 |
| `crates/theme` | 皮肤包：清单、继承、校验、安装与导出 |
| `crates/i18n` | Fluent 文案（zh-CN / en-US / ja-JP） |
| `crates/settings` | 设置结构、版本迁移、原子写入、窗口位置 |
| `crates/audio` | 解码、混音与按采样时刻调度 |
| `crates/bridge` | VTube Studio、OSC、WebSocket |
| `crates/core` | 会话逻辑：草稿、发送、继续、队列、历史、回溯 |
| `app` | NanaUI 宿主、窗口、视图、热键、托盘、Spout |

纯逻辑 crate 不依赖界面框架，可单独测试。

## 许可

MIT OR Apache-2.0
