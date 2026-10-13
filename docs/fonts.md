# 内置皮肤的字体

内置皮肤只写系统字体名时，没装那些字体的电脑会用普通界面字体代替：像素、水墨这几套就失去了味道。这里说明怎样把开源字体编进程序，以及建议带哪些。

> **状态**：机制已就绪，仓库里还没有任何字体文件。下表的来源与许可是按维护者的已有知识整理的，**未联网核实**；标 ⚠ 的项目在下载前必须到官方仓库确认。

## 机制

- 字体放在 `crates/theme/fonts/<名字>.ttf`（或 `.otf`），旁边必须有同名许可 `<名字>.LICENSE.txt`（上游的 `OFL.txt` 原样复制，含版权行）。缺许可时编译失败。
- 皮肤写 `file = "builtin:fonts/<名字>.ttf"`。用户皮肤也能引用。路径不能离开 `crates/theme/`。
- `build.rs` 扫描该目录，打开 Cargo 特性 `builtin-fonts`（应用默认打开）时用 `include_bytes!` 嵌入字体与许可，否则什么都不嵌入。没有字体文件时两种情况都与以前完全一样。
- 选中皮肤时才向 NanaUI 注册（`app/src/skin_fonts.rs`）；正文、名牌、装饰三个角色或几套皮肤共用同一个字体时只注册一次，切到不用它的皮肤时注销。
- 本次构建没带的字体不报错，直接回落到 `families`；`fonts/` 里根本没有的名字给一条警告（多半是拼错）。
- 皮肤库会给 `families` 第一个字体既没安装、也没随附的皮肤标上「替代字体」。

### 体积开关

| 方式 | 作用 |
| --- | --- |
| `cargo build --no-default-features -p nanatype` | 不嵌入任何内置字体 |
| `NANATYPE_BUILTIN_FONTS_BUDGET_KB=<KiB>` | 体积上限，默认 12288（12 MiB）。超出时编译失败 |

嵌入是逐字节的，程序增大的体积就是 `fonts/` 里字体文件的总大小（不压缩；安装包的压缩会省一部分）。

## 逐套建议

| 皮肤 | 首选字体 | 官方来源 | 许可（SPDX） | 原始大小（约） | 需要的字重 / 子集 | 建议 |
| --- | --- | --- | --- | --- | --- | --- |
| `pixel`、`rpg` | Fusion Pixel Font 12px Proportional SC | <https://github.com/TakWolf/fusion-pixel-font>（Releases） | OFL-1.1 ⚠ | 单个 zh_hans `.ttf` 约 3–6 MB ⚠ | 只要 400；子集到 GB2312 约 0.8–1.5 MB ⚠ | **带**：没有可用的系统替代 |
| `ink` | LXGW WenKai（霞鹜文楷）Regular | <https://github.com/lxgw/LxgwWenKai>（Releases） | OFL-1.1 ⚠ | 约 20–25 MB ⚠ | 只带 Regular，名牌 700 用合成粗体（文楷没有 Bold，只有 Light / Regular / Medium ⚠）；子集到 3500 常用字约 3–4 MB ⚠ | **带**（子集） |
| `parchment` | Noto Serif SC | <https://github.com/notofonts/noto-cjk>（`Serif/`），或 google/fonts `ofl/notoserifsc` | OFL-1.1 | 每个字重的 SubsetOTF 约 10–15 MB ⚠ | 需要 600 与 700：两个静态字重或一个可变字体；子集后每个约 1.5–3 MB ⚠ | 暂不带：SimSun / 宋体是可以接受的回落 |
| `terminal` | Cascadia Mono | <https://github.com/microsoft/cascadia-code>（Releases） | OFL-1.1，含保留字体名 “Cascadia Code” ⚠ | 静态 Regular 约 0.3–0.6 MB ⚠ | 只要 400；**不要子集**（保留字体名的修改版须改名） | 不带：Windows 11 自带 ⚠，Consolas / Menlo 回落已足够 |
| `echo_*` | Source Han Sans SC | <https://github.com/adobe-fonts/source-han-sans> | OFL-1.1 | — | — | 不下载：NanaUI 已内置同一设计的 **Noto Sans SC**（400/500/600/700），把它放进 `families` 即可，零字节 |

### 候选但不建议

