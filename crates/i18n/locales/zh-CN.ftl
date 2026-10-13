## 应用
app-name = NanaType
app-tagline = 虚拟主播打字机
history-count = { $count } 条

## 窗口标题
window-input = 输入框
window-display = 展示框
window-history = 历史记录
window-settings = 设置

## 托盘
tray-mode = 输入模式
tray-skin = 皮肤
tray-quit = 退出 NanaType
tray-action-show-input = 显示输入框
tray-action-hide-input = 隐藏输入框
tray-action-show-display = 显示对话框
tray-action-hide-display = 隐藏对话框
tray-action-show-history = 显示历史记录
tray-action-hide-history = 隐藏历史记录
tray-action-show-chat = 打开弹幕面板
tray-action-hide-chat = 关闭弹幕面板
tray-action-lock = 锁定叠加窗口
tray-action-unlock = 解锁叠加窗口
tray-action-continue = 继续
tray-action-sample = 播放示例台词
tray-action-settings = 打开设置…
tray-action-obs = 接入 OBS…
tray-tooltip = NanaType — { $mode } · { $skin }
tray-tooltip-error = NanaType · ⚠ { $message }

## 输入模式
mode-compose = 编辑后发送
mode-compose-hint = 编辑完成后发送，展示框以打字机效果播放
mode-mirror = 实时镜像
mode-mirror-hint = 展示框实时跟随输入内容
mode-compose-short = 编辑
mode-mirror-short = 镜像

## 工具栏
tb-bold = 粗体
tb-italic = 斜体
tb-underline = 下划线
tb-strike = 删除线
tb-color = 文字颜色
tb-size = 字号
tb-stroke = 描边
tb-shadow = 阴影
tb-stroke-none = 去除描边
tb-shadow-none = 去除阴影
tb-ruby = 注音
tb-ruby-placeholder = 输入注音，例如 かんじ
tb-ruby-apply = 添加注音
tb-ruby-remove = 移除注音
tb-effects = 动效
tb-effect-shake = 抖动
tb-effect-wave = 波浪
tb-effect-jump = 跳跃
tb-effect-rainbow = 彩虹
tb-entrance-skin = 跟随皮肤
tb-entrance-none = 无
tb-entrance-fade = 淡入
tb-entrance-pop = 弹出
tb-entrance-slide = 上浮
tb-pacing = 节奏
tb-wait-ms = 停顿 { $ms } 毫秒
tb-speed-slow = 放慢
tb-speed-fast = 加快
tb-speed-instant = 瞬间显示
tb-speed-reset = 恢复速度
tb-pause = 等待继续
tb-pause-auto = 暂停后自动继续
tb-page = 翻页
tb-insert = 插入
tb-sticker = 贴纸
tb-sticker-search = 搜索贴纸
tb-image = 图片…
tb-sound = 音效
tb-expression = 表情
tb-expression-placeholder = 表情名称，例如 smile
tb-screenshake = 震屏
tb-phrases = 快捷短语
tb-source = 标记源码
tb-send = 发送
tb-clear-format = 清除格式
tb-more = 更多

## 编辑器
editor-placeholder = 在这里输入台词…
editor-source-placeholder = 使用 [b]标记[/b] 编辑，例如 [shake]哇！[/shake][wait=500]
editor-diagnostic = 第 { $pos } 个字符：{ $message }
marker-wait = 停 { $ms }ms
marker-pause = 等待
marker-pause-auto = 等 { $ms }ms
marker-page = 翻页
marker-sound = 音 { $name }
marker-expression = 表情 { $name }
marker-portrait = 立绘 { $name }
marker-shake = 震屏
marker-vts = VTS { $name }
status-mirror = 镜像中
status-queued-chip = 排队 { $position }
status-sent = 已发送
empty-phrases = 还没有快捷短语
empty-stickers = 还没有贴纸目录
empty-open-assets = 去 设置 › 素材 添加
empty-vts-hotkeys = 还没有读取到 VTube Studio 热键

## 设置导航
settings-general = 常规
settings-playback = 播放
settings-skins = 皮肤
settings-windows = 窗口
settings-history = 历史记录
settings-hotkeys = 快捷键
settings-output = 输出 / OBS
settings-integrations = 联动
settings-assets = 素材
settings-about = 关于

