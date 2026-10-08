# 台词标记

标记写在输入框的编辑区里，工具栏按钮会替你插入。解析永不失败：认不出的标签原样显示为文字，并在状态栏提示。`[[` 表示一个字面的 `[`。

## 样式（成对使用）

| 标记 | 效果 |
| --- | --- |
| `[b]…[/b]` `[i]…[/i]` `[u]…[/u]` `[s]…[/s]` | 粗体、斜体、下划线、删除线；`[b=0]` 取消皮肤默认的粗体 |
| `[color=#e86a92]…[/color]` | 文字颜色，`#rgb` `#rrggbb` `#rrggbbaa` |
| `[size=1.5]…[/size]` `[size=32px]…[/size]` | 字号：皮肤字号的倍数，或像素 |
| `[font=accent]…[/font]` `[font="Noto Serif SC"]…[/font]` | 皮肤字体槽（body / name / accent）或指定字体族 |
| `[stroke=#ffffff,2]…[/stroke]` | 描边：颜色，宽度 |
| `[shadow=#000000aa,2,2,4]…[/shadow]` | 阴影：颜色，x，y，模糊 |
| `[ruby=かんじ]漢字[/ruby]` | 注音 |

## 逐字动效（成对使用）

| 标记 | 效果 |
| --- | --- |
| `[shake]` `[wave]` `[jump]` `[rainbow]` | 抖动、波浪、跳跃、彩虹；可带 `amp=` 与 `speed=`（皮肤默认值的倍数） |
| `[fade]` `[pop]` `[slide]` | 出场：淡入、弹出、上浮；`[pop=120]` 指定毫秒 |

## 节奏

| 标记 | 效果 |
| --- | --- |
| `[speed=0.5]…[/speed]` | 局部变速：倍数（0.5 为两倍速） |
| `[speed=40ms]…[/speed]` | 每字固定毫秒 |
| `[instant]…[/instant]` | 整段瞬间出现 |
| `[wait=500]` | 停顿 500 毫秒 |
| `[pause]` | 停下，等待继续键 |
| `[pause auto=2000]` | 停下，2 秒后自动继续 |
| `[page]` | 等待继续，然后清空对话框接着打 |

## 音效、表情与画面

| 标记 | 效果 |
| --- | --- |
| `[se=coin]` `[se=coin vol=0.6]` | 播放皮肤中名为 `coin` 的音效 |
| `[expr=smile]` | 触发表情：切换同名立绘差分，并发往 VTube Studio / OSC / WebSocket |
| `[portrait=happy]` | 只切换立绘差分 |
| `[screenshake=6,300]` | 对话框震动：幅度，毫秒 |
| `[sticker=cats/hello h=1.6]` | 插入贴纸库 `cats` 中的 `hello`，高度为 1.6 倍字号 |
| `[emote=wave]` | 插入皮肤自带表情 `emotes/wave` |
| `[img="file:C:/pic.png" alt="图"]` | 插入图片 |

## 整句设置

写在最开头：

```text
[@auto=1500 after=fade delay=800 per=20]
```

- `auto`：播放完后多少毫秒自动推进到下一句（排队模式下）。
- `after`：`hold` 保持、`fade` 渐隐（`delay` 停留毫秒，`per` 每字额外毫秒）、`clear` 直接清除。

## 快捷短语模板

快捷短语里可以用 `{{name}}`（角色名）和 `{{time}}`（当前时间）。