| 字体 | 来源 | 许可 | 结论 |
| --- | --- | --- | --- |
| Zpix（最像素） | <https://github.com/SolidZORO/zpix-pixel-font> | **非 OFL**：自定义许可，按记忆个人免费、商业使用（含随软件分发）需授权 ⚠ | **排除**。即使核实后可免费分发，也有 Fusion Pixel 可替代 |
| DotGothic16 | <https://github.com/fontworks-fonts/DotGothic16>，或 google/fonts `ofl/dotgothic16` | OFL-1.1 | 日文字形（汉字按日本写法），不适合简中台词；可作为日语界面的像素备选，约 2 MB ⚠ |
| Sarasa Mono SC（更纱黑体） | <https://github.com/be5invis/Sarasa-Gothic> | OFL-1.1 | 单个字重约 20 MB 以上 ⚠，太大；终端皮肤的中文回落交给系统字体 |

### 最小集合

1. **Fusion Pixel 12px Proportional SC**（GB2312 子集）→ `pixel`、`rpg` 共用一份
2. **LXGW WenKai Regular**（3500 常用字子集）→ `ink`
3. `echo_*`：`families` 加上 `"Noto Sans SC"`（不需要下载）

合计约 4–5.5 MB ⚠，在 12 MiB 预算内。`parchment`、`terminal` 先靠系统字体回落，以后有需要再加。

## 子集

用 fontTools（`pip install fonttools`）。保留全部 name 记录：OFL 要求版权与许可声明随字体保留，家族名也要原样留着，皮肤才对得上。

GB2312 一二级汉字（6763 字）加 ASCII 与常用标点，离线就能生成字表：

```sh
python -c "import sys; sys.stdout.reconfigure(encoding='utf-8'); print(''.join(bytes([h, l]).decode('gb2312', 'ignore') for h in range(0xB0, 0xF8) for l in range(0xA1, 0xFF)))" > gb2312.txt
```

```sh
pyftsubset fusion-pixel-12px-proportional-zh_hans.ttf \
  --text-file=gb2312.txt \
  --unicodes="U+0020-007E,U+00A0-00FF,U+2010-206F,U+2190-21FF,U+25A0-25FF,U+3000-303F,U+FF00-FFEF" \
  --layout-features='*' --name-IDs='*' --name-languages='*' --name-legacy \
  --notdef-outline --no-hinting \
  --output-file=crates/theme/fonts/FusionPixel12pxProportionalSC.ttf
```

文楷换成 3500 字的《通用规范汉字表》一级字表（需自备字表文件 `common3500.txt`）可以再小一半；超出子集的字会逐字回落到系统字体，台词里偶尔夹一个宋体/黑体字，这是体积换来的代价。

**保留字体名（RFN）**：OFL 第 3 条规定，字体声明了保留字体名时，修改版（子集也算）不能再用这个名字。子集前先看上游 `OFL.txt` 第一段的版权行有没有 “with Reserved Font Name …”：

- 没有：子集后可沿用原名（Fusion Pixel、文楷需要核实 ⚠）。
- 有：要么不子集，要么确认保留名不在家族名里，否则改家族名并同步修改皮肤的 `families`。例：NanaUI 带的 Noto Sans SC 2.004 在 name 表里写着 “with Reserved Font Name 'Source'”（已在本机字体文件里核实），保留的是 “Source”，“Noto Sans SC” 不受影响；Cascadia 保留 “Cascadia Code” ⚠。

## 随附的署名与许可

OFL 1.1 第 2 条要求每份副本都带上版权声明与许可全文。本仓库的做法：

- `crates/theme/fonts/<名字>.LICENSE.txt`：上游 `OFL.txt` 原样复制（构建强制要求），随字体一起编进程序（`BuiltinFont::license`）。
- **发布前还缺一步**：用户要能看到这些文字。可选方案是安装包把 `crates/theme/fonts/*.LICENSE.txt` 拷到 `licenses/fonts/`，或者「关于」页加一个字体许可列表。两者择一，由维护者决定。

关于页 / 第三方声明里可以用这样的署名（版权行以上游 `OFL.txt` 为准 ⚠）：

```text
Fusion Pixel Font — Copyright (c) TakWolf. SIL Open Font License 1.1.
LXGW WenKai / 霞鹜文楷 — Copyright (c) LXGW. SIL Open Font License 1.1.
Noto Sans SC (NanaUI) — Copyright 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source'. SIL Open Font License 1.1.
```

## 加入一个字体的步骤

1. 从官方 Release 下载，核对许可（OFL-1.1 或同样允许随软件分发的许可），记下版本与 SHA-256。
2. 按上面的方法子集，放进 `crates/theme/fonts/`，旁边放 `<名字>.LICENSE.txt`。
3. 皮肤里写 `file = "builtin:fonts/<名字>.ttf"`，`families` 保持不变作为回落。
4. `cargo test -p nanatype-theme`：测试会检查每个嵌入的字体能解析、带许可，每个内置皮肤的 `builtin:` 引用都在 `fonts/` 里。