## 常规
general-language = 界面语言
general-language-system = 跟随系统
general-speaker = 角色名
general-speaker-hint = 显示在名牌和历史记录中，留空则隐藏名牌
general-avatar = 头像
general-avatar-pick = 选择图片…
general-avatar-clear = 移除
general-presence = 出现方式
general-presence-resident = 常驻
general-presence-summon = 快捷键唤出
general-presence-hint = 唤出模式下，发送或按 Esc 后输入框自动隐藏
general-input = 输入
general-mode = 输入模式
general-send-key = 发送键
general-send-key-enter = Enter
general-send-key-ctrl-enter = Ctrl+Enter
general-send-key-hint = 另一种组合用于换行
general-autostart = 开机启动

## 播放
playback-speed = 打字速度
playback-typing = 打字机效果
playback-typing-hint = 关闭后整句直接显示
playback-punct = 标点停顿
playback-punct-hint = 逗号、句号和省略号后稍作停顿
playback-entrance = 文字出场
playback-after = 播放结束后
playback-flow = 台词流转
playback-after-hold = 保持显示
playback-after-fade = 渐隐消失
playback-after-clear = 直接清除
playback-after-delay = 停留时长
playback-after-delay-value = { $seconds } 秒
playback-auto = 自动推进下一句
playback-auto-off = 关闭
playback-auto-value = { $seconds } 秒后
playback-queue = 正在播放时发送新台词
playback-queue-replace = 立即替换
playback-queue-finish = 先完整显示当前句
playback-queue-queue = 排队等待继续键
playback-queue-replace-short = 立即替换
playback-queue-finish-short = 先播完
playback-queue-queue-short = 排队
playback-reduced = 减少动态效果
playback-reduced-hint = 关闭逐字动效与震屏，适合对动态敏感的观众
playback-stagger = 镜像模式字间隔
playback-sound = 打字音效
playback-audio = 声音
playback-volume = 主音量
playback-volume-typing = 打字音量
playback-volume-effects = 音效音量
playback-device = 输出设备
playback-device-default = 系统默认

## 皮肤
skins-active = 当前
skins-library = 皮肤库
skins-import = 导入皮肤…
skins-export = 导出…
skins-duplicate = 复制为可编辑
skins-open-folder = 打开文件夹
skins-delete = 删除
skins-delete-confirm = 删除皮肤「{ $name }」？此操作无法撤销。
skins-builtin = 内置
skins-broken = 无法加载
skins-by = 作者 { $author }
skins-version = 版本 { $version }
skins-overrides = 个性化
skins-override-text = 文字颜色
skins-override-name = 名牌颜色
skins-override-box = 展示框颜色
skins-override-accent = 强调色
skins-override-size = 文字大小
skins-override-overflow = 长文本
skins-override-overflow-hint = 一句话超出展示框高度时的处理方式
skins-override-overflow-skin = 跟随皮肤
skins-override-overflow-clip = 裁掉
skins-override-overflow-scroll = 跟随滚动
skins-override-overflow-page = 分页（等待后清空）
skins-override-page-auto = 满页自动翻页
skins-override-page-auto-hint = 分页模式下满一页后等待的秒数；0 表示等继续键
skins-override-reset = 恢复皮肤默认
skins-chrome = 界面外观
skins-interface = 界面
skins-chrome-skin = 跟随皮肤
skins-chrome-system = 跟随系统
skins-chrome-light = 浅色
skins-chrome-dark = 深色
skins-hot-reload = 文件变化时自动重载
skins-hot-reload-hint = 制作皮肤时使用
skins-installed = 已安装「{ $name }」
skins-replaced = 已将「{ $name }」从 { $previous } 更新
skins-import-failed = 皮肤无法导入
skins-fallback = 皮肤「{ $id }」无法加载，已改用默认皮肤
skins-preview = 预览
skins-preview-name = 奈奈
skins-preview-line = 大家好！今天也来[b]打字[/b]啦～[wave]波浪[/wave]、还有[rainbow]彩虹[/rainbow]和[shake]抖动[/shake]都可以哦。
skins-filter-all = 全部
skins-tag-light = 浅色
skins-tag-dark = 深色
skins-tag-classic = 经典
skins-tag-streaming = 直播
skins-tag-game = 游戏
skins-tag-retro = 复古
skins-tag-cute = 可爱
skins-tag-elegant = 雅致
skins-tag-minimal = 简约

