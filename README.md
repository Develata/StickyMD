# StickyMD

> 一张 Markdown 草稿。

<p align="center">
  <a href="https://github.com/Develata/StickyMD/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Develata/StickyMD?display_name=tag"></a>
  <a href="https://github.com/Develata/StickyMD/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Develata/StickyMD/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/github/license/Develata/StickyMD"></a>
  <img alt="Windows 11 x64" src="https://img.shields.io/badge/Windows-11%20x64-0078D4">
</p>

![StickyMD 在 Split 模式中同时显示 Markdown 源码和原生数学预览](assets/readme/stickymd-overview.png)

StickyMD 是用 Rust 编写的 Windows 11 桌面应用。它打开即写、自动保存，能够原生渲染
Markdown 与数学公式，也可以贴在屏幕边缘，在需要时迅速出现。

它不是知识管理系统，也不试图成为功能繁复的通用编辑器。StickyMD 只认真做好一件事：
让一张常驻桌面的 Markdown 草稿保持轻巧、可靠、随手可用。

[下载最新版本](https://github.com/Develata/StickyMD/releases/latest) ·
[发布说明](docs/release-notes/0.1.0.md) ·
[English](README.en.md) ·
[报告问题](https://github.com/Develata/StickyMD/issues/new/choose)

<details>
<summary>目录</summary>

- [30 秒开始使用](#quick-start)
- [适合这些场景](#use-cases)
- [核心功能](#features)
- [本地数据与隐私](#privacy)
- [下载与校验](#download)
- [常用快捷键](#shortcuts)
- [常见问题](#faq)
- [设计边界](#boundaries)
- [参与贡献](#contributing)

</details>

<a id="quick-start"></a>

## 30 秒开始使用

1. 从 [Releases](https://github.com/Develata/StickyMD/releases/latest) 下载
   `StickyMD-<version>-windows-x64-portable.zip`。
2. 解压到当前用户可写的目录，例如 `D:\Notes\MathScratch\`。
3. 双击 `StickyMD.exe`，直接开始输入。
4. 程序会在同一目录自动维护 `note/note.md`；停止输入约 650 ms 后自动保存。

一个目录就是一张草稿。需要另一张时，复制整个 StickyMD 目录即可。

### 可选：跟随 Windows 登录自动打开

1. 为 `StickyMD.exe` 创建一个快捷方式；Windows 11 中可以在右键菜单的“显示更多选项”中找到
   “创建快捷方式”。
2. 按 `Win+R`，输入 `shell:startup` 并回车。
3. 把快捷方式移动到打开的“启动”文件夹。下次登录 Windows 时，StickyMD 就会自动打开。

这是完全由 Windows 管理的可选设置，StickyMD 不会自行写入注册表或注册开机任务。移动程序目录
后需要重新创建快捷方式。本地 Release 测试中，常见视图稳定常驻约 20 MB，适合作为轻量桌面
草稿；实际占用会随视图、文档、图片和系统环境变化。

<a id="use-cases"></a>

## 适合这些场景

- 临时记录带数学公式的推导、想法和片段。
- 在桌面边缘常驻一张随手可用的 Markdown 草稿。
- 一边编辑源码，一边查看原生 Markdown 与公式预览。
- 为研究、教学或临时任务分别保留互不影响的 portable 草稿目录。

<a id="features"></a>

## 核心功能

### 原生数学公式渲染

- 支持 `$...$`、`$$...$$`、`\(...\)`、`\[...\]` 四种分隔符。
- 使用 RaTeX 原生排版与绘制，不依赖浏览器、JavaScript 或 WebView。
- 覆盖常用分数、根式、上下标、积分、求和、矩阵和 cases 等 KaTeX-compatible 语法。
- 公式有误时保留原文并显示错误提示，不修改 Markdown。
- 许多 AI 生成的数学内容倾向使用 `\(...\)` / `\[...\]`。顶部 `$` 按钮可将确认的公式一键
  转换成 Markdown 常用的 `$...$` / `$$...$$`，且一次撤销即可恢复；代码块和普通讨论文本不受影响。

StickyMD 支持的是 **RaTeX/KaTeX-compatible 数学语法**，不是 TeX Live 或完整 LaTeX
文档编译环境。

### Source、Preview 与 Split

- Source 直接编辑 Markdown 原文。
- Preview 原生渲染 CommonMark、GFM、表格、任务列表、代码、链接、图片和公式。
- Split 固定 50/50，默认按 Markdown 语义位置同步滚动；同步可以独立关闭。
- 预览文字按实际 shaping geometry 精确选择和复制，Raw HTML 只显示原文而不执行。
- 三种视图共用 50–300% 内容缩放。

### 字体与中英文混排

- 中文/CJK 首选 `仿宋_GB2312`，并依次尝试 `FangSong_GB2312`、`仿宋`、`FangSong`、
  `Microsoft YaHei`。
- Latin 首选 `Times New Roman`，其次尝试 `Georgia`。
- 上述字体均不可用时，由文本引擎选择电脑上可用的系统 fallback；代码和数学分别使用系统
  monospace 与 RaTeX 内置数学字体。

v0.1.0 暂不提供运行时字体设置。需要自行构建定制版本时，请编辑
[`crates/stickymd-render/src/source/fonts.rs`](crates/stickymd-render/src/source/fonts.rs) 中的
`CJK_CANDIDATES` 和 `LATIN_CANDIDATES`，把已经安装的 Windows 字体 family name 放到候选列表
首位，然后重新构建；`config.toml` 目前不能修改字体。

### 输入、查找与桌面交互

- 支持微软拼音与微信输入法；一次 commit 对应一次撤销。
- `Ctrl+F` 纯文本查找，`Ctrl+H` 展开替换；支持大小写开关，不支持正则。
- 左、右、上三边吸附，失焦自动收起，触碰 3 DIP 感应边即可展开。
- 输入和 IME composition 期间不会自动收起；置顶与自动隐藏彼此独立。
- 点击关闭按钮隐藏到托盘，从托盘菜单真正退出。

![StickyMD 从桌面顶部感应边展开并自动收起](assets/readme/stickymd-top-edge.gif)

> 动图只演示顶部；实际支持顶部、左侧和右侧三边停靠与自动隐藏。

### Portable 与手动多开

- 复制整个 StickyMD 目录即可创建另一张独立草稿，并可手动同时运行多个目录中的实例。
- 每个目录分别维护自己的 `note/note.md`、配置和图片，彼此不共享运行时状态。
- 同一个 canonical 目录仍然只允许一个实例；再次启动只会唤醒已有窗口，防止两个进程同时写入。

### 图片、导出与可靠保存

- 可粘贴截图及 PNG、JPEG、WebP、GIF；managed 图片按内容哈希命名和去重。
- 用户手工放入 `note/images/` 的文件不会被自动删除。
- `Ctrl+Shift+S` 导出 Markdown 及实际引用的本地图片，不切换当前草稿。
- 保存使用同目录临时文件和原子替换；外部修改、冲突和异常恢复都有明确处理路径。

### 面向低资源占用设计

没有浏览器运行时、数据库、网络客户端或通用异步运行时；空闲时不持续重绘，Undo、公式和
图片缓存均有明确上限。

本机五轮独立 Release 进程在空闲 30 秒后测得：Source、Preview、Split 的 Private Working Set
中位数分别为 12.98、15.50、20.89 MiB，最大值分别为 13.03、15.56、23.58 MiB，空闲 CPU p95
为 0–0.0027%。这些数字是可复现的本机证据，不是所有电脑上的固定保证；资源仍会随文档、公式、
图片和 Windows 环境变化。完整方法与数据见
[Release 内存归因报告](docs/report/phase-14-memory-attribution.md)，目标和 hard gate 见
[性能与可靠性合同](docs/plan/10_performance_reliability.md)。

<a id="privacy"></a>

## 本地数据与隐私

```text
MathScratch/
├─ StickyMD.exe
└─ note/
   ├─ note.md
   ├─ config.toml
   ├─ images/
   └─ .trash/
```

- 所有草稿和图片都保存在当前 Program Directory。
- 不写 AppData 或注册表，不需要账户。
- 没有云同步、遥测、广告、自动更新或 runtime 网络请求。
- 移动、复制或备份时保留整个目录即可。

<a id="download"></a>

## 下载与校验

系统要求：**Windows 11 x64**，以及一个允许当前用户写入的普通目录。不要放在
`Program Files`，也不需要管理员权限。

从依赖角度，公开 portable ZIP 不要求安装 Rust、Visual Studio、C/C++ 编译器、Windows SDK
或独立 Visual C++ Redistributable。Release 静态链接 MSVC CRT，并通过普通与延迟加载 PE import
检查。`v0.1.0` 尚未在全新的 Windows 11 VM 中完成独立启动验收；这是已披露的验证缺口，而不是
额外运行库要求。

`v0.1.0` 没有 Authenticode 签名，Windows 可能显示 SmartScreen 或信誉提示。不要因此关闭
Defender 或 SmartScreen；请下载同一 Release 的
[`SHA256SUMS.txt`](https://github.com/Develata/StickyMD/releases/latest/download/SHA256SUMS.txt) 并核对：

```powershell
Get-FileHash .\StickyMD-0.1.0-windows-x64-portable.zip -Algorithm SHA256
```

更多信息见[正式发布说明](docs/release-notes/0.1.0.md)和[安全策略](SECURITY.md)。

<a id="shortcuts"></a>

## 常用快捷键

| 快捷键 | 功能 |
| --- | --- |
| `Ctrl+S` | 立即保存 |
| `Ctrl+Z` / `Ctrl+Y` | 撤销 / 重做 |
| `Ctrl+F` | 打开或关闭查找 |
| `Ctrl+H` | 展开查找与替换 |
| `Ctrl++` / `Ctrl+-` / `Ctrl+0` | 放大 / 缩小 / 恢复 100% |
| `Ctrl+滚轮` | 细粒度内容缩放 |
| `Ctrl+Shift+S` | 导出 Markdown 与本地图片 |
| `Esc` | 收起已贴边窗口 |

`Ctrl+Insert`、`Shift+Delete`、`Shift+Insert` 也分别对应复制、剪切和粘贴。

<a id="faq"></a>

## 常见问题

**为什么只能编辑一份 `note.md`？**

“一个目录一张草稿”是 StickyMD 的产品模型，不是功能缺失；它避免文件管理界面和隐藏状态。

**如何创建另一张草稿？**

复制整个 StickyMD 目录。不同目录可以同时运行，同一目录只允许一个实例。

**如何备份或移动？**

退出 StickyMD 后复制整个目录；`note/` 中包含草稿、配置和本地图片。

**需要安装 Rust 或 Visual C++ 吗？**

普通用户不需要。它们只用于从源码构建；Clean VM 实测缺口见上面的“下载与校验”。

**为什么 Windows 显示 SmartScreen？**

`v0.1.0` 没有 Authenticode 签名。请核对官方 Release 的 SHA-256，不要关闭系统防护。

**支持完整 LaTeX 吗？**

不支持。公式范围是 RaTeX/KaTeX-compatible 数学语法，不包含宏包、`\usepackage` 或 TeX 执行器。

<a id="boundaries"></a>

## 设计边界

StickyMD 的“精简”指功能边界窄、日常负担低，不代表数据安全和输入正确性可以简化。

首版明确不做：

- 新建/打开文件、最近文件、文件树、多标签页或 Vault。
- 双向链接、Graph、标签系统或知识库管理。
- WYSIWYG、插件系统、LSP、语法高亮或代码执行。
- 云同步、账户、AI、遥测、远程图片下载或自动更新。
- WebView、Electron、Tauri 或 JavaScript runtime。

<details>
<summary>工程实现、源码构建与验证</summary>

- Rust workspace；Windows 平台调用集中在受控 adapter 中。
- Markdown 语义由 Comrak 定义，数学解析与布局由 RaTeX 定义。
- `DocumentState` 是运行时文本的唯一权威；编辑器、Preview 和磁盘文件都是投影或外部事实。
- 平台无关核心禁止 `unsafe`，产品不包含 runtime 网络客户端。
- 自动化 verdict 由 `tools/stickymd-smoke` Rust CLI 持有。

从源码构建需要 Windows 11 x64、MSVC C++/Windows SDK 构建工具，以及
`rust-toolchain.toml` 固定的 Rust 工具链：

```powershell
cargo build --workspace --release --locked
./tools/smoke/all.ps1 -Ci
```

详细工程合同见 [`docs/plan/`](docs/plan/)，验收合同见
[`docs/acceptance-cases/`](docs/acceptance-cases/)。

</details>

<a id="contributing"></a>

## 参与贡献

我们尤其欢迎准确、可复现的问题报告。Pull Request 也欢迎，但代码贡献请先建立 Issue，与维护者
确认问题、范围和方案后再开始实现。

- [报告 Bug 或提出建议](https://github.com/Develata/StickyMD/issues/new/choose)
- [完整贡献流程](CONTRIBUTING.md)
- [安全问题私密报告](SECURITY.md)
- [用户可见行为](docs/features/00_v1_product_behavior.md)
- [第三方声明](THIRD_PARTY_NOTICES.md)

## 许可证

StickyMD 使用 [MIT License](LICENSE)。嵌入的 KaTeX-compatible 字体使用 SIL Open Font
License 1.1，完整声明随发布包分发并记录在 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

## 社区

[LINUX DO](https://linux.do/) — 感谢社区为开源项目提供交流与展示空间；我也从这里的分享与讨论中
学到了许多知识和实践经验。
