# 皮肤格式

皮肤是一个文件夹（或把它压缩成的 `.ntskin` 文件），根目录有一个 `skin.toml`，以及它引用的图片、字体和音效。设置 › 皮肤 › 导入皮肤 会先完整校验，通过后才安装；「复制为可编辑」会在数据目录里生成一个继承当前皮肤的副本并打开文件夹。

所有字段都有默认值（即内置皮肤「光环」的样子）。用 `base` 继承另一个皮肤时只需写不同的部分；表格逐层合并，其它值整体替换。

```toml
[meta]
id = "my.sakura"                  # a-z 0-9 . - _，最长 64
version = "1.0.0"
min_app = "0.1.0"                 # 需要的 NanaType 最低版本
base = "nanatype.halo"            # 可选：继承的皮肤
name = { zh-CN = "樱", en-US = "Sakura", ja-JP = "さくら" }
author = "你的名字"
license = "CC-BY-4.0"
preview = "preview.png"

[chrome]                          # 应用界面配色（设置、工具栏）
appearance = "light"              # light | dark
accent = "#e86a92"
background = "#fff5f8"
surface = "#ffffff"
text = "#3a2230"
muted = "#7a5a68"
border = "#f0cfdc"

[fonts.body]                      # 台词字体；name、accent 同理
file = "fonts/LXGWWenKai.ttf"     # 随皮肤分发的字体（可选）
families = ["LXGW WenKai", "Microsoft YaHei UI", "PingFang SC"]
weight = 500

[dialogue]
region = [760, 200]               # 对话区域（逻辑像素）
overscan = 18                     # 四周透明边距，防止跳动 / 描边被裁切
padding = [20, 22, 20, 14]        # 文字内边距：左 上 右 下
background = "images/bg.png"      # 可选

[dialogue.box]
color = "#ffffffee"
border_color = "#f0cfdc"
border_width = 2
radius = 18
inner_border_color = "#ffe4ee"    # 可选的内框
inner_border_width = 2
glow = "#e86a9260"                # 可选的外发光
shadow = { color = "#00000030", dy = 3, blur = 8 }
frame = { image = "images/frame.png", slice = [24, 24, 24, 24] }   # 九宫格贴图，替代上面的绘制
width_fraction = 1.0              # 框宽占区域的比例

[dialogue.text]
color = "#3a2230"
size = 28
line_height = 1.4
stroke = { color = "#ffffff", width = 2 }
shadow = { color = "#00000055", dx = 1, dy = 1, blur = 2 }

[dialogue.nameplate]
color = "#e86a92"
text_color = "#ffffff"
size = 18
radius = 10
padding = [10, 4]
offset = [16, -12]                # 相对对话框左上角；负值压在边框上
frame = { image = "images/nameplate.png", slice = [8, 8, 8, 8] }

[dialogue.portrait]               # 立绘 / 头像
visible = true
side = "left"
size = [180, 220]
image = "images/portrait.png"
variants = { smile = "images/portrait-smile.png", angry = "images/portrait-angry.png" }

[dialogue.continue]               # 等待继续时的指示符
glyph = "▼"
color = "#e86a92"
size = 16
image = "images/continue.png"     # 可选：用图片代替字符（上下浮动）

[dialogue.reveal]                 # 打字节奏
base_ms = 50                      # 每个拉丁字符
cjk_factor = 2.0                  # 中日韩字符等于几个拍子
punct = { comma_ms = 120, stop_ms = 260, ellipsis_ms = 300 }
entrance = { kind = "fade", ms = 140 }   # none | fade | pop | slide

[history]                         # 历史记录（blivechat 风格）
width = 420
height = 560
avatar = { shape = "circle", size = 40 }   # circle | square | none
name_color = "#e86a92"
name_size = 14
bubble_color = "#ffffffee"
bubble_radius = 14
bubble_frame = { image = "images/bubble.png", slice = [12, 12, 12, 12] }
gap = 10
entrance = "slide_up"             # none | slide_up | fade | slide_left

[history.text]
color = "#3a2230"
size = 18

[sounds]
typing = { files = ["sounds/key1.ogg", "sounds/key2.ogg"], volume = 0.6, pitch_jitter = 0.05, every_n = 1, min_interval_ms = 30 }
open = "sounds/open.ogg"          # 一句开始
advance = "sounds/advance.ogg"    # 按继续
se = { coin = "se/coin.ogg", boom = "se/boom.wav" }   # [se=coin]

[effects]                         # 逐字动效的默认幅度
shake_px = 1.6
wave_px = 3
jump_px = 6
rainbow_period_s = 2.4
screen_shake_px = 6
```

## 校验规则

导入时以下任何一项不满足都会拒绝整个皮肤，并显示具体字段：

- `schema` 比当前程序新，或 `min_app` 高于当前版本；
- `id` 不合法，或与内置皮肤重名；
- 路径越界（`..`、绝对路径、盘符）或文件缺失；
- 单个文件超过 64 MiB，或整个皮肤超过 256 MiB；
- 图片不是 PNG / JPEG / WebP / GIF，边长超过 8192，或九宫格切片超出图片；
- 字体无法解析；音效不是 WAV / OGG / FLAC / MP3；
- 颜色或数值不合法（如字号不在 4–400）。

清单中不认识的字段只会提示，不影响加载，以便旧版本打开新皮肤。

## 热重载

制作皮肤时，在 设置 › 皮肤 打开「文件变化时自动重载」，修改文件夹中的文件后对话框会立即更新。