## 窗口
windows-input = 输入框
windows-display = 展示框
windows-history = 历史记录
windows-visible = 显示
windows-lock = 交互
windows-lock-interactive = 可交互
windows-lock-edit = 调整布局
windows-on-top = 置顶
windows-background = 捕获背景
windows-background-transparent = 透明
windows-background-chroma = 色键
windows-reset = 重置位置
windows-size = 尺寸
windows-size-skin = 跟随皮肤

## 历史
history-max = 保留条数
history-visible = 显示条数
history-fade = 自动淡出
history-avatar = 显示头像
history-name = 显示名字
history-time = 显示时间
history-sync = 出现时机
history-sync-submit = 发送时
history-sync-reveal = 随打字同步
history-sync-complete = 显示完成后
history-hide-latest = 隐藏最新一条
history-hide-latest-hint = 展示框上还显示着的那句先不进历史，展示框清空或换下一句后再出现（同 Echo-Live）
history-hide-latest-skin = 跟随皮肤
history-hide-latest-always = 隐藏
history-hide-latest-never = 不隐藏
history-clear = 清空历史记录
history-clear-confirm = 清空本次的全部历史记录？
history-session = 本场历史记录

## 快捷键
hotkeys-summon = 唤出输入框
hotkeys-continue = 继续
hotkeys-skip = 跳过当前句
hotkeys-clear = 清空展示框
hotkeys-toggle-mode = 切换输入模式
hotkeys-toggle-overlays = 显示/隐藏叠加窗口
hotkeys-record = 录制
hotkeys-clear-binding = 清除
hotkeys-ok = 已注册
hotkeys-conflict-os = 被其他程序占用
hotkeys-conflict-dup = 与「{ $action }」重复
hotkeys-unsafe = 单个按键会影响正常打字，请加上 Ctrl、Alt 或 Shift
hotkeys-badge-dup = 重复
hotkeys-badge-conflict = 冲突
hotkeys-badge-unsafe = 不安全
hotkeys-click-to-set = 点击设置
hotkeys-press-keys = 请按下按键 · Esc 取消
hotkeys-local = 输入框内快捷键
hotkeys-local-hint = Ctrl+B/I/U 设置样式，Ctrl+↑/↓ 调出已发送的台词，Esc 隐藏输入框

## 输出
output-display = 展示框
output-history = 历史记录
output-spout = Spout2 输出
output-spout-name = 发送端名称
output-spout-copy = 复制名称
output-spout-size = 输出分辨率
output-spout-size-window = 跟随窗口
output-spout-straight = 直通 Alpha
output-spout-straight-hint = 接收端不支持预乘 Alpha 时开启
output-status-active = 正在发送
output-status-unavailable = 不可用：{ $reason }
output-spout-unsupported = Spout2 仅支持 Windows，请使用窗口捕获
output-sample = 播放示例台词
output-sample-line = [b]你好！[/b]这是 NanaType 的[wave]示例台词[/wave]。[wait=300][shake]看得到吗？[/shake]
output-obs-title = 在 OBS 中接入
output-obs-window = 也可以使用「窗口捕获」（捕获方式选 Windows 10 (1903 及以上)），展示框窗口背景为透明。
output-obs-mac = macOS 上请使用「macOS 屏幕捕获」并选择展示框窗口。

## 联动
int-vts = VTube Studio
int-vts-hint = 打字到 [expr=名称] 或 [vts …] 标记时，开启 VTube Studio 表情或触发热键
int-vts-port = 端口
int-vts-authorize = 请求授权
int-vts-authorize-hint = 点击后请在 VTube Studio 中允许 NanaType 插件
int-vts-map = 表情映射
int-vts-map-expression = 标记
int-vts-map-hotkey = VTS 热键
int-vts-map-auto = 自动（同名）
int-vts-refresh = 刷新热键列表
int-vts-lipsync = 口型同步
int-vts-lipsync-hint = 打字时让模型的嘴动起来
int-vts-mouth-param = 嘴部参数
int-vts-mouth-param-hint = 说话时驱动的输入参数（默认 MouthOpen）
int-vts-mouth-amp = 张嘴幅度
int-vts-reset = 表情复位
int-vts-reset-hint = 关闭一句台词中开启的表情
int-vts-reset-line-end = 台词离开展示框时
int-vts-reset-next-line = 下一句开始时
int-vts-reset-off = 不复位
int-osc = OSC
int-osc-hint = 向 VNyan、Warudo、VSeeFace 等工具发送 OSC 消息
int-osc-host = 目标地址
int-osc-port = 端口
int-osc-prefix = 地址前缀
int-ws = WebSocket 广播
int-ws-hint = 在本机端口广播台词与表情事件（JSON）
int-ws-port = 端口
int-ws-clients = 已连接 { $count } 个客户端
int-status-off = 未启用
int-status-connecting = 连接中…
int-status-connected = 已连接
int-status-needs-token = 需要授权
int-status-error = 错误：{ $message }

int-off-hint = 开启后可在此设置连接选项
output-off-hint = 开启后可在此设置名称与尺寸
## 素材
assets-stickers = 贴纸目录
assets-stickers-hint = 目录中的图片成为贴纸，文件夹名即贴纸库名
assets-add-folder = 添加目录…
assets-remove = 移除
assets-remove-folder-confirm = 从贴纸中移除这个文件夹？
assets-phrases = 快捷短语
assets-phrase-label = 名称
assets-phrase-markup = 内容（支持标记）
assets-phrase-send = 直接发送
assets-phrase-add = 添加短语
assets-phrase-remove-confirm = 删除这条短语？
assets-phrase-hotkey = 快捷键

## 关于
about-build = 构建 { $build }
about-logs = 打开日志文件夹
about-data = 打开数据文件夹
about-homepage = 项目主页

## 首次启动
onboard-welcome = 欢迎使用 NanaType
onboard-welcome-body = 只需几步即可开始在直播中使用打字机。
onboard-language = 选择语言
onboard-speaker = 你的角色名
onboard-skin = 选择皮肤
onboard-hotkeys = 快捷键
onboard-hotkeys-body = 按 { $summon } 唤出输入框，按 { $continue } 继续播放。
onboard-obs = 接入 OBS
onboard-next = 下一步
onboard-back = 上一步
onboard-skip = 跳过
onboard-done = 开始使用

## 通用
common-cancel = 取消
common-apply = 应用
common-reset = 重置
common-copied = 已复制
common-ms = { $ms } 毫秒
common-skin-default = 跟随皮肤

## 错误与提示
err-settings-quarantined = 设置文件已损坏，已备份到 { $path } 并使用默认设置
err-settings-newer = 设置文件来自更新版本的 NanaType，本次修改不会保存
err-audio = 音频设备不可用：{ $message }
err-image = 图片无法读取：{ $message }
err-spout = Spout 输出失败：{ $message }
# blivechat
window-chat = 弹幕
chat-mention = @ 提及
chat-reply = 回复
chat-replying = 回复 { $name }
chat-reply-cancel = 取消回复
chat-clear = 清空
chat-filter-text = 弹幕
chat-filter-super-chat = 醒目留言
chat-filter-gift = 礼物
chat-filter-member = 上舰
chat-empty = 等待弹幕…
chat-not-configured = 还没有连接 blivechat
chat-gift = 赠送 { $gift } ×{ $count }
chat-member = 开通了 { $guard } ×{ $count }{ $unit }
chat-guard-governor = 总督
chat-guard-admiral = 提督
chat-guard-captain = 舰长
chat-min-gift = 礼物最低价值
chat-min-gift-hint = 单位：元；免费礼物价值为 0
chat-status-joined = 已进入房间 { $room }
chat-status-retrying = 连接断开，正在重试：{ $message }
chat-status-no-room = 请填写身份码或房间号
chat-fatal = blivechat 停止了连接：{ $message }
history-chat = 弹幕
history-chat-show = 在历史记录中显示弹幕
history-chat-show-hint = blivechat 的弹幕、醒目留言、礼物与上舰会和台词一起出现在历史记录里
history-show-quote = 显示回复引用
history-show-quote-hint = 回复弹幕的台词上方显示被回复的弹幕
history-role-host = 主播
history-role-viewer = 观众
int-blive-section = blivechat 弹幕
int-blive = 连接 blivechat
int-blive-hint = 从本机运行的 blivechat 读取 B 站直播弹幕，可在弹幕面板里 @ 或回复观众
int-blive-room-kind = 房间
int-blive-room-kind-hint = 推荐使用开放平台身份码；房间号连接 blivechat 已不再维护
int-blive-auth-code = 身份码
int-blive-room-id = 房间号
int-blive-room = 身份码 / 房间号
int-blive-room-placeholder = 粘贴身份码或输入房间号
int-blive-connect = 连接
int-blive-host = blivechat 地址
int-blive-host-hint = 运行 blivechat 的主机，默认端口 12450
int-blive-port = 端口
int-blive-panel = 弹幕面板
int-blive-panel-hint = 仅供主播操作的窗口，不会被采集
int-blive-open-panel = 打开弹幕面板
int-blive-quote-display = 对话框显示回复引用
int-blive-quote-display-hint = 回复弹幕时，对话框文字上方显示被回复的弹幕

## wave3-input
tb-cheatsheet = 标记速查
tb-stroke-white = 白色描边
tb-stroke-black = 黑色描边
tb-stroke-accent = 主题色描边
tb-shadow-drop = 投影
tb-shadow-glow = 发光
cheat-markers = 标记
cheat-shortcuts = 快捷键
cheat-wait = 停顿指定毫秒后继续
cheat-pause = 停住，按继续后再往下打
cheat-page = 翻到新的一页
cheat-expr = 切换立绘表情
cheat-se = 播放音效
cheat-sticker = 插入贴纸
cheat-at = 提到一位观众
cheat-newline = 换行
cheat-recall = 上一句 / 下一句
cheat-hide = 隐藏输入窗
## end wave3-input

## wave3-settings
settings-group-input = 输入
settings-group-look = 外观
settings-group-stream = 直播
settings-general-sub = 语言、你的名字，以及输入框的行为
settings-playback-sub = 台词如何逐字出现、推进与发声
settings-skins-sub = 为对话框挑选外观，并按你的喜好调整
settings-windows-sub = 显示、锁定或重置展示框、历史记录和输入框
settings-history-sub = 历史记录保留和显示哪些内容
settings-hotkeys-sub = 全局快捷键，在游戏或 OBS 里同样有效
settings-output-sub = 把展示框和历史记录送进 OBS
settings-integrations-sub = 弹幕、VTube Studio 与其他应用联动
settings-assets-sub = 输入框可用的贴纸和快捷短语
settings-about-sub = 版本、文件夹与链接
output-obs-step-plugin = 为 OBS 安装 Spout2 插件
output-obs-step-source = 添加「Spout2 Capture」来源，选择这个发送端：
output-obs-step-alpha = 将合成模式设为「预乘 Alpha」
output-obs-other = 其他方式
int-vts-authorized = 已授权
int-vts-map-empty = 连接并授权后即可列出你的 VTS 热键
skins-more = 更多操作
## end wave3-settings

## wave4-skins
skins-view-grid = 卡片
skins-view-list = 列表
windows-lock-locked-short = 锁定
windows-lock-tip = 锁定：点击穿透到下层窗口。可交互：可以点击和滚动。调整布局：拖动以移动和缩放。
windows-background-tip = 透明：为 OBS 保留透明度。色键：用绿色填满窗口，供不支持透明度的捕获方式抠像。
windows-reset-tip = 重置按钮会把窗口移回皮肤设定的位置，并恢复皮肤的尺寸
windows-input-na = “—”表示不适用：输入框始终可交互，也不会被捕获。
history-window-link = 窗口与捕获
history-window-link-hint = 显示、锁定历史记录窗口并调整尺寸，或发送到 OBS
## end wave4-skins

## wave4-onboarding
onboard-step = 第 { $step } 步，共 { $total } 步
onboard-skin-hint = 为对话框挑一个外观，下方实时预览。更多调整在「皮肤」页。
onboard-mode-body = 选择台词怎样进入展示框，之后随时可在输入框里切换。
onboard-obs-hint = 通过 Spout2 把展示框送进直播画面
onboard-reopen = 之后可以在「关于」页重新打开这份引导。
about-setup-guide = 打开新手引导
playback-reduced-system-on = 系统设置：开
playback-reduced-system-off = 系统设置：关
## end wave4-onboarding

## wave5-fonts
skins-fallback-font = 替代字体
skins-fallback-font-detail = 未安装 { $family }，正以替代字体显示
## end wave5-fonts
