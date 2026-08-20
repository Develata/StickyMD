# StickyMD v1.0 主规格与 Agent 实施蓝图

**状态：v1 需求冻结**  
**工作名：StickyMD**  
**目标平台：Windows 11 x64**  
**许可证：MIT**  
**文档日期：2026-08-18**

> StickyMD 不是 Obsidian，不是 Typora，不是知识管理工具，也不是通用 Markdown 编辑器。  
> 它是一张极度轻量、原生、常驻桌面的 Markdown 草稿纸：打开即写，自动保存，公式可靠，贴边即隐，需要时迅速出现。

本文中的：

- **必须**表示 v1 发布所需的硬性条件。
    
- **应该**表示除非有明确技术障碍，否则必须实现。
    
- **可以**表示非阻塞性实现选择。
    
- 未写入本文的功能，默认不属于 v1。
    

---

# 1. 产品定义

## 1.1 核心使用场景

用户将 `StickyMD.exe` 解压到某个可写目录：

```
D:\Notes\MathScratch\
    StickyMD.exe
```

第一次运行后，程序自动创建：

```
D:\Notes\MathScratch\
├─ StickyMD.exe
└─ note\
   ├─ note.md
   ├─ config.toml
   ├─ images\
   └─ .trash\
```

程序永远编辑：

```
./note/note.md
```

它不提供“新建文档”“打开文档”“最近文件”“文件树”或多标签页。

用户需要另一张独立便签时，只需复制整个文件夹：

```
D:\Notes\Research\
D:\Notes\Teaching\
D:\Notes\Temporary\
```

不同目录中的程序可以同时运行；同一目录只能运行一个实例。

## 1.2 产品原则

StickyMD 的设计优先级固定如下：

1. 数据可靠性。
    
2. 中文输入法正确性。
    
3. 输入与窗口响应速度。
    
4. 低内存和低空闲 CPU。
    
5. Markdown 与数学公式渲染正确性。
    
6. 窗口贴边交互。
    
7. 视觉细节。
    
8. 功能数量。
    

任何新功能如果显著损害前六项，应被拒绝。

## 1.3 参考项目的取舍

StickyMD 参考：

- Tinta 的原生 Windows 渲染、低资源设计和无浏览器引擎路线。
    
- PaperTodo 的桌面纸片、屏幕边缘停靠和鼠标悬停展开交互。
    

Tinta 当前采用原生 C++、Win32、Direct2D/DirectWrite，并明确不使用 Electron；PaperTodo 则实现了贴边胶囊、悬停展开和多显示器行为。StickyMD 只借鉴产品和架构思想，不复制未经确认可复用的实现。citeturn213259search0turn213259search1turn213259search2

---

# 2. v1 功能规格

## 2.1 窗口模式

程序只有一个主窗口，提供三个互斥视图状态：

### 源码模式

```
┌────────────────────────────┐
│ # 标题                     │
│                            │
│ 这是 **粗体**。            │
│                            │
│ $x^2+y^2=1$                │
└────────────────────────────┘
```

用户看到并编辑原始 Markdown。

### 预览模式

```
┌────────────────────────────┐
│ 标题                       │
│                            │
│ 这是 粗体。                │
│                            │
│      x² + y² = 1           │
└────────────────────────────┘
```

预览只读，但必须支持：

- 鼠标选择文字。
    
- `Ctrl+C`。
    
- 滚动。
    
- 点击允许的链接。
    
- 公式错误提示。
    
- 本地图片显示。
    

### 分栏模式

```
┌──────────────────┬──────────────────┐
│ Markdown 源码    │ 渲染预览         │
│                  │                  │
│ $x^2$            │       x²         │
└──────────────────┴──────────────────┘
```

要求：

- 固定 50/50。
    
- 分隔线不可拖动。
    
- 源码与预览分别保存自己的滚动位置。
    
- 不做源码—预览滚动同步。
    
- 进入分栏模式时，如果窗口过窄，可以向屏幕内侧临时扩展。
    
- 离开分栏模式时恢复先前宽度。
    
- 在可用屏幕空间不足时，允许保持较窄分栏，但每栏不得窄于 240 DIP。
    

## 2.2 预览更新规则

Markdown 预览不是逐键实时刷新。

每次文本修改：

```
preview_dirty = true
preview_generation += 1
```

分栏已经打开时：

1. 用户停止输入。
    
2. 等待固定 1000 ms。
    
3. 后台解析当前最新文本。
    
4. 构建新预览。
    
5. 只有 generation 仍为最新时才提交结果。
    

连续输入会不断重置 1000 ms 计时器。

切换到纯预览模式时：

- 必须立即触发刷新。
    
- 可以暂时显示上一次预览。
    
- 新预览生成后原子替换。
    
- 过期后台结果必须丢弃。
    

## 2.3 默认窗口参数

建议冻结为：

|参数|默认值|
|---|---|
|默认模式|源码|
|默认主题|Light|
|默认透明度|96%|
|默认置顶|关闭|
|默认尺寸|520 × 680 DIP|
|最小尺寸|360 × 240 DIP|
|分栏推荐宽度|900 DIP|
|顶部控制区高度|34 DIP|
|屏幕吸附距离|12 DIP|
|隐藏后保留边缘|3 DIP|
|展开动画|140 ms|
|收起动画|140 ms|
|hover 展开延迟|100 ms|
|失焦收起延迟|700 ms|
|未聚焦 hover 离开收起|500 ms|
|自动保存 debounce|650 ms|
|分栏预览 debounce|1000 ms|

所有布局长度使用 DIP，由当前显示器 DPI 转换为物理像素。

## 2.4 窗口控制

窗口顶部只保留少量图标控件：

- 源码模式。
    
- 分栏模式。
    
- 预览模式。
    
- Always on top。
    
- 主题选择。
    
- 透明度。
    
- 手动收起。
    
- 关闭到托盘。
    

按钮只在以下状态明显显示：

- 鼠标位于窗口内。
    
- 窗口获得焦点。
    
- 用户正在与顶部控件交互。
    

非交互状态下，控件应淡化，但不得影响可发现性。

## 2.5 主题

主题控件为三态滑块：

```
☀  Light    ▣  System    ☾  Dark
```

行为：

- Light 是首次运行默认值。
    
- System 跟随 Windows 应用主题。
    
- 当 Windows 主题在运行时变化，System 模式立即响应。
    
- Dark 是强制深色。
    
- 主题写入 `config.toml`。
    
- 不提供主题编辑器、自定义颜色或主题文件。
    

## 2.6 透明度

透明度作用于整个窗口，包括：

- 背景。
    
- 文字。
    
- 公式。
    
- 图片。
    
- 控件。
    
- 阴影。
    

控件包括：

- 70–100 的 slider。
    
- 步长为 1。
    
- 一个整数输入框。
    
- 默认值为 96。
    

输入规则：

- 小于 70，clamp 为 70。
    
- 大于 100，clamp 为 100。
    
- 非整数不提交。
    
- 拖动时实时预览。
    
- 只有松开滑块、按 Enter 或输入框失焦时才写配置。
    
- 不因拖动每一步都写磁盘。
    

Windows 上的整窗透明度通过 `WS_EX_LAYERED` 与 `SetLayeredWindowAttributes` 实现；alpha 值为 0–255。Windows 11 的窗口圆角通过 DWM 窗口属性实现。citeturn250214search0turn250214search2turn250214search4

## 2.7 固定视觉效果

以下效果由程序固定，不提供用户配置：

- 小幅圆角。
    
- 轻微窗口阴影。
    
- 140 ms ease-out 滑入/滑出动画。
    
- Light/Dark 对应的固定纸张背景。
    
- 控件 hover 动画。
    
- 1 DIP 分栏分隔线。
    
- 适度内边距。
    

不得加入：

- Acrylic。
    
- Mica。
    
- 毛玻璃。
    
- 动态背景。
    
- 主题市场。
    
- 背景图片。
    

---

# 3. 屏幕贴边与自动隐藏

## 3.1 支持的边缘

v1 支持：

- 左边缘。
    
- 右边缘。
    
- 上边缘。
    

不支持底部边缘，以避免任务栏和自动隐藏任务栏行为冲突。

## 3.2 吸附

用户拖动窗口并松开时，如果窗口边缘距当前显示器工作区边缘不超过 12 DIP：

```
Floating → DockedExpanded(edge)
```

吸附后：

- 左边缘窗口向左缩入。
    
- 右边缘窗口向右缩入。
    
- 上边缘窗口向上缩入。
    
- 保留 3 DIP 可见感应条。
    
- 顶部吸附时，感应条宽度与窗口宽度一致。
    
- 左右吸附时，感应条高度与窗口高度一致。
    

用户把已吸附窗口向内拖离边缘超过 16 DIP 时：

```
DockedExpanded → Floating
```

## 3.3 自动隐藏规则

窗口获得键盘焦点后：

- 禁止自动收起。
    
- IME composition 期间绝不自动收起。
    
- 鼠标临时离开窗口也不触发收起。
    

窗口失去焦点后：

```
等待 700 ms → 收起
```

前提是：

- 没有重新获得焦点。
    
- 没有正在拖动。
    
- 没有打开内部弹出控件。
    
- 没有冲突或恢复提示需要处理。
    

用户把鼠标移入 3 DIP 感应区：

```
等待 100 ms → 展开
```

如果只是 hover 展开但没有点击获得焦点：

```
鼠标离开 → 等待 500 ms → 收起
```

以下操作无条件立即收起：

- `Esc`。
    
- 点击手动收起按钮。
    

## 3.4 多显示器

多显示器属于 v1 一级需求。

必须支持：

- 左右排列。
    
- 上下排列。
    
- 负坐标显示器。
    
- 混合 DPI。
    
- 主显示器切换。
    
- 运行中拔掉显示器。
    
- 睡眠/唤醒后显示器重枚举。
    
- 远程桌面导致的显示配置变化。
    

配置不只保存绝对坐标，而应保存：

```
monitor_identity
dock_edge
offset_ratio
width_dip
height_dip
floating_x_ratio
floating_y_ratio
```

监视器 identity 优先使用 Windows 显示配置设备路径的稳定哈希。Windows 的 `QueryDisplayConfig` 和 `DisplayConfigGetDeviceInfo` 可以枚举显示路径并获取显示设备信息。citeturn250214search17

如果已保存显示器不存在：

1. 恢复到主显示器。
    
2. 保证窗口完全位于工作区内。
    
3. 保留原尺寸，除非尺寸大于新工作区。
    
4. 若原来处于 dock 状态，在主屏幕相同边缘恢复。
    
5. 不允许窗口留在不可见坐标。
    

---

# 4. 托盘与程序生命周期

## 4.1 托盘菜单

系统托盘只包含：

```
显示/隐藏
置顶
退出
```

不添加：

- 新建。
    
- 打开。
    
- 最近文件。
    
- 设置。
    
- 关于。
    
- 更新。
    
- 同步。
    

“关于”和许可证信息可以放在 README，不占用托盘菜单。

## 4.2 关闭行为

纸张窗口上的关闭按钮：

```
Close → Hide to tray
```

不得终止进程。

真正退出只能从托盘菜单选择：

```
退出
```

退出前必须：

1. 完成立即保存。
    
2. 等待待处理图片事务。
    
3. 完成安全的 `.trash` 清理。
    
4. 保存配置。
    
5. 释放单实例 mutex。
    
6. 删除当前进程创建且已确认无用的临时文件。
    

保存失败时：

- 显示明确错误。
    
- 保持程序运行。
    
- 不静默退出。
    
- 不丢弃内存中的文本。
    

---

# 5. 单实例模型

## 5.1 实例身份

实例身份不是进程名或 EXE 文件名，而是：

```
canonical_program_directory
```

处理步骤：

1. 获取 `current_exe()`。
    
2. 取得父目录。
    
3. 解析 `.`、`..`。
    
4. 解析 Windows junction/symlink/reparse point。
    
5. 统一 Windows 大小写语义。
    
6. 对最终目录路径做 SHA-256。
    
7. 用哈希构造本地命名对象。
    

例如：

```
Local\StickyMD.Mutex.<dir-hash>
Local\StickyMD.Show.<dir-hash>
```

## 5.2 行为

同一 canonical 目录：

- 第一个实例持有 named mutex。
    
- 第二个实例发现 mutex 已存在。
    
- 第二个实例通过 named event 或轻量 IPC 通知第一个实例显示并激活。
    
- 第二个实例立即退出。
    

不同目录：

- 哈希不同。
    
- 可以同时运行。
    
- 各自读写自己的 `./note/note.md`。
    

Windows named mutex 适合进行进程间互斥，named event 可用于通知已运行实例。citeturn628483search5turn628483search6turn628483search8

---

# 6. Portable 文件模型

## 6.1 运行时目录

```
<program-dir>\
├─ StickyMD.exe
└─ note\
   ├─ note.md
   ├─ config.toml
   ├─ images\
   │  ├─ stickymd-0123456789abcdef0123.png
   │  └─ user-supplied-image.png
   ├─ .trash\
   │  └─ stickymd-abcdef0123456789abcd.jpg
   ├─ note.md.tmp
   └─ crash.log
```

其中：

- `note.md` 是唯一工作文档。
    
- `config.toml` 是唯一配置文件。
    
- `images/` 存放粘贴图片和用户手工放入的图片。
    
- `.trash/` 存放等待物理删除的 managed 图片。
    
- `note.md.tmp` 只在写入事务中存在。
    
- `crash.log` 只在发生 panic 或关键故障时创建。
    

## 6.2 目录可写检查

启动时，在创建 UI 之前：

1. 创建 `./note/`。
    
2. 在目录中创建测试文件。
    
3. 写入少量字节。
    
4. flush。
    
5. 删除测试文件。
    

失败时显示：

> 当前目录不可写，请将程序移动到有写权限的文件夹。

然后退出。

不得偷偷 fallback 到：

- `%APPDATA%`
    
- `%LOCALAPPDATA%`
    
- Documents
    
- 注册表
    
- 用户配置目录
    

否则会破坏“目录就是便签身份”的模型。

## 6.3 文本编码

磁盘格式：

- 保存为 UTF-8 without BOM。
    
- 读取时兼容 UTF-8 BOM。
    
- 非 UTF-8 文件不允许静默覆盖。
    
- 首次创建使用 CRLF。
    
- 已存在文件保存时保留原有主要换行风格。
    
- 混合换行文件按占多数的换行风格统一保存；数量相同时使用 CRLF。
    

内部文本统一使用：

```
UTF-8 + \n
```

保存时再转换为所记录的行尾风格。

---

# 7. Markdown 方言

## 7.1 Parser

使用 **Comrak**。

当前 Comrak 提供 CommonMark 与 GitHub Flavored Markdown 支持、AST，以及 dollar math 和 LaTeX delimiter math 扩展。当前基线版本为 0.54 系列。citeturn641255search0turn467913search0turn467913search3turn467913search4

StickyMD 不自行实现 Markdown parser。

## 7.2 正式方言

StickyMD v1 方言定义为：

```
CommonMark 0.31.2
+ GitHub Flavored Markdown
+ Comrak math_dollars
+ Comrak math_latex
```

启用：

- 标题。
    
- 段落。
    
- 强调。
    
- 粗体。
    
- 删除线。
    
- 引用。
    
- 有序列表。
    
- 无序列表。
    
- task list。
    
- inline code。
    
- fenced code block。
    
- indented code block。
    
- 链接。
    
- 自动链接。
    
- 图片。
    
- 表格。
    
- 水平线。
    
- soft/hard line breaks。
    
- 转义。
    
- HTML entity。
    
- `$...$`。
    
- `$$...$$`。
    
- `\(...\)`。
    
- `\[...\]`。
    

数学 delimiter 的边界和转义规则完全继承 Comrak：

- 不自行写智能识别。
    
- 不对 `$5` 等情况额外猜测。
    
- code span 和 code block 中的公式标记不得被解释。
    
- `\$` 按 parser 结果处理。
    

## 7.3 Raw HTML

Comrak 正常识别 raw HTML 节点，但 StickyMD：

- 不执行。
    
- 不构建 DOM。
    
- 不解析 CSS。
    
- 不加载 iframe。
    
- 不运行 JavaScript。
    
- 不解释 `<style>`。
    
- 不解释 `<script>`。
    
- 不解释自定义元素。
    

预览中：

- inline HTML 以 inline-code 风格显示原始文本。
    
- block HTML 以 code-block 风格显示原始文本。
    
- 必须保留用户原文。
    

## 7.4 代码块

代码块：

- 字体使用 Consolas。
    
- 不做语法高亮。
    
- 不引入 syntect、Tree-sitter 或语言服务器。
    
- fenced info string 可以作为顶部小标签显示。
    
- 超长行允许横向滚动或在设置好的最大宽度内截断显示，但不得影响整个窗口布局。
    

## 7.5 表格

表格支持：

- GFM alignment。
    
- 单元格换行。
    
- 基本边框。
    
- 行背景轻微交替。
    
- 宽度不足时在表格区域横向滚动。
    
- 不支持列宽拖动。
    
- 不支持单元格编辑。
    
- task checkbox 在预览中只读。
    

## 7.6 链接安全

允许点击并交给系统处理：

```
http
https
mailto
file
```

自定义 URI scheme：

- 可以显示成链接。
    
- 不执行。
    
- 鼠标 hover 可以显示目标。
    

本地链接：

- 相对路径以 `note.md` 所在目录为基准。
    
- 点击时交给 Windows Shell。
    
- 不在程序内构建文件浏览器。
    

---

# 8. 数学公式规范

## 8.1 正式支持范围

StickyMD 的数学支持正式定义为：

> RaTeX/KaTeX-compatible 数学语法。

它不是：

- TeX Live。
    
- 完整 LaTeX 文档系统。
    
- 宏包管理器。
    
- `\usepackage` 环境。
    
- 任意 TeX 执行器。
    
- LaTeX 编译器。
    

## 8.2 数学引擎

使用 **RaTeX** 的 Rust crates：

```
ratex-parser
ratex-layout
ratex-types
ratex-font
ratex-font-loader
ratex-katex-fonts
```

RaTeX 当前采用纯 Rust lexer、parser、layout 与 DisplayList 管线，不依赖 JavaScript、DOM 或 WebView；其目标是与 KaTeX 数学语法和排版行为兼容。当前 workspace 基线版本为 0.1.14。citeturn641255search1turn26file0

不得自行实现：

- TeX tokenizer。
    
- 数学 AST。
    
- 分数布局。
    
- 根号布局。
    
- 矩阵布局。
    
- 可伸缩括号。
    
- 上下标算法。
    
- 数学字距算法。
    

## 8.3 数学字体

数学公式使用 RaTeX 配套的 KaTeX-compatible 数学字体。

不强制 Cambria Math。

RaTeX 分发的 KaTeX 字体采用 SIL Open Font License 1.1，而 RaTeX 程序代码采用 MIT。因此 release 必须包含相应第三方字体声明与 OFL 文本。

## 8.4 公式错误

公式解析失败时：

- 程序不得 panic。
    
- 预览显示原始公式文本。
    
- 使用轻微错误边框。
    
- 显示简短错误图标。
    
- hover 可显示简化错误信息。
    
- 不修改 `note.md`。
    
- 不尝试自动修复公式。
    

## 8.5 资源限制

为避免异常文本导致卡死：

|   |   |
|---|---|
|项目|v1 限制|
|单个公式源码|64 KiB|
|单文档公式数量|2000|
|超限行为|显示原文及错误提示|
|公式后台任务|可丢弃过期 generation|
|公式渲染|不得阻塞 UI 线程|

限制只用于保护程序，不修改源文件。

---

# 9. 字体与排版

## 9.1 字体规则

正文按字符脚本分段：

|   |   |   |
|---|---|---|
|内容|首选字体|fallback|
|中文/CJK 正文|仿宋_GB2312|仿宋 / FangSong / 系统 CJK 字体|
|Latin 正文|Times New Roman|系统 serif|
|代码|Consolas|系统 monospace|
|数学|RaTeX KaTeX fonts|RaTeX 内置 fallback|
|Emoji/特殊字符|系统 fallback|系统 emoji/CJK fallback|

例如：

```
这是 Rust 的 trait 示例
```

应形成多个字体 run：

```
这是              → 仿宋_GB2312
Rust              → Times New Roman
的                → 仿宋_GB2312
trait             → Times New Roman
示例              → 仿宋_GB2312
```

Markdown 标点跟随相邻正文 run。

## 9.2 源码模式

源码编辑区：

- 不做完整语法高亮。
    
- 中文使用仿宋_GB2312。
    
- 英文使用 Times New Roman。
    
- Markdown marker 不额外解析着色。
    
- 当前行可以有极轻背景提示。
    
- caret、selection、IME preedit 必须明显。
    
- 默认字号建议 16 DIP。
    
- 默认行高建议 1.55。
    

不做语法高亮是明确的性能和复杂度选择，而不是缺陷。

## 9.3 预览模式

建议排版 token：

|   |   |
|---|---|
|元素|建议|
|正文字号|17 DIP|
|正文行高|1.55|
|H1|1.75 em|
|H2|1.45 em|
|H3|1.25 em|
|行内代码|0.92 em|
|代码块|0.90 em|
|引用缩进|16 DIP|
|纸张内边距|22–28 DIP|

这些值固定，不提供排版设置页面。

---

# 10. 技术架构

## 10.1 总体原则

1. 不使用 WebView。
    
2. 不使用 Electron。
    
3. 不使用 Tauri。
    
4. 不使用 HTML/CSS 作为 UI 或预览渲染层。
    
5. 不使用 Tokio 或通用 async runtime。
    
6. 平台无关核心尽量使用 safe Rust。
    
7. Win32 调用集中在薄平台层。
    
8. UI 线程不执行文件写入、Markdown 全量解析或图片解码。
    
9. 空闲状态不持续 redraw。
    
10. 所有后台结果带 generation，过期结果直接丢弃。
    

## 10.2 总体数据流

```
┌──────────────────────────────────────────────┐
│                 winit UI thread              │
│                                              │
│  Input ─→ EditorController ─→ DocumentState  │
│                         │                    │
│                         ├─→ SaveScheduler    │
│                         ├─→ PreviewScheduler │
│                         └─→ AssetReconciler  │
│                                              │
│  AppState ─→ State Reducers ─→ Redraw        │
└───────────────────┬──────────────────────────┘
                    │
          EventLoopProxy / bounded channel
                    │
       ┌────────────┴────────────┐
       │                         │
┌──────▼────────┐        ┌───────▼────────┐
│ Preview Worker│        │   I/O Worker   │
│               │        │                │
│ Comrak        │        │ atomic save    │
│ Owned AST     │        │ export         │
│ RenderTree    │        │ image moves    │
│ RaTeX         │        │ recovery       │
│ layout        │        │ config save    │
└──────┬────────┘        └───────┬────────┘
       │                         │
       └────────────┬────────────┘
                    │
             UI event/result
                    │
┌───────────────────▼──────────────────────────┐
│ cosmic-text + tiny-skia + softbuffer         │
└──────────────────────────────────────────────┘
                    │
┌───────────────────▼──────────────────────────┐
│ platform/windows                             │
│                                              │
│ opacity / DWM / monitor / mutex / clipboard  │
│ atomic replace / Shell / optional RichEdit   │
└──────────────────────────────────────────────┘
```

## 10.3 线程模型

固定线程：

### UI 主线程

负责：

- winit event loop。
    
- 编辑状态。
    
- caret/selection。
    
- 窗口状态机。
    
- 输入事件。
    
- softbuffer present。
    
- tiny-skia 绘制调度。
    
- 后台结果提交。
    

### Preview Worker

单线程，建议栈大小 512 KiB：

- Markdown parse。
    
- AST 转换。
    
- RenderTree 构建。
    
- 文档 layout。
    
- 公式 parse/layout。
    
- 生成公式 raster 或可绘 DisplayList。
    
- generation 检查。
    

使用单线程而非线程池，避免额外线程栈和复杂调度。

### I/O Worker

单线程，建议栈大小 256 KiB：

- note.md 原子保存。
    
- config.toml 原子保存。
    
- 图片写入。
    
- 图片移动。
    
- `.trash` 清理。
    
- 导出。
    
- 文件恢复。
    

### 文件监听

由 `notify` 的 Windows backend 提供。回调必须只发送轻量事件，不直接改变 AppState。`notify` 在 Windows 使用系统目录变化通知机制。citeturn446267search2

---

# 11. Crate 选型

以下版本为 2026-08-18 的实现基线。正式提交必须保留 `Cargo.lock` 并使用 `--locked` 构建。

Rust 工具链固定为当时稳定版 1.97.1；v1 不对外承诺独立 MSRV，仓库以 `rust-toolchain.toml` 为准。citeturn181774search0turn181774search10

|   |   |   |   |
|---|---|---|---|
|Crate|基线|用途|说明|
|`winit`|0.30.13|窗口、事件、键盘、IME|稳定版；使用 IME cursor area、drag window 等接口|
|`cosmic-text`|0.19|shaping、fallback、caret、selection|纯 Rust 多行文本布局与编辑基础|
|`softbuffer`|0.4.8|将软件 framebuffer 显示到窗口|不初始化 WebGPU|
|`tiny-skia`|0.12|2D CPU 绘制|背景、边框、路径、分隔线、图片合成|
|`comrak`|0.54|CommonMark/GFM AST|Markdown 与 delimiter 解析|
|`ratex-*`|0.1.14|数学 parser/layout/fonts|KaTeX-compatible math|
|`tray-icon`|0.24|Windows 系统托盘|与 winit EventLoopProxy 集成|
|`notify`|锁定兼容版本|外部文件监听|只监听 `note/`|
|`rfd`|锁定兼容版本|导出文件对话框|使用原生 Windows dialog|
|`arboard`|锁定兼容版本|文本/位图剪贴板|文件列表剪贴板由 Windows 层补充|
|`image`|0.25 系列|PNG/JPEG/WebP/GIF 解码与 PNG 编码|关闭不需要的格式 feature|
|`windows`|锁定兼容版本|必需 Win32 API|仅启用所需 Win32 feature|
|`serde`|1.x|config 序列化|derive|
|`toml`|锁定版本|config.toml|不使用 JSON|
|`sha2`|0.10 系列|SHA-256|单实例、图片命名、文件 hash|
|`url`|2.x|URL scheme 与路径处理|链接安全|
|`thiserror`|2.x|typed error|核心库错误|
|`crossbeam-channel`|锁定版本|bounded worker channel|不引入 async runtime|
|`bytemuck`|锁定版本|framebuffer 安全转换|严格限制使用位置|
|`unicode-script`|锁定版本|字符脚本分类|中文/Latin 字体 run|

winit、cosmic-text、softbuffer、tiny-skia 和 tray-icon 当前都提供对应的原生窗口、文本排版、软件 framebuffer 与托盘能力。citeturn279187search0turn575945search0turn575945search1turn174535search1turn174535search3turn446267search0

## 11.1 明确禁止的依赖

不得加入：

```
tauri
electron
webview2
cef
wry
tokio
async-std
wgpu
iced
egui
slint
qt
gtk
sdl
browser engine
javascript runtime
```

例外需要：

1. 新增 ADR。
    
2. 明确说明现有方案为何不可行。
    
3. 给出内存、二进制体积和安全影响。
    
4. 获得项目维护者批准。
    

## 11.2 Feature 控制

应做到：

- `image` 只启用必要格式。
    
- `windows` 只启用必要 Win32 namespaces。
    
- `tray-icon` 作为 Windows target-specific dependency。
    
- RaTeX release 启用嵌入字体。
    
- 不启用 Comrak 不需要的 CLI、语法高亮或额外输出后端。
    
- dev-only benchmark、snapshot 和 fuzz crates 不进入 release。
    

---

# 12. 仓库结构

```
StickyMD\
├─ Cargo.toml
├─ Cargo.lock
├─ rust-toolchain.toml
├─ LICENSE
├─ README.md
├─ SPEC.md
├─ ARCHITECTURE.md
├─ ROADMAP.md
├─ AGENTS.md
├─ CONTRIBUTING.md
├─ SECURITY.md
├─ THIRD_PARTY_NOTICES.md
├─ deny.toml
│
├─ .github\
│  ├─ workflows\
│  │  ├─ ci.yml
│  │  ├─ release.yml
│  │  └─ scheduled.yml
│  ├─ ISSUE_TEMPLATE\
│  └─ pull_request_template.md
│
├─ crates\
│  ├─ stickymd-core\
│  │  ├─ Cargo.toml
│  │  └─ src\
│  │     ├─ lib.rs
│  │     ├─ document.rs
│  │     ├─ text_store.rs
│  │     ├─ edit.rs
│  │     ├─ undo.rs
│  │     ├─ assets.rs
│  │     ├─ config.rs
│  │     ├─ markdown.rs
│  │     ├─ links.rs
│  │     ├─ state.rs
│  │     └─ error.rs
│  │
│  ├─ stickymd-render\
│  │  ├─ Cargo.toml
│  │  └─ src\
│  │     ├─ lib.rs
│  │     ├─ owned_ast.rs
│  │     ├─ render_tree.rs
│  │     ├─ layout.rs
│  │     ├─ paragraph.rs
│  │     ├─ table.rs
│  │     ├─ code.rs
│  │     ├─ math.rs
│  │     ├─ image.rs
│  │     ├─ selection.rs
│  │     ├─ cache.rs
│  │     └─ paint.rs
│  │
│  └─ stickymd-win\
│     ├─ Cargo.toml
│     ├─ build.rs
│     └─ src\
│        ├─ main.rs
│        ├─ app.rs
│        ├─ event.rs
│        ├─ commands.rs
│        ├─ editor\
│        │  ├─ mod.rs
│        │  ├─ cosmic_backend.rs
│        │  ├─ ime.rs
│        │  └─ rich_edit_backend.rs
│        ├─ ui\
│        │  ├─ mod.rs
│        │  ├─ controls.rs
│        │  ├─ theme.rs
│        │  ├─ opacity.rs
│        │  ├─ conflict_banner.rs
│        │  └─ recovery_banner.rs
│        ├─ workers\
│        │  ├─ preview.rs
│        │  └─ io.rs
│        └─ platform\
│           └─ windows\
│              ├─ mod.rs
│              ├─ window_effects.rs
│              ├─ monitor.rs
│              ├─ docking.rs
│              ├─ single_instance.rs
│              ├─ atomic_file.rs
│              ├─ clipboard.rs
│              ├─ shell.rs
│              └─ crash.rs
│
├─ assets\
│  ├─ icon\
│  └─ licenses\
│     ├─ SIL-OFL-1.1.txt
│     └─ KaTeX-fonts-NOTICE.txt
│
├─ tests\
│  ├─ markdown\
│  ├─ math\
│  ├─ images\
│  ├─ recovery\
│  └─ export\
│
├─ benches\
│  ├─ markdown.rs
│  ├─ layout.rs
│  └─ editing.rs
│
└─ tools\
   ├─ measure-memory.ps1
   ├─ package.ps1
   └─ verify-release.ps1
```

## 12.1 unsafe 边界

`stickymd-core`：

```
#![forbid(unsafe_code)]
```

`stickymd-render`：

```
#![forbid(unsafe_code)]
```

`stickymd-win`：

```
#![deny(unsafe_op_in_unsafe_fn)]
```

所有 `unsafe` 必须：

- 只位于 `platform/windows/` 或 RichEdit fallback。
    
- 紧邻 `// SAFETY:` 注释。
    
- 说明指针、句柄、线程和生命周期约束。
    
- 有对应测试或最小复现程序。
    
- 不把裸句柄泄漏到核心层。
    

---

# 13. 核心数据模型

## 13.1 AppState

```
struct AppState {
    lifecycle: LifecycleState,
    visibility: VisibilityState,
    docking: DockState,
    view_mode: ViewMode,
    theme: ThemeMode,
    opacity: u8,
    always_on_top: bool,

    document: DocumentState,
    preview: PreviewState,
    save: SaveState,
    conflict: Option<FileConflict>,
    recovery: Option<RecoveryCandidate>,
    assets: AssetState,
    ime: ImeState,

    source_scroll: ScrollState,
    preview_scroll: ScrollState,
}
```

## 13.2 DocumentState

```
struct DocumentState {
    text: StringTextStore,
    generation: u64,
    saved_generation: u64,
    base_disk_hash: Option<[u8; 32]>,
    dirty: bool,
    line_ending: LineEnding,
    undo: UndoManager,
    managed_ref_counts: HashMap<ManagedAssetName, usize>,
}
```

v1 首先使用 `String` 作为规范文本存储。

理由：

- 只有一个 scratchpad。
    
- 目标文档通常较小。
    
- 简化 UTF-8、保存、undo 和 worker snapshot。
    
- 避免同时引入 rope 与 editor model 的双重复杂性。
    

定义内部 trait：

```
trait TextStore {
    fn as_str(&self) -> &str;
    fn apply(&mut self, delta: &TextDelta) -> Result<(), EditError>;
    fn len_bytes(&self) -> usize;
}
```

如果 1 MiB 文档的性能 gate 无法满足，可以在不改变上层 API 的情况下切换为 rope。不得在基准测试前提前引入 rope。

## 13.3 TextDelta

```
struct TextDelta {
    range: Range<usize>,
    deleted: Arc<str>,
    inserted: Arc<str>,
    cursor_before: CursorSnapshot,
    cursor_after: CursorSnapshot,
}
```

要求：

- range 始终落在 UTF-8 char boundary。
    
- 一次 IME commit 是一个 delta。
    
- 一次图片粘贴是一个 delta。
    
- 连续普通输入可进行 undo grouping。
    
- worker 不直接持有可变 DocumentState。
    

---

# 14. Undo/Redo

## 14.1 范围

Undo/Redo：

- 只存在于当前进程。
    
- 重启后清空。
    
- 不写磁盘。
    
- 不实现历史浏览。
    
- 不与 autosave 绑定。
    

限制：

```
最多 256 entries
或
最多 4 MiB undo memory
```

先达到者触发丢弃最老 entry。

## 14.2 UndoEntry

```
struct UndoEntry {
    text: TextDelta,
    assets: Vec<AssetEffect>,
    approx_bytes: usize,
    timestamp: Instant,
    group: UndoGroup,
}
```

`AssetEffect`：

```
enum AssetEffect {
    MoveToTrash { name: ManagedAssetName },
    RestoreFromTrash { name: ManagedAssetName },
    CreateManaged { name: ManagedAssetName },
}
```

## 14.3 输入分组

连续输入满足以下条件时可以合并：

- 相邻位置。
    
- 同一输入类型。
    
- 间隔小于 750 ms。
    
- 中间没有 selection 替换。
    
- 中间没有换行、粘贴或 IME commit。
    

以下必须独立成为一个 undo entry：

- IME commit。
    
- 粘贴。
    
- 图片粘贴。
    
- 删除 selection。
    
- Enter。
    
- 外部 reload 不进入 undo，而是清空 undo。
    
- 程序化恢复不进入普通 undo。
    

---

# 15. IME 与源码编辑器策略

## 15.1 第一实现

第一实现必须是：

```
winit IME events
+ cosmic-text shaping
+ 自有 DocumentState
+ 自绘 caret / selection / preedit
```

不得一开始使用 RichEdit。

## 15.2 IME 状态

```
enum ImeState {
    Disabled,
    Enabled,
    Preediting {
        text: String,
        selection: Option<Range<usize>>,
        anchor: CursorSnapshot,
    },
}
```

IME preedit：

- 不写入规范文档。
    
- 不触发 autosave。
    
- 不进入 undo。
    
- 不触发图片引用 reconcile。
    
- 以带下划线的临时 run 绘制。
    
- composition commit 后一次性产生 TextDelta。
    
- composition cancel 后文档保持不变。
    

候选框位置必须通过当前 caret 的屏幕坐标更新：

```
window.set_ime_cursor_area(...)
```

## 15.3 一级验收输入法

必须人工通过：

- 微软拼音。
    
- 微信输入法。
    

验证项目：

1. 中文连续输入。
    
2. 中英文混输。
    
3. 候选框位于 caret 附近。
    
4. selection 状态下开始 composition。
    
5. composition 中按左右键。
    
6. composition 中按 Backspace。
    
7. composition commit 后一次 Ctrl+Z 撤销整次提交。
    
8. composition 取消不污染 undo。
    
9. 高 DPI 候选框位置正确。
    
10. 分栏、源码模式、吸附展开后都可输入。
    
11. 整窗透明度 70–100 时输入正常。
    
12. 窗口失焦/重新聚焦后 composition 状态正确。
    
13. 输入期间绝不自动收起。
    

## 15.4 RichEdit 最后回退

定义统一接口：

```
trait EditorBackend {
    fn set_text(&mut self, text: &str);
    fn apply_delta(&mut self, delta: &TextDelta);
    fn selection(&self) -> Selection;
    fn set_selection(&mut self, selection: Selection);
    fn handle_event(&mut self, event: EditorEvent) -> EditorOutcome;
    fn draw(&mut self, frame: &mut Frame);
}
```

实现：

```
CosmicEditorBackend
RichEditBackend
```

RichEdit fallback 只有在以下条件全部满足后才允许启用：

1. 已完成至少两轮纯 Rust IME 修复。
    
2. 微软拼音或微信输入法仍存在阻塞性问题。
    
3. 问题有可复现步骤。
    
4. `DESIGN_RISK_IME.md` 已记录。
    
5. fallback 被 feature flag 隔离：
    

```
richedit-fallback
```

RichEdit 只负责源码输入区。

以下仍必须保持 Rust 实现：

- DocumentState。
    
- Undo/Redo 外层事务。
    
- Markdown。
    
- 数学。
    
- Preview。
    
- 文件系统。
    
- 图片。
    
- 窗口。
    
- 托盘。
    
- Docking。
    

---

# 16. Markdown 与预览渲染管线

## 16.1 解析流程

```
Arc<str> source snapshot
        │
        ▼
Comrak parse_document
        │
        ▼
Comrak Arena AST
        │
        ▼
OwnedDocumentTree
        │
        ▼
RenderTree
        │
        ▼
Block/Inline Layout
        │
        ├─ cosmic-text paragraphs
        ├─ RaTeX formulas
        ├─ lazy images
        └─ tiny-skia decorations
        │
        ▼
LaidOutDocument
```

## 16.2 不长期保存 Comrak Arena

Comrak AST 带 Arena 生命周期。

Preview worker 应：

1. 创建 Arena。
    
2. parse。
    
3. 遍历 AST。
    
4. 转为项目自有 `OwnedDocumentTree`。
    
5. 释放 Arena。
    
6. 从 owned tree 构建 RenderTree。
    

不得把 Comrak Arena 跨线程或长期保存在 AppState。

## 16.3 OwnedDocumentTree

节点示例：

```
enum BlockNode {
    Paragraph(Vec<InlineNode>),
    Heading { level: u8, content: Vec<InlineNode> },
    BlockQuote(Vec<BlockNode>),
    List(ListNode),
    CodeBlock(CodeBlockNode),
    Table(TableNode),
    ThematicBreak,
    HtmlLiteral(String),
    DisplayMath(String),
}

enum InlineNode {
    Text(String),
    Emphasis(Vec<InlineNode>),
    Strong(Vec<InlineNode>),
    Strikethrough(Vec<InlineNode>),
    Code(String),
    Link { destination: String, children: Vec<InlineNode> },
    Image { destination: String, alt: String },
    InlineMath(String),
    SoftBreak,
    HardBreak,
    HtmlLiteral(String),
}
```

保存 source range，以便：

- Preview selection。
    
- 点击链接。
    
- 错误定位。
    
- 公式复制。
    
- 调试。
    

## 16.4 Preview layout

v1 不实现增量 Markdown parser。

每次刷新：

- 后台全量 parse。
    
- 后台全量构建 block layout。
    
- UI 只绘制 viewport 范围内的 block。
    
- 旧 preview 在新结果完成前保持可用。
    
- generation 不匹配的结果立即释放。
    

这是用 debounce 换取架构简单和稳定性的明确决策。

## 16.5 文本选择

Preview 中每个文字 run 应保存：

```
struct TextMapEntry {
    source_range: Range<usize>,
    display_text: Arc<str>,
    rects: Vec<GlyphRect>,
}
```

选择公式时：

- 视觉上选择公式矩形。
    
- `Ctrl+C` 复制其原始数学源码和 delimiter。
    

选择图片时：

- 复制 alt text。
    
- 不把 bitmap 复制到剪贴板，除非未来单独设计。
    

---

# 17. RaTeX 渲染桥接

RaTeX parser/layout 输出 `DisplayList`。

当前 `ratex-render` 的公开接口主要暴露 `render_to_png`；其内部 renderer 使用 tiny-skia 绘制 DisplayList。

## 17.1 原型阶段

技术 spike 可以使用：

```
DisplayList
→ render_to_png
→ decode
→ preview
```

只用于验证：

- parser。
    
- layout。
    
- 字体。
    
- delimiter。
    
- 公式正确性。
    

## 17.2 正式实现

正式 beta 前禁止保留 PNG encode/decode 热路径。

按优先级选择：

### 方案 A：向 RaTeX 上游贡献 API

建议接口：

```
pub fn render_into_pixmap(
    display_list: &DisplayList,
    pixmap: &mut tiny_skia::PixmapMut<'_>,
    origin: Point,
    options: &RenderOptions,
) -> Result<RenderMetrics, RenderError>;
```

这是首选。

### 方案 B：项目内维护很薄的 DisplayList painter

如果上游尚未发布：

- 只遍历 RaTeX `DisplayList`。
    
- 只负责 GlyphPath、Line、Rect、Path 绘制。
    
- 不实现数学 parser/layout。
    
- 控制在约 200–400 行。
    
- 保留 MIT attribution。
    
- 与 RaTeX golden tests 对照。
    

### 禁止方案

不得：

- fork 整套 RaTeX。
    
- 自行实现数学布局。
    
- 运行外部 LaTeX。
    
- 调用浏览器 KaTeX。
    
- 启动 Node.js。
    
- 将公式交给 WebView2。
    

---

# 18. 渲染后端

## 18.1 Framebuffer

建议管线：

```
tiny-skia Pixmap
+ cosmic-text glyph raster
+ formula raster
+ decoded images
        │
        ▼
single software framebuffer
        │
        ▼
softbuffer present
```

要求：

- 仅在 dirty 时绘制。
    
- 空闲时 event loop 进入 wait。
    
- 动画期间短暂 request redraw。
    
- 不运行永久 60 FPS 循环。
    
- resize 时复用容量，避免每帧分配。
    
- framebuffer 大小变化时才重建。
    

## 18.2 绘制层级

```
1. transparent/window background
2. paper background
3. shadow/border
4. source or preview content
5. selection
6. caret/IME preedit
7. conflict/recovery banner
8. top controls
9. transient popup
```

## 18.3 缓存

### 公式缓存

两级缓存：

```
MathLayoutCache:
    source + display_mode
    → RaTeX DisplayList

MathRasterCache:
    display_list_hash + font_size + dpi + theme
    → raster
```

预算：

```
MathRasterCache ≤ 8 MiB
```

### 图片缓存

只缓存当前 viewport 附近图片：

```
DecodedImageCache ≤ 16 MiB
```

超出预算时 LRU 淘汰。

### 隐藏时

进入 tray hidden 或 dock collapsed 一段时间后：

- 清理所有解码图片。
    
- 清理公式 raster。
    
- 保留小型 layout cache。
    
- 保留文档和字体数据库。
    
- 不保留无必要 framebuffer 副本。
    

---

# 19. 自动保存与原子写入

## 19.1 保存调度

文本修改后：

```
dirty = true
save_deadline = now + 650ms
```

连续输入不断后移 deadline。

以下操作立即保存：

- `Ctrl+S`。
    
- 窗口失焦。
    
- Hide to tray。
    
- 程序退出。
    
- Windows session shutdown 通知。
    
- 外部冲突选择“保留本地”。
    

保存由 I/O worker 执行。

如果保存期间又有新修改：

- 当前保存完成。
    
- 立即保存最新 generation。
    
- 中间 generation 可被合并。
    
- `saved_generation` 只更新到实际落盘 generation。
    

## 19.2 原子保存步骤

当目标文件已存在：

1. 在同目录创建 `note.md.tmp`。
    
2. 写入完整 UTF-8 内容。
    
3. flush 用户态 buffer。
    
4. 调用 Windows `FlushFileBuffers`。
    
5. 使用 `ReplaceFileW` 替换原文件。
    
6. 替换失败时，在安全条件下使用 `MoveFileExW`：
    
    - `MOVEFILE_REPLACE_EXISTING`
        
    - `MOVEFILE_WRITE_THROUGH`
        
7. 更新磁盘 hash。
    
8. 删除残留 temp。
    

Windows 提供 `ReplaceFileW` 和带 replace/write-through 标志的 `MoveFileExW` 用于文件替换。citeturn628483search0turn628483search3

不得：

- 原地 truncate 后写入。
    
- 在 UI 线程保存。
    
- 保存半个文件。
    
- 因 config 保存失败而损坏 note.md。
    

## 19.3 崩溃恢复边界

StickyMD 保证：

- 正常保存不会留下半文件。
    
- 原文件在替换前仍然存在。
    
- 启动时能检测更新的有效 temp。
    
- 配置损坏不影响笔记。
    

StickyMD 不保证：

- 进程突然终止时最后 650 ms 的每一个字符都已落盘。
    
- 断电期间尚未触发 autosave 的输入永久存在。
    

## 19.4 启动恢复

发现 `note.md.tmp`：

1. 校验为合法 UTF-8。
    
2. 比较 mtime。
    
3. 比较 hash。
    
4. 如果 temp 更新且内容不同，显示薄恢复提示：
    

```
发现未完成保存的内容
[恢复临时内容] [使用当前文件]
```

在用户选择前：

- 不覆盖任何文件。
    
- 暂停 autosave。
    
- 内存中保留两份内容所需的最小数据。
    

---

# 20. 外部文件修改与冲突

## 20.1 自己的保存事件

文件 watcher 可能观察到程序自己的原子替换。

程序通过：

- `last_saved_hash`
    
- 保存 generation
    
- 短期 write token
    

识别自身写入。

如果 watcher 内容 hash 与 `last_saved_hash` 相同，忽略。

## 20.2 Buffer 干净

外部 `note.md` 发生变化且当前 buffer 干净：

1. 读取新内容。
    
2. 校验 UTF-8。
    
3. 更新 DocumentState。
    
4. 清空 Undo/Redo。
    
5. 更新 base hash。
    
6. 重新 reconcile 图片。
    
7. 标记 preview dirty。
    
8. 不弹阻塞对话框。
    

## 20.3 Buffer 脏

外部发生变化且当前 buffer 脏：

```
SaveState → Conflict
autosave → paused
```

顶部显示薄 banner：

```
文件已在外部修改
[载入外部] [保留本地]
```

选择“载入外部”：

- 丢弃当前未保存 buffer。
    
- 载入外部。
    
- 清空 undo。
    
- 更新 hash。
    
- reconcile assets。
    

选择“保留本地”：

- 原子覆盖外部文件。
    
- 更新 hash。
    
- 保留当前 undo。
    
- 解除冲突。
    
- 继续 autosave。
    

冲突期间允许继续输入，但 autosave 暂停。

## 20.4 外部删除

如果 `note.md` 被外部删除：

- 不清空内存。
    
- 不创建空白文件覆盖内存。
    
- 使用当前内存内容原子恢复 canonical `note.md`。
    
- 显示短暂非阻塞提示。
    

## 20.5 无效 UTF-8

外部内容不是合法 UTF-8：

- 不载入。
    
- 不自动覆盖。
    
- 进入冲突状态。
    
- 提示“外部文件不是有效 UTF-8”。
    
- 允许用户选择“保留本地覆盖”。
    
- 用户也可以先用导出保存内存内容。
    

---

# 21. 配置文件

## 21.1 config.toml

建议 schema：

```
version = 1
theme = "light"
opacity = 96
always_on_top = false
view_mode = "source"

[window]
width_dip = 520
height_dip = 680
monitor_id = ""
dock_edge = "none"
dock_offset_ratio = 0.5
floating_x_ratio = 0.5
floating_y_ratio = 0.25
```

枚举值：

```
theme:
    light
    system
    dark

view_mode:
    source
    split
    preview

dock_edge:
    none
    left
    right
    top
```

## 21.2 保存

配置也必须使用：

```
config.toml.tmp
→ flush
→ atomic replace
```

未知字段：

- 应忽略，以便向前兼容。
    

缺少字段：

- 使用默认值。
    

## 21.3 配置损坏

解析失败：

```
config.toml
→ config.invalid-<timestamp>.toml
```

然后：

- 使用默认配置启动。
    
- 不影响 `note.md`。
    
- 显示短暂提示。
    
- 不覆盖损坏文件。
    

---

# 22. 图片粘贴

## 22.1 剪贴板优先级

Windows 粘贴时按以下顺序检测：

1. `CF_HDROP` 图片文件列表。
    
2. 可直接取得的原编码 PNG/JPEG/WebP。
    
3. DIB/DIBV5 或普通 bitmap。
    
4. Unicode text。
    

## 22.2 编码规则

### 文件剪贴板

如果是：

- PNG：保留原始 bytes。
    
- JPEG：保留原始 bytes。
    
- WebP：保留原始 bytes。
    
- GIF：保留原始 bytes；预览可只显示首帧。
    
- 其他稳定且 `image` 可解码格式：可以保留。
    
- 不适合稳定预览的格式：解码后转 PNG。
    

### 截图或 bitmap

统一编码为 PNG。

## 22.3 文件命名

对最终准备写入磁盘的 bytes 计算 SHA-256：

```
sha256(final_bytes)
```

取前 20 个十六进制字符：

```
images/stickymd-7c9a0d7f8139e921a3f4.png
```

相同 bytes：

- 不重复写文件。
    
- 复用现有文件。
    
- 如果同名文件位于 `.trash`，先恢复。
    

## 22.4 Markdown 插入

单张图片：

```
![](images/stickymd-7c9a0d7f8139e921a3f4.png)
```

多张图片：

```
![](images/stickymd-a....png)

![](images/stickymd-b....jpg)

![](images/stickymd-c....webp)
```

图片写入成功后才插入 Markdown。

如果写入失败：

- 不插入引用。
    
- 显示错误。
    
- 剪贴板文本不受影响。
    

图片写入和文本插入必须成为同一个 UndoEntry。

---

# 23. 图片引用与安全边界

## 23.1 Managed 图片

只有文件名匹配：

```
stickymd-<20-hex>.<supported-ext>
```

且位于：

```
./note/images/
./note/.trash/
```

时，才是 managed asset。

程序只能自动移动或删除 managed asset。

## 23.2 用户图片

用户手工放入：

```
./note/images/my-photo.png
```

程序：

- 可以显示。
    
- 可以导出。
    
- 不自动删除。
    
- 不移动到 `.trash`。
    
- 不重命名。
    

## 23.3 任意本地引用

允许预览：

```
![](images/custom.png)
![](../shared/diagram.png)
![](C:/Users/name/Desktop/a.png)
```

这些引用：

- 只读。
    
- 不属于 GC。
    
- Export 时可以复制。
    
- 必须进行路径规范化。
    
- 不允许因路径穿越而写入目标目录外。
    

## 23.4 远程图片

对于：

```
![alt](https://example.com/a.png)
```

StickyMD：

- 不发起网络请求。
    
- 不下载。
    
- 不缓存。
    
- 显示 alt text。
    
- 显示可点击链接。
    
- 导出 Markdown 时保留原 URL。
    

程序默认不需要网络权限或 HTTP client dependency。

## 23.5 图片安全限制

建议：

|   |   |
|---|---|
|限制|值|
|最大解码像素|40 MP|
|最大单边尺寸|16384 px|
|最大编码文件|64 MiB|
|decoded cache|16 MiB|
|超限行为|显示占位符，不修改源文件|

导出可以复制超限原文件，但预览不解码。

---

# 24. 图片 GC 与 Undo 事务

## 24.1 引用统计

为避免每个按键都运行完整 Markdown parse，managed 图片 GC 使用保守引用扫描：

- 扫描当前文本中的 managed 文件名。
    
- 只要 managed 路径字面量仍存在，就视为引用。
    
- 即使它位于 code block 中，也宁可暂时保留。
    
- 不允许因为 parser 边界判断错误而误删图片。
    

完整 Comrak AST 只用于 preview 与 export。

## 24.2 逻辑删除

一次编辑后，如果 managed image 引用计数：

```
1 → 0
```

生成：

```
AssetEffect::MoveToTrash
```

I/O worker 将：

```
note/images/stickymd-x.png
→ note/.trash/stickymd-x.png
```

文本变化和 asset move 属于同一个 UndoEntry。

## 24.3 Undo

按 `Ctrl+Z`：

1. 恢复 Markdown 文本。
    
2. `0 → 1`。
    
3. 将图片从 `.trash` 恢复到 `images/`。
    
4. 更新 managed ref count。
    
5. 标记 preview dirty。
    

## 24.4 Redo

按 `Ctrl+Y`：

1. 再次删除 Markdown 引用。
    
2. 将图片重新移动到 `.trash/`。
    
3. 更新引用计数。
    
4. 标记 preview dirty。
    

## 24.5 并发与顺序

Asset operations 使用单一 I/O worker 串行执行。

每个操作带：

```
transaction_id
document_generation
asset_name
expected_state
```

如果 undo 在实际 move 前发生：

- 可以取消尚未执行的 move。
    
- 或按队列顺序先 move 后 restore。
    
- 最终状态必须与最新 DocumentState 一致。
    

## 24.6 正常退出清理

退出时：

1. 等待所有 asset operations。
    
2. 重新扫描最新 `note.md` 内存内容。
    
3. 只删除 `.trash` 中确认无引用的 managed 文件。
    
4. referenced managed 文件必须恢复到 `images/`。
    
5. 用户文件永不删除。
    

## 24.7 异常退出后的启动清理

启动时绝不能先清空 `.trash`。

正确顺序：

1. 加载 `note.md`。
    
2. 处理可能存在的恢复 temp。
    
3. 建立 managed reference set。
    
4. 如果被引用文件位于 `.trash`，恢复到 `images`。
    
5. 对未引用 managed trash 执行永久删除。
    
6. 对 `images` 中未引用 managed 文件移动到 `.trash`，再按安全策略清理。
    
7. 用户文件不动。
    

---

# 25. 导出

快捷键：

```
Ctrl+Shift+S
```

UI 名称：

```
导出
```

不是“另存为”。

## 25.1 语义

用户选择：

```
D:\Export\my-note.md
```

程序创建：

```
D:\Export\
├─ my-note.md
└─ my-note-assets\
   ├─ stickymd-a.png
   └─ external-b.jpg
```

## 25.2 导出规则

- 当前工作文件仍然是 `./note/note.md`。
    
- 不切换 active document。
    
- 只复制实际引用的本地图片。
    
- remote URL 保留原样。
    
- 导出副本中的本地图片引用重写为：
    

```
![](my-note-assets/stickymd-a.png)
```

- 文件名冲突使用内容 hash 解决。
    
- raw HTML 原样保留在 Markdown。
    
- 不导出配置。
    
- 不导出 `.trash`。
    
- 不导出未引用 managed 图片。
    
- 不生成 HTML/PDF。
    

---

# 26. 窗口与平台适配层

## 26.1 优先使用跨平台 Rust API

优先使用 winit：

- 创建窗口。
    
- keyboard/mouse。
    
- IME。
    
- drag window。
    
- resize。
    
- cursor。
    
- redraw。
    
- window level，若行为满足要求。
    

只有 winit 不足时才进入 Win32 层。

## 26.2 Windows 专用功能

薄平台层负责：

- `SetLayeredWindowAttributes`。
    
- DWM rounded corner。
    
- fixed window shadow。
    
- QueryDisplayConfig。
    
- named mutex/event。
    
- `ReplaceFileW`。
    
- `MoveFileExW`。
    
- `FlushFileBuffers`。
    
- `CF_HDROP`。
    
- Shell open。
    
- Windows theme registry/event。
    
- 可选 RichEdit fallback。
    
- session shutdown notification。
    

业务层不得直接调用 Win32。

## 26.3 DPI

应用 manifest 必须声明：

```
PerMonitorV2
```

要求：

- 移动到另一显示器时重算 scale。
    
- 公式 cache key 包含 DPI。
    
- 图片按设备像素绘制。
    
- IME candidate rect 使用物理坐标。
    
- Docking 保存 DIP，而不是固定物理像素。
    
- 3 DIP 感应条按显示器 scale 转换。
    

---

# 27. 状态机

## 27.1 VisibilityState

```
enum VisibilityState {
    HiddenToTray,
    Floating,
    DockedExpanded(DockEdge),
    DockedCollapsed(DockEdge),
    Animating {
        from: WindowRect,
        to: WindowRect,
        end: Instant,
        final_state: Box<VisibilityState>,
    },
}
```

优先级：

```
Quit
> ManualHide / Esc
> ActiveDrag
> Focused / IME composing
> Conflict/Recovery interaction
> Auto-hide timers
> Hover reveal
```

## 27.2 PreviewState

```
enum PreviewState {
    Empty,
    Clean {
        generation: u64,
        document: Arc<LaidOutDocument>,
    },
    Dirty {
        generation: u64,
    },
    Scheduled {
        generation: u64,
        deadline: Instant,
    },
    Rendering {
        generation: u64,
    },
    Failed {
        generation: u64,
        error: PreviewError,
    },
}
```

过期结果：

```
result.generation != document.generation
→ drop
```

## 27.3 SaveState

```
enum SaveState {
    Clean { hash: [u8; 32] },
    Dirty { generation: u64 },
    Scheduled { generation: u64, deadline: Instant },
    Saving { generation: u64 },
    Conflict(FileConflict),
    Failed(SaveError),
}
```

## 27.4 DockState

```
struct DockState {
    edge: Option<DockEdge>,
    monitor_id: Option<MonitorId>,
    offset_ratio: f32,
    manually_hidden: bool,
    hover_revealed: bool,
    focus_guard: bool,
}
```

## 27.5 生命周期状态转移

```
Start
  ↓
Writable Check
  ↓
Single Instance Check
  ↓
Load Config
  ↓
Load note.md / Recovery Detection
  ↓
Asset Reconciliation
  ↓
Create Window + Tray
  ↓
Running
  ├─ Close button → HiddenToTray
  ├─ Tray Show → Floating/DockedExpanded
  └─ Tray Quit → Flush → Cleanup → Exit
```

---

# 28. 性能预算

这些数值是工程目标和 release gate，不是未经测量的宣传承诺。

## 28.1 测量口径

测试环境必须记录：

- Windows 11 build。
    
- CPU。
    
- RAM。
    
- DPI。
    
- 显示器数量。
    
- release commit。
    
- Rust toolchain。
    
- 是否首次启动。
    
- 是否开启 Defender。
    
- 文档 fixture。
    

内存指标使用：

- Private Working Set。
    
- Commit Size。
    
- 启动后等待 30 秒。
    
- 所有动画结束。
    
- 无调试器。
    
- Release build。
    
- 重复至少 5 次，报告 median 和最大值。
    

## 28.2 目标

|   |   |   |
|---|---|---|
|场景|目标|v1 硬门槛|
|源码模式、20 KiB 文本|≤28 MiB|≤40 MiB|
|预览模式、20 KiB、20 个公式|≤40 MiB|≤52 MiB|
|分栏模式、同上|≤48 MiB|≤64 MiB|
|Hidden to tray，cache purge 后|≤24 MiB|≤36 MiB|
|空闲 CPU，60 秒平均|≤0.05%|≤0.1%|
|冷启动到可输入 p95|≤180 ms|≤300 ms|
|热启动到可输入 p95|≤100 ms|≤180 ms|
|100 KiB 输入延迟 p95|≤16 ms|≤25 ms|
|1 MiB 输入延迟 p95|≤33 ms|≤50 ms|
|20 KiB preview 构建|≤50 ms|≤100 ms|
|100 KiB preview 构建|≤200 ms|≤400 ms|
|1 MiB preview 构建|≤1 s，后台|≤2 s，后台|
|Portable ZIP|≤20 MiB|≤30 MiB|

分栏模式由于同时保留源码布局和预览布局，可以有有限的内存例外，但仍不得持续增长。

## 28.3 支持范围

优化重点：

```
典型：0–100 KiB
支持：0–1 MiB
容忍：1–5 MiB
```

超过 5 MiB：

- 源码编辑仍应尽力工作。
    
- Preview 可以显示性能警告并要求手动继续。
    
- 不允许程序崩溃或无响应。
    
- 不把 StickyMD 发展成大文件编辑器。
    

## 28.4 内存策略

- RaTeX 字体按需加载。
    
- Source-only 模式不主动初始化数学字体。
    
- Preview 不可见时释放图片 decode cache。
    
- Hidden 状态释放公式 raster。
    
- 不缓存完整历史 preview。
    
- 最新 preview 替换旧 preview 后立即释放旧树。
    
- Undo 达到 4 MiB 后淘汰旧 entry。
    
- 不使用通用线程池。
    
- 不引入 mimalloc，除非基准证明有明确收益。
    
- 不在每次按键复制整个文档。
    
- 只有 preview worker snapshot 时产生一次 `Arc<str>` 快照。
    
- 不在空闲时持续轮询。
    

## 28.5 Release profile

```
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

可以建立额外的 `release-size` profile，但正式默认优先运行性能。

---

# 29. 测试矩阵

## 29.1 单元测试

### 文本编辑

- UTF-8 byte range。
    
- CJK 插入和删除。
    
- emoji。
    
- combining mark。
    
- surrogate 输入转换。
    
- selection replacement。
    
- undo grouping。
    
- 256 entry 限制。
    
- 4 MiB 限制。
    
- IME commit 一次撤销。
    

### Markdown 转换

- 所有 CommonMark block。
    
- GFM table。
    
- task list。
    
- strikethrough。
    
- autolink。
    
- 四种公式 delimiter。
    
- 转义 dollar。
    
- code 中的公式标记。
    
- raw HTML literal。
    
- reference link。
    
- reference image。
    
- local/remote image。
    
- malformed input 不 panic。
    

### 数学

fixture 至少包含：

- 分数。
    
- 根式。
    
- 上下标。
    
- 积分。
    
- 求和。
    
- 极限。
    
- 矩阵。
    
- cases。
    
- align。
    
- 可伸缩括号。
    
- Greek。
    
- `\mathbb`。
    
- `\mathbf`。
    
- `\operatorname`。
    
- Unicode 数学字符。
    
- 错误公式。
    
- 超长公式。
    

### 文件

- UTF-8 BOM。
    
- CRLF/LF。
    
- mixed newline。
    
- atomic replace。
    
- temp recovery。
    
- config corruption。
    
- invalid UTF-8。
    
- external delete。
    
- self-write watcher ignore。
    
- dirty conflict。
    

### 图片

- PNG/JPEG/WebP 保留原编码。
    
- bitmap 转 PNG。
    
- hash 去重。
    
- 多图片粘贴。
    
- managed/user file 区分。
    
- move to trash。
    
- undo restore。
    
- redo re-trash。
    
- startup restore。
    
- startup purge。
    
- path traversal。
    
- remote 不下载。
    
- oversized image placeholder。
    

## 29.2 Property tests

使用 property-based tests 验证：

- 任意 Unicode TextDelta 不破坏 UTF-8。
    
- undo 后恢复原文。
    
- redo 后恢复编辑后文本。
    
- 任意图片事务最终与引用状态一致。
    
- 任意窗口几何变化后窗口仍在至少一个工作区内。
    
- 任意配置缺字段时使用默认值。
    
- Markdown AST 转换不 panic。
    

## 29.3 Fuzz

建立 fuzz target：

```
fuzz_markdown_to_owned_ast
fuzz_render_tree_builder
fuzz_managed_asset_scanner
fuzz_local_path_normalizer
fuzz_text_delta
```

定时 workflow 运行，不阻塞普通 PR 的快速 CI。

## 29.4 Golden tests

数学和预览使用固定测试字体进行 golden 测试：

- Light。
    
- Dark。
    
- 100% DPI。
    
- 150% DPI。
    
- 200% DPI。
    

允许极小 anti-aliasing tolerance，不允许大范围 pixel mismatch。

## 29.5 手工 Windows 11 验收

系统：

- 当前受支持 Windows 11 生产版本。
    
- 前一个受支持 Windows 11 版本。
    
- 100%、125%、150%、200% DPI。
    
- 单显示器。
    
- 双显示器同 DPI。
    
- 双显示器混合 DPI。
    
- 显示器位于主屏左侧。
    
- 显示器位于主屏上方。
    
- 运行中断开外接显示器。
    
- Sleep/resume。
    
- Remote Desktop reconnect。
    

输入法：

- 微软拼音。
    
- 微信输入法。
    

窗口：

- 左/右/上 dock。
    
- hover 展开。
    
- 失焦收起。
    
- typing guard。
    
- Esc。
    
- 手动按钮。
    
- topmost。
    
- 透明度 70、85、96、100。
    
- Light/System/Dark。
    
- Windows 主题运行时切换。
    
- Close to tray。
    
- tray Quit。
    

文件：

- 外部 VS Code 编辑。
    
- 外部 Notepad 编辑。
    
- 外部删除。
    
- 外部 invalid UTF-8。
    
- 同目录双击两次。
    
- 不同目录同时运行。
    
- 无写权限目录。
    
- 进程强制终止后的 temp recovery。
    

---

# 30. GitHub Actions

## 30.1 CI workflow

每个 PR 和 main push：

### Windows job

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets --locked
cargo build --release --locked --target x86_64-pc-windows-msvc
cargo test --features richedit-fallback --no-run
cargo deny check
```

### Portable-core job

在 Linux runner 上只构建：

```
stickymd-core
stickymd-render
```

目的不是发布 Linux app，而是防止平台无关代码被 Win32 污染。

### Tests

- 上传失败日志。
    
- 上传 math/preview diff。
    
- 缓存 Cargo registry 与 target，但 cache key 包含 Cargo.lock。
    
- 不缓存 release artifact 作为最终发布物。
    

## 30.2 Scheduled workflow

每周：

- `cargo deny check advisories`
    
- `cargo update --dry-run`
    
- fuzz smoke test。
    
- Debug sanitizer 或 Miri 测试平台无关核心。
    
- dependency license report。
    

不得自动把 dependency 更新直接合并到 main。

## 30.3 Release workflow

触发：

```
tag: v*
```

步骤：

1. 验证 tag 与 Cargo version 一致。
    
2. `cargo test --locked`。
    
3. `cargo deny check`。
    
4. Release build。
    
5. 检查 EXE manifest。
    
6. 运行 smoke test。
    
7. 打包 portable ZIP。
    
8. 生成 SHA-256 checksums。
    
9. 生成 dependency/license notice。
    
10. 生成 SBOM。
    
11. 生成 GitHub artifact provenance/attestation。
    
12. 创建 draft release。
    
13. 人工完成 Windows 11 验收后发布。
    

## 30.4 Release 包

```
StickyMD-v1.0.0-windows-x64-portable.zip
├─ StickyMD.exe
├─ README.txt
├─ LICENSE.txt
├─ THIRD_PARTY_NOTICES.txt
└─ licenses\
   ├─ SIL-OFL-1.1.txt
   └─ KaTeX-fonts-NOTICE.txt
```

不得预创建用户 `note/`，由首次运行创建。

另外发布：

```
StickyMD-v1.0.0-SHA256SUMS.txt
StickyMD-v1.0.0-symbols.zip
SBOM.spdx.json
```

v1 不提供：

- MSI。
    
- MSIX。
    
- Microsoft Store。
    
- 自动更新器。
    
- 管理员安装。
    
- Program Files 安装。
    

代码签名可以后续加入，但不阻塞开源 v1。

---

# 31. Agent 分阶段实施计划

Agent 必须逐阶段工作。每一阶段通过 gate 后才能进入下一阶段。

## Phase 0：仓库与工程规则

### 目标

建立可编译 workspace 和治理文档。

### 交付

- Cargo workspace。
    
- rust-toolchain。
    
- LICENSE。
    
- SPEC。
    
- ARCHITECTURE。
    
- ROADMAP。
    
- AGENTS。
    
- CI。
    
- cargo-deny。
    
- Windows manifest。
    
- 最小空窗口。
    

### Gate

- Windows release build 成功。
    
- core/render 在 Linux 编译。
    
- fmt/clippy/test 通过。
    
- 没有 WebView/Tauri/Tokio 依赖。
    

---

## Phase 1：技术风险 Spike

这一阶段不追求 UI 美观。

### Spike A：窗口与 framebuffer

验证：

- winit 窗口。
    
- softbuffer present。
    
- tiny-skia 绘制。
    
- resize。
    
- 透明度。
    
- DWM round corner。
    
- 100/150/200% DPI。
    
- idle 时零持续 redraw。
    

### Spike B：cosmic-text 编辑与 IME

验证：

- 中英文输入。
    
- caret。
    
- selection。
    
- clipboard。
    
- Microsoft Pinyin。
    
- WeChat IME。
    
- preedit。
    
- candidate position。
    
- undo commit boundary。
    

### Spike C：Comrak + RaTeX

验证：

- 四种 delimiter。
    
- CommonMark/GFM。
    
- RaTeX parse/layout。
    
- KaTeX fonts。
    
- formula error。
    
- Light/Dark。
    
- 公式 raster。
    

### Spike D：Portable 数据可靠性

验证：

- canonical directory。
    
- same-directory mutex。
    
- wake existing instance。
    
- atomic replace。
    
- file watcher。
    
- temp recovery。
    
- config atomic save。
    

### Gate

生成：

```
docs/SPIKE_REPORT.md
```

必须记录：

- 实测内存。
    
- 实测启动时间。
    
- IME 通过/失败项。
    
- RaTeX 热路径方案。
    
- unsafe API 列表。
    
- 未解决风险。
    

没有通过 Spike C，不得开始完整 Preview。

---

## Phase 2：Core document 与 Undo

### 实现

- StringTextStore。
    
- TextDelta。
    
- cursor/selection snapshot。
    
- UndoManager。
    
- 256/4 MiB 限制。
    
- editing commands。
    
- UTF-8 与 newline。
    
- state reducers。
    

### Gate

- Unicode property tests。
    
- undo/redo roundtrip。
    
- 1 MiB 编辑 benchmark。
    
- core 无 unsafe。
    

---

## Phase 3：源码编辑器

### 实现

- cosmic-text layout。
    
- source viewport。
    
- scrolling。
    
- selection。
    
- caret。
    
- common shortcuts。
    
- clipboard text。
    
- script-level fonts。
    
- IME。
    
- autosave dirty events。
    
- source-only UI。
    

### Gate

- 微软拼音手工矩阵。
    
- 微信输入法手工矩阵。
    
- 100/150/200% DPI。
    
- 输入 p95 达标。
    
- 输入期间无 auto-hide。
    

如果失败，先进行两轮修复；之后才允许启动 RichEdit fallback 评审。

---

## Phase 4：文件可靠性

### 实现

- note directory。
    
- writable check。
    
- note load。
    
- atomic save。
    
- autosave scheduler。
    
- Ctrl+S。
    
- config。
    
- crash temp。
    
- watcher。
    
- conflict banner。
    
- external reload/delete。
    
- same-directory single instance。
    

### Gate

- 故障注入测试。
    
- kill-process recovery。
    
- 双实例测试。
    
- 无写权限测试。
    
- config corruption 测试。
    

---

## Phase 5：Markdown Owned AST

### 实现

- Comrak option 固定。
    
- Arena → owned AST。
    
- raw HTML literal。
    
- link scheme。
    
- image destination normalization。
    
- math node extraction。
    
- source ranges。
    

### Gate

- Markdown fixture 全通过。
    
- fuzz 10 分钟无 panic。
    
- 无 HTML 执行路径。
    
- 无网络依赖。
    

---

## Phase 6：原生 Preview

### 实现

- RenderTree。
    
- paragraph layout。
    
- headings。
    
- emphasis。
    
- lists。
    
- quotes。
    
- tables。
    
- code。
    
- links。
    
- task list。
    
- selection/copy。
    
- viewport culling。
    
- Source/Preview/Split。
    
- generation scheduler。
    

### Gate

- 20 KiB 与 100 KiB preview benchmark。
    
- split 固定 50/50。
    
- Preview 可选文字。
    
- 过期结果不覆盖最新结果。
    
- 无持续 redraw。
    

---

## Phase 7：RaTeX 正式集成

### 实现

- inline/display math。
    
- layout cache。
    
- raster cache。
    
- DPI/theme key。
    
- error rendering。
    
- source copying。
    
- font notices。
    
- 移除 PNG encode/decode 热路径。
    

### Gate

- math golden tests。
    
- 公式语法 fixture。
    
- 内存 cache budget。
    
- Dark/Light。
    
- 200% DPI。
    
- release 包包含 OFL notice。
    

---

## Phase 8：图片与资源事务

### 实现

- clipboard file list。
    
- bitmap PNG。
    
- encoded image preservation。
    
- SHA-256 naming。
    
- dedup。
    
- lazy preview。
    
- size limits。
    
- managed scanner。
    
- `.trash`。
    
- Undo/Redo asset effects。
    
- startup reconciliation。
    
- export assets。
    

### Gate

- paste/undo/redo。
    
- crash between asset write and text save。
    
- startup referenced-trash restore。
    
- 用户图片不删除。
    
- remote 图片零请求。
    
- cache ≤16 MiB。
    

---

## Phase 9：Dock、托盘与视觉

### 实现

- tray。
    
- close to tray。
    
- Always on top。
    
- left/right/top docking。
    
- state machine timers。
    
- animation。
    
- multi-monitor。
    
- display disconnect。
    
- theme control。
    
- opacity slider/input。
    
- fixed corner/shadow。
    
- manual hide。
    

### Gate

- 全部窗口手工矩阵。
    
- mixed DPI。
    
- no off-screen recovery failure。
    
- focus/IME guard。
    
- idle CPU gate。
    

---

## Phase 10：性能优化

优化顺序固定：

1. 查找持续 redraw。
    
2. 查找重复 framebuffer allocation。
    
3. 查找无界 cache。
    
4. 查找过期 preview 持有。
    
5. 查找字体重复加载。
    
6. 查找图片过早 decode。
    
7. 查找不必要文档 copy。
    
8. 查找线程栈与后台线程。
    
9. 查找 Win32 resource leak。
    
10. 最后才考虑 allocator。
    

禁止“凭感觉优化”。

每项优化必须有：

```
before benchmark
patch
after benchmark
regression test
```

### Gate

达到第 28 节 release hard limits。

---

## Phase 11：Release Candidate

### 完成

- README。
    
- screenshots。
    
- portable instructions。
    
- security policy。
    
- third-party notices。
    
- release workflow。
    
- SBOM。
    
- checksums。
    
- Windows 11 手工验收。
    
- clean VM 测试。
    
- 中文文档。
    
- 英文基本 README。
    

### Gate

Definition of Done 全部通过。

---

# 32. Agent 工作规则

仓库 `AGENTS.md` 应包含以下总控要求：

## 32.1 必须遵守

1. 开始任何 phase 前阅读 `SPEC.md` 和 `ARCHITECTURE.md`。
    
2. 不实现当前 phase 之外的产品功能。
    
3. 不引入 WebView、Tauri、Tokio 或 GPU 框架。
    
4. 新增 dependency 前先检查：
    
    - 许可证。
        
    - transitive dependency。
        
    - binary size。
        
    - MSRV/toolchain。
        
    - 是否已有依赖能完成。
        
5. 不在 runtime path 使用无解释的 `unwrap()` 或 `expect()`。
    
6. 不在 platform/windows 以外写 unsafe。
    
7. 每个 unsafe block 写 `SAFETY` 说明。
    
8. 所有后台结果使用 generation 防止 stale commit。
    
9. 所有文件写入采用原子替换。
    
10. 不静默吞掉保存错误。
    
11. 不自动删除用户文件。
    
12. 不访问网络。
    
13. 不执行 raw HTML。
    
14. 不改变 Markdown parser 语义。
    
15. 不自己实现 TeX。
    
16. 不以“先做功能、以后优化”为由引入持续 redraw 或无界 cache。
    
17. 每个状态机 transition 必须可测试。
    
18. 每个 phase 完成后更新 ROADMAP 和 SPIKE/benchmark 结果。
    

## 32.2 Agent 停止条件

遇到以下情况时，Agent 应停止扩展实现并记录问题，而不是绕过规格：

- RaTeX 无法覆盖关键公式。
    
- winit/cosmic-text 无法稳定处理指定 IME。
    
- softbuffer 与 layered window 存在不可接受冲突。
    
- 原子保存不能满足数据安全要求。
    
- 新依赖许可证不兼容。
    
- 内存超过 hard gate 且无法定位原因。
    
- 实现需要引入 WebView 或 JS。
    
- 产品需求与 Non-Goals 冲突。
    

输出：

```
docs/RISK-<topic>.md
```

内容包括：

- 重现步骤。
    
- 根因。
    
- 已尝试方案。
    
- 数据。
    
- 可选路径。
    
- 对规格的影响。
    

---

# 33. Non-Goals

以下功能明确不属于 v1，并应默认拒绝相关 PR：

## 文档管理

- New。
    
- Open。
    
- Recent files。
    
- 多文档。
    
- 多标签页。
    
- 文件树。
    
- Workspace。
    
- Vault。
    
- 数据库。
    
- 搜索全部笔记。
    
- 标签。
    
- 双向链接。
    
- Backlink。
    
- Graph view。
    

## 云与账户

- 登录。
    
- 云同步。
    
- WebDAV。
    
- Git 同步 UI。
    
- OneDrive 集成。
    
- Dropbox。
    
- 账户系统。
    
- 协同编辑。
    

## 编辑器膨胀

- WYSIWYG。
    
- Typora 模式。
    
- Vim 模式。
    
- Emacs 模式。
    
- 多光标。
    
- 插件系统。
    
- 宏。
    
- 命令面板。
    
- LSP。
    
- 语法高亮。
    
- 代码执行。
    
- Terminal。
    
- Mermaid。
    
- PlantUML。
    
- LaTeX 文档编译。
    
- PDF 导出。
    
- HTML 导出。
    

## AI 与网络

- AI 写作。
    
- AI 总结。
    
- AI 补全。
    
- 遥测。
    
- Analytics。
    
- 崩溃自动上传。
    
- 远程图片下载。
    
- 自动更新。
    
- 广告。
    
- 在线服务。
    

## 视觉扩展

- 自定义 CSS。
    
- 主题市场。
    
- 背景图片。
    
- Acrylic/Mica。
    
- 动态壁纸。
    
- 可配置动画。
    
- 可配置圆角。
    
- 可配置阴影。
    
- 可拖动 split divider。
    

## 系统扩展

- MSI/MSIX。
    
- Microsoft Store。
    
- Windows 10。
    
- ARM64 v1。
    
- Linux app v1。
    
- macOS app v1。
    
- 自动开机启动 v1。
    
- 全局快捷键 v1。
    

---

# 34. Definition of Done

StickyMD v1 只有在以下条件全部满足时才可发布。

## 产品

- 只有一个固定 `./note/note.md`。
    
- 同目录单实例。
    
- 不同目录可同时运行。
    
- Source/Preview/Split 正常。
    
- Preview debounce 为 1000 ms。
    
- Ctrl+S、导出、Undo/Redo、剪贴板正常。
    
- Close to tray。
    
- Tray Quit 安全保存。
    

## Markdown/数学

- CommonMark/GFM fixture 通过。
    
- 四种 math delimiter。
    
- RaTeX/KaTeX-compatible math。
    
- raw HTML 不执行。
    
- remote images 不下载。
    
- Preview 可选择复制。
    
- 公式错误不崩溃。
    

## 输入

- 微软拼音通过。
    
- 微信输入法通过。
    
- candidate rect 正确。
    
- composition undo 正确。
    
- 输入期间不自动隐藏。
    
- 100/150/200% DPI。
    

## 文件

- UTF-8/BOM/CRLF 正确。
    
- atomic save。
    
- autosave 650 ms。
    
- temp recovery。
    
- external clean reload。
    
- dirty conflict。
    
- invalid UTF-8 安全处理。
    
- config 损坏不影响 note。
    

## 图片

- 文件原编码尽量保留。
    
- screenshot 转 PNG。
    
- hash 去重。
    
- managed/user 图片边界。
    
- undo 恢复图片。
    
- redo 重新删除。
    
- startup reconciliation。
    
- export 重写路径。
    
- remote 零请求。
    

## 窗口

- 左/右/上吸附。
    
- 3 DIP 感应区。
    
- hover 100 ms。
    
- 失焦 700 ms。
    
- hover leave 500 ms。
    
- Esc/manual hide。
    
- 多显示器。
    
- 混合 DPI。
    
- 显示器拔除恢复。
    
- opacity 70–100。
    
- Light/System/Dark。
    
- Always on top。
    

## 工程

- core/render 无 unsafe。
    
- unsafe 均有 SAFETY 注释。
    
- 无 WebView。
    
- 无 Tauri。
    
- 无 Tokio。
    
- 无网络依赖。
    
- cargo fmt。
    
- clippy `-D warnings`。
    
- tests。
    
- cargo deny。
    
- release provenance。
    
- MIT 与 OFL notice。
    
- 达到性能 hard gate。
    
- Clean Windows 11 VM 可运行。
    

---

# 35. 第一条 Agent 总控指令

将下面内容放入 `AGENTS.md` 顶部：

> 你正在实现 StickyMD，一个 Windows 11 x64、纯 Rust 主体、无 WebView 的单文件 Markdown 桌面便签。  
> 本仓库以 `SPEC.md` 为产品真相来源，以 `ARCHITECTURE.md` 为技术边界，以 `ROADMAP.md` 为阶段计划。  
> 任何未写入 SPEC 的产品功能默认禁止实现。  
> 你的首要任务不是增加功能，而是在数据安全、IME 正确、输入延迟、内存占用和 Markdown/数学渲染正确性之间保持最小且可验证的实现。  
> 必须逐 phase 工作，每个 phase 先写测试或验收条件，再实现，再给出测量结果。  
> 不允许使用 WebView、Tauri、Electron、JavaScript runtime、Tokio 或通用 GPU UI 框架。  
> 不允许自行实现 Markdown parser 或 TeX layout。  
> Comrak 定义 Markdown 语义，RaTeX 定义数学语义。  
> 平台无关 crates 禁止 unsafe；Win32 调用只能存在于 `stickymd-win/src/platform/windows/` 与经批准的 RichEdit fallback。  
> 任何保存、图片删除和冲突处理都必须优先保证用户文本与文件不被静默丢失。  
> 当技术事实与预期不符时，停止扩展实现，创建风险文档并提供数据，不得用增加依赖或改变产品语义的方式偷偷绕过。

---

# 36. 最终架构结论

StickyMD v1 的正式技术路线冻结为：

```
Windows 11 x64
Rust 1.97.1
winit
cosmic-text
tiny-skia
softbuffer
Comrak
RaTeX
tray-icon
notify
少量 windows crate Win32 adapter
```

核心边界：

```
无 WebView
无 HTML renderer
无 JavaScript
无 Electron
无 Tauri
无 async runtime
无数据库
无网络
无插件
无文档管理
```

最终产品应当表现为：

> 一张始终可靠、几乎没有等待、可以渲染严肃数学内容、不会逐渐膨胀成另一款知识管理软件的桌面纸片。


# StickyMD Phase 0 — Repository Governance & Architecture Documentation Initialization

你现在位于 **StickyMD** 本地 Git 仓库根目录。

你的任务是执行 **Phase 0：Repository Governance & Architecture Documentation Initialization**。

本阶段的唯一目标是：

> 在任何正式 Rust 功能代码出现之前，建立 StickyMD 的仓库治理骨架、Agent 执行规则、工程宪法、术语体系、产品边界、架构蓝图、状态/数据责任、验证入口与文档投影关系。

本阶段不是产品开发阶段。

**禁止实现编辑器、窗口、Markdown parser、数学渲染、文件保存、托盘、贴边隐藏或任何运行时功能。**

---

# 0. 执行原则

## 0.1 先检查现状

开始编辑前必须执行并理解：

```
git status --short
git branch --show-current
git log -5 --oneline
```

然后递归查看当前仓库：

```
find . -maxdepth 3 -type f | sort
```

Windows 环境无法使用 `find` 时使用等价 PowerShell。

如果仓库不是空仓库：

1. 不得删除已有用户文件。
    
2. 不得覆盖已有设计而不先比较。
    
3. 若已有同名治理文档，先读取、合并并保留有价值内容。
    
4. 若已有内容与本 Prompt 的已批准骨架发生冲突：
    
    - 不得自行决定骨架修改；
        
    - 建立 `docs/report/phase-00-conflict-report.md`；
        
    - 说明冲突、现状、影响和建议；
        
    - 对不冲突部分继续完成。
        
5. 不得 reset、clean、rebase 或覆盖用户未提交修改。
    

---

# 1. 权威来源与优先级

本次任务的设计权威优先级为：

1. 本 Prompt 中明确写出的 USER 决策。
    
2. 本 Prompt 附录中的《工程宪法》。
    
3. 本 Prompt 中定义的 StickyMD 冻结产品边界。
    
4. 现有仓库中已经获得 USER 明确批准且不冲突的设计文档。
    
5. Agent 自己的工程判断。
    

Agent 自己的判断不得覆盖前四项。

---

# 2. 参考仓库原则

架构组织可参考：

```
github:Develata/Deve-Notebook
```

只参考其治理思想：

- 根目录 `AGENTS.md` 是 Agent 总入口。
    
- 更窄目录的 `AGENTS.md` 可以覆盖父规则。
    
- `docs/plan/` 是工程骨架和工程合同的唯一权威文档树。
    
- `docs/features/` 是用户可见行为的投影。
    
- `docs/acceptance-cases/` 是验证合同的投影。
    
- `docs/report/` 是有时间属性的分析证据，不是长期权威。
    
- `docs/adr/` 记录重要决策历史，但 ADR 不凌驾于当前 `docs/plan/`。
    
- 实现未来必须遵循：
    

```
docs/plan
    ↓
projection docs
    ↓
code
```

代码不得反向成为架构真相源。

不要复制 Deve-Notebook 的业务内容、ledger 模型、网络模型、Web 架构或与 StickyMD 无关的复杂度。

StickyMD 必须比 Deve-Notebook 简洁得多。

---

# 3. 本阶段严格 Scope

本阶段允许：

- 创建治理 Markdown。
    
- 创建架构 Markdown。
    
- 创建目录级 `AGENTS.md`。
    
- 创建 `README.md`。
    
- 创建 MIT `LICENSE`。
    
- 创建 `.gitignore` / `.gitattributes`。
    
- 创建 GitHub PR template。
    
- 创建文档目录。
    
- 创建 ADR template。
    
- 创建 Phase 0 report。
    
- 使用 Markdown Mermaid/ASCII 图描述架构。
    
- 检查 Markdown 链接。
    
- 创建极少量纯文本辅助脚本，仅在确有必要用于文档验证时。
    

本阶段禁止：

- 创建正式 Cargo workspace。
    
- 添加 Rust crate。
    
- 添加 `src/main.rs`。
    
- 添加运行时代码。
    
- 下载 crate。
    
- 创建 UI prototype。
    
- 做技术 spike。
    
- 调用 Win32 API。
    
- 初始化 winit。
    
- 初始化 softbuffer。
    
- 初始化 cosmic-text。
    
- 集成 Comrak。
    
- 集成 RaTeX。
    
- 写 editor。
    
- 写 autosave。
    
- 写文件 watcher。
    
- 写 tray。
    
- 写 docking。
    
- 写 image GC。
    
- 做性能 benchmark 实现。
    
- 添加 GitHub Release workflow。
    
- 添加任何尚未验证的 crate 版本。
    
- 为未来“可能需要”提前引入大量框架。
    

**Phase 0 是纯 contract-first 阶段。**

---

# 4. StickyMD 已冻结的系统本体

你必须基于以下本体写文档，不得重新发明产品定位。

StickyMD 是：

> 一个极致优化、常驻 Windows 11 桌面的、以 Rust 为主体实现的、近乎零负担的 Markdown 临时草稿纸。

它不是：

- Markdown IDE。
    
- 通用 Markdown 编辑器。
    
- Obsidian。
    
- Typora。
    
- 知识管理软件。
    
- 第二大脑。
    
- 文档管理器。
    
- 文件管理器。
    

核心本体对象只有：

```
Note
Document Text
Preview
Managed Image Asset
Runtime Config
Window Placement
Editor Session
```

首版永远只编辑：

```
./note/note.md
```

一个程序目录就是一张便签的身份边界。

---

# 5. v1 已冻结产品事实

以下均视为 USER 已批准的骨架级决策。

Agent 不得在 Phase 0 中重新讨论是否需要这些能力，只需忠实建模。

---

## 5.1 平台

```
Windows 11 x64 only
```

v1 不支持：

- Windows 10。
    
- Linux。
    
- macOS。
    
- ARM64。
    

但平台依赖必须被隔离，为未来替代留出干净边界。

---

## 5.2 技术方向

正式实现阶段的预定技术方向：

```
Rust
winit
cosmic-text
tiny-skia
softbuffer
Comrak
RaTeX
少量 windows crate Win32 adapter
```

这些是当前批准的**架构方向**，不是本阶段要求安装的 dependency。

精确 crate 版本必须在后续技术验证阶段重新核实后锁定。

不得在 Phase 0 编造版本号。

禁止架构方向：

```
Electron
Tauri
WebView
WebView2
CEF
HTML/CSS UI
JavaScript runtime
Tokio/general async runtime
通用 GPU UI framework
数据库
网络层
插件系统
```

---

## 5.3 文件身份

永远只有一个 canonical working note：

```
<program-dir>/note/note.md
```

运行时目录：

```
<program-dir>/
├─ StickyMD.exe
└─ note/
   ├─ note.md
   ├─ config.toml
   ├─ images/
   └─ .trash/
```

纯 portable。

无 MSI/MSIX。

如果程序目录不可写：

```
当前目录不可写，请将程序移动到有写权限的文件夹。
```

然后退出。

禁止 fallback 到：

```
AppData
LocalAppData
Documents
Registry
```

---

## 5.4 实例模型

同一 canonical 程序目录：

```
只能运行一个实例
```

第二实例：

```
唤醒第一实例
→ 自己退出
```

不同程序目录：

```
可以同时运行
```

因此：

```
D:\NoteA\StickyMD.exe
D:\NoteB\StickyMD.exe
```

是两张完全独立便签。

---

## 5.5 编辑模式

三个视图：

```
Source
Preview
Split
```

### Source

看到原始 Markdown：

```
**bold**
$x^2$
```

### Preview

纯只读渲染。

允许：

- selection。
    
- copy。
    
- scroll。
    
- 点击允许链接。
    

### Split

```
Source | Preview
```

固定 50/50。

禁止拖分隔线。

---

## 5.6 Preview 更新

不是实时 Preview。

编辑产生：

```
preview_dirty = true
```

Split 已打开：

```
最后一次输入
→ 1000 ms debounce
→ parse/render
```

持续输入不断重置计时器。

切换到 Preview：

```
立即刷新
```

后台 stale generation 结果不得覆盖最新文档。

---

# 6. Markdown / Math 冻结决策

## 6.1 Markdown

Markdown parser：

```
Comrak
```

正式方言方向：

```
CommonMark
+ GFM
+ Comrak math extensions
```

不要自行实现 Markdown parser。

---

## 6.2 Raw HTML

Comrak 可以识别 raw HTML。

StickyMD：

```
不执行
不创建 DOM
不解释 CSS
不运行 JavaScript
```

Preview 中 raw HTML 只按 literal/code 风格展示。

---

## 6.3 数学

数学语法定义：

```
RaTeX / KaTeX-compatible math syntax
```

delimiter：

```
$ ... $
$$ ... $$
\( ... \)
\[ ... \]
```

delimiter 的识别语义继承 Comrak。

不得自行重写 dollar parsing。

不得自行实现 TeX parser/layout。

---

## 6.4 数学字体

数学使用：

```
RaTeX / KaTeX-compatible math fonts
```

不强制 Cambria Math。

---

# 7. 字体冻结决策

纯中文正文优先：

```
仿宋_GB2312
```

fallback：

```
仿宋
FangSong
system CJK fallback
```

Latin 正文：

```
Times New Roman
```

fallback：

```
system serif
```

代码：

```
Consolas
```

fallback：

```
system monospace
```

Math：

```
RaTeX math fonts
```

中英文混排：

```
character/script-level font runs
```

例如：

```
这是 Rust 的 trait 示例
```

需要形成中文和 Latin 两类 font run。

---

# 8. 输入法冻结决策

中文输入法体验是一级需求。

v1 人工验收至少包括：

```
Microsoft Pinyin
WeChat Input Method
```

第一实现：

```
winit IME
+ cosmic-text
+ 自绘 editor
```

RichEdit：

```
只允许作为最后 fallback
```

必须先尝试纯 Rust 路线。

只有 IME composition/caret 无法稳定通过时才能进入 RichEdit fallback 评审。

RichEdit 即使启用，也只能替代 Source 输入控件。

以下仍必须保持 Rust：

- DocumentState。
    
- Markdown。
    
- Preview。
    
- Math。
    
- File model。
    
- Assets。
    
- Window state。
    
- 保存逻辑。
    

---

# 9. 自动保存与 Undo

Autosave：

```
输入 debounce 约 650 ms
失焦立即保存
退出立即保存
Ctrl+S 手动保存
```

不做历史版本。

Undo/Redo：

```
Ctrl+Z
Ctrl+Y
```

仅当前进程。

限制：

```
max entries = 256
max memory = 4 MiB
```

任一先达到即开始淘汰最老历史。

重启后 Undo 历史清空。

---

# 10. 图片冻结决策

## 10.1 粘贴

图片可以直接粘贴到 Markdown。

managed image：

```
./note/images/stickymd-<content-hash>.<ext>
```

最终保存 bytes 计算 SHA-256。

文件名只需要使用足够长的 hash prefix。

---

## 10.2 格式

优先保留原编码：

```
PNG → PNG
JPEG → JPEG
WebP → WebP
```

截图 / bitmap：

```
→ PNG
```

---

## 10.3 GC

只有软件自己管理的：

```
stickymd-*
```

可以自动清理。

用户手工放进 `images/` 的文件永远不得自动删除。

当 Markdown 不再引用 managed image：

```
images/
→ .trash/
```

这是逻辑删除。

不是立即物理删除。

Undo 恢复 Markdown 引用时：

```
.trash/
→ images/
```

Redo 再次删除时重新进入 `.trash/`。

---

## 10.4 永久清理

正常退出：

```
重新检查最新引用
→ 删除确认未引用 managed trash
```

异常退出后的下一次启动：

```
先加载 note.md
→ 恢复被引用 asset
→ 再执行 GC
```

禁止一启动就清空 `.trash/`。

---

## 10.5 Remote 图片

HTTP/HTTPS 图片：

```
不下载
不请求网络
不缓存
```

Preview：

```
显示 alt text + link
```

---

# 11. Export 冻结决策

不要称为：

```
另存为
```

称为：

```
导出
```

快捷键：

```
Ctrl+Shift+S
```

工作文档永远还是：

```
./note/note.md
```

导出例：

```
my-note.md
my-note-assets/
```

只复制当前实际引用的本地图片。

导出的 Markdown 重写图片相对路径。

不导出：

- config。
    
- trash。
    
- 未引用 managed assets。
    

---

# 12. Window 冻结决策

支持：

```
Always on top
left dock
right dock
top dock
auto-hide
hover reveal
manual collapse
Esc collapse
multi-monitor
opacity
tray
```

不支持 bottom dock。

默认行为：

```
edge snap threshold ≈ 12 DIP
collapsed sensor strip = 3 DIP
hover reveal ≈ 100 ms
focus lost collapse ≈ 700 ms
hover-only leave collapse ≈ 500 ms
animation ≈ 140 ms
```

只要正在输入：

```
绝不自动收起
```

但：

```
Esc
手动收起按钮
```

始终可以主动收起。

---

# 13. 多显示器冻结决策

多显示器属于 v1 一级要求。

必须覆盖：

- 混合 DPI。
    
- 负坐标 monitor。
    
- monitor disconnect。
    
- monitor reconnect。
    
- sleep/resume。
    
- 主屏变化。
    

Window config 保存：

```
monitor identity
dock edge
relative offset
DIP size
floating relative position
```

不只保存绝对坐标。

目标显示器不存在：

```
恢复到主显示器
保证完全可见
```

---

# 14. 主题与透明度

Theme：

```
Light
System
Dark
```

首次默认：

```
Light
```

UI 是三态滑块：

```
☀ / computer / moon
```

System 跟随 Windows theme。

Opacity：

```
70–100
integer
default 96
```

slider + 整数输入。

整个窗口透明。

拖动时实时 preview。

只在：

```
slider release
Enter
focus loss
```

写 config。

---

# 15. Tray 生命周期

窗口 close：

```
Hide to tray
```

不退出。

Tray 只有：

```
显示/隐藏
置顶
退出
```

真正退出只能从 tray：

```
退出
```

退出前：

- 保存 note。
    
- flush asset transaction。
    
- 保存 config。
    
- 安全 GC。
    

---

# 16. File reliability 冻结决策

Text：

```
UTF-8 without BOM
```

读取兼容 BOM。

首次创建：

```
CRLF
```

已有文件尽量保持主要 line ending 风格。

保存禁止：

```
truncate + in-place write
```

正式实现必须使用：

```
same-dir temp
flush
atomic replace
```

Crash recovery：

```
note.md.tmp
```

若 temp 有效且更新：

```
让 USER 选择恢复
```

不承诺崩溃时最后约 650 ms 的所有字符一定持久化。

但不得产生半写文件。

---

# 17. 外部文件修改

外部编辑器修改 `note.md`：

### 内存 buffer clean

```
自动 reload
```

### 内存 buffer dirty

进入 conflict：

```
文件已在外部修改
[载入外部] [保留本地]
```

Autosave 暂停。

不得偷偷覆盖任何一边。

---

# 18. 体系结构必须遵循 USER 工程宪法

StickyMD 使用：

> 四层调用架构 + Object Plane。

你必须在 `docs/plan/03_system_architecture.md` 中将 StickyMD 映射到该模型。

---

## 18.1 Interaction Shell

职责：

- Window。
    
- Source/Preview/Split 呈现。
    
- Keyboard/mouse/IME 捕获。
    
- Tray。
    
- Theme/opacity controls。
    
- visual selection。
    
- drag interaction。
    

唯一职责：

```
转译 + 呈现
```

不得直接做：

- 文件写入决策。
    
- asset GC 判断。
    
- Markdown 业务判断。
    
- save conflict 决策。
    
- lifecycle business state 决策。
    

---

## 18.2 Instruction Interface

负责把 UI action 转为 typed intent。

预期 intent 类别至少包括：

```
EditText
Undo
Redo
SaveNow
Export
SetViewMode
SetTheme
SetOpacity
SetAlwaysOnTop
RequestCollapse
RequestShow
RequestQuit
ResolveFileConflict
PasteClipboard
```

本阶段只定义 contract，不写 Rust enum。

---

## 18.3 Flow Coordination

负责：

```
SaveCoordinator
PreviewCoordinator
AssetCoordinator
ConflictCoordinator
RecoveryCoordinator
WindowDockCoordinator
LifecycleCoordinator
```

负责顺序、冲突、失败路径和状态推进。

不得直接绕过 Execution Domain 操作 filesystem。

---

## 18.4 Execution Domain

未来执行能力包括：

```
Markdown parsing
Math parsing/layout
Text shaping
Rasterization
File I/O
Atomic replace
Asset move/delete
Clipboard
File watch
Monitor query
Window platform adaptation
Shell launch
Config serialization
```

环境依赖通过 adapter 进入。

---

## 18.5 Object Plane

至少明确：

```
doc::text
doc::snapshot
doc::generation
preview::owned_ast
preview::render_tree
math::display_list
asset::managed_image
asset::trash_entry
config::runtime
window::placement
file::note_md
file::config_toml
```

Object Plane 是对象层，不是第五调用层。

---

# 19. Single Source of Truth

文档必须明确以下 authority。

## 19.1 Note

程序未运行时：

```
./note/note.md
```

是 durable canonical representation。

程序运行时：

```
DocumentState
```

是唯一 authoritative working state。

磁盘发生外部变化：

```
External Fact
```

必须通过 reconcile 流程进入 DocumentState。

不得让：

```
UI text
Preview text
Disk text
DocumentState
```

同时成为平级 authority。

---

## 19.2 Preview

Preview 永远只是：

```
DocumentState snapshot 的派生 projection
```

Preview 不得反写 source。

---

## 19.3 Managed image reference

Managed asset 是否“需要存在”的真相来自：

```
当前 authoritative DocumentState 中的引用状态
```

Filesystem 只是存储事实。

---

## 19.4 Config

运行时：

```
ConfigState
```

是当前配置 authority。

`config.toml` 是 durable projection。

---

# 20. 本阶段目标仓库结构

在不破坏已有仓库的前提下，将项目收敛为至少以下结构：

```
.
├─ AGENTS.md
├─ README.md
├─ LICENSE
├─ .gitignore
├─ .gitattributes
│
├─ .github/
│  └─ pull_request_template.md
│
└─ docs/
   ├─ AGENTS.md
   ├─ coverage-matrix.md
   │
   ├─ plan/
   │  ├─ AGENTS.md
   │  ├─ 00_engineering_constitution.md
   │  ├─ 01_terminology.md
   │  ├─ 02_positioning_and_scope.md
   │  ├─ 03_system_architecture.md
   │  ├─ 04_runtime_state_model.md
   │  ├─ 05_document_persistence.md
   │  ├─ 06_markdown_math_rendering.md
   │  ├─ 07_editor_and_ime.md
   │  ├─ 08_assets_and_export.md
   │  ├─ 09_windows_shell.md
   │  ├─ 10_performance_reliability.md
   │  └─ 11_testing_and_release.md
   │
   ├─ features/
   │  ├─ AGENTS.md
   │  └─ 00_v1_product_behavior.md
   │
   ├─ acceptance-cases/
   │  ├─ AGENTS.md
   │  └─ 00_v1_acceptance.md
   │
   ├─ overview/
   │  ├─ AGENTS.md
   │  └─ architecture.md
   │
   ├─ adr/
   │  ├─ AGENTS.md
   │  ├─ README.md
   │  └─ 0000-template.md
   │
   ├─ report/
   │  ├─ AGENTS.md
   │  └─ README.md
   │
   ├─ tasks/
   │  ├─ AGENTS.md
   │  └─ phase-00-repository-governance.md
   │
   └─ reference/
      ├─ AGENTS.md
      └─ README.md
```

如果已有更合理但兼容的仓库结构，不需要为了机械一致而大规模移动。

但以下逻辑层必须存在：

```
plan
features
acceptance-cases
overview
adr
report
tasks
reference
```

---

# 21. 根 AGENTS.md 具体要求

根 `AGENTS.md` 必须成为未来 Agent 的第一入口。

至少包含：

## Purpose

用 1–2 段定义 StickyMD。

## Authority Order

明确：

```
Engineering Constitution
↓
docs/plan
↓
features / acceptance / overview
↓
code
```

## Mandatory Agent Workflow

未来任何：

- implementation。
    
- bug fix。
    
- architecture review。
    
- dependency change。
    
- code generation。
    
- refactor。
    

必须依次：

1. 阅读最近适用的 `AGENTS.md`。
    
2. 阅读 `docs/plan/00_engineering_constitution.md`。
    
3. 阅读 `docs/plan/01_terminology.md`。
    
4. 找到对应 plan chapter。
    
5. 确认：
    
    - boundary。
        
    - authority。
        
    - state transition。
        
    - failure path。
        
    - verification。
        
6. 阅读对应 feature/acceptance。
    
7. 判断 plan 是否需要改变。
    
8. 若是骨架级改变，停止实施，先提交 report 请求 USER 批准。
    
9. 只有 contract 清晰后才实现。
    
10. 实现后运行 targeted tests。
    
11. review boundary drift。
    
12. 最后运行适用 baseline。
    
13. 不得 push remote，除非 USER 明确要求。
    

## Architecture Change Rule

必须清楚写：

> 已有代码与 plan 冲突时，默认判定为 implementation drift，而不是修改 plan 迁就代码。

但：

> 如果 plan 被事实证伪，必须创建分析报告并请求 USER 批准骨架修改。

## File Cohesion

参考 Deve-Notebook 的治理方式加入：

```
~250 lines = soft architecture warning
~500 handwritten lines = hard review threshold
```

不是机械拆文件规则。

测试文件可合理超出。

## plan_ref

未来正式 Rust 业务 module 应使用：

```
//! plan_ref: docs/plan/<chapter>.md#<stable-anchor>
```

但 Phase 0 不创建 Rust module。

ADR 不作为 `plan_ref` target。

## Forbidden Architecture

明确列出：

```
WebView
Electron
Tauri
runtime network client
database
plugin system
general async runtime
cross-layer filesystem calls
business logic in UI shell
```

---

# 22. docs/AGENTS.md

必须说明各文档树职责：

```
docs/plan/
    authoritative engineering contract

docs/features/
    user-visible product behavior projection

docs/acceptance-cases/
    verification contract

docs/overview/
    readable architecture projection

docs/adr/
    decision history, non-authoritative against current plan

docs/report/
    dated analysis/evidence, non-authoritative

docs/tasks/
    phase implementation plans

docs/reference/
    external technical references, never overrides plan
```

并明确：

```
不要在 feature 文档重新定义架构。
不要在 acceptance 文档发明产品需求。
不要在 report 中建立永久权威。
```

---

# 23. docs/plan/AGENTS.md

必须定义 plan 文档格式。

每章顶部统一：

```
# <filename> - <title>

## Metadata

- `Layer`: Foundation | Architecture | Runtime | Capability | Verification
- `Status`: Governing Rule | Approved Contract | Draft
- `Version`: 0.1.0
- `Last Review`: 2026-08-19
- `Scope`: ...
```

每个 plan chapter 至少回答与该章节适用的：

```
Purpose
Boundary
Owned Objects
Inputs
Outputs
State Changes
Failure Paths
Configuration
Lifecycle
Extension / Replacement Points
Performance Critical Paths
Verification
Non-Goals
```

如果某项不适用，要明确写：

```
Not applicable
```

而不是省略导致语义不明确。

---

# 24. 工程宪法落盘要求

创建：

```
docs/plan/00_engineering_constitution.md
```

正文必须以 USER 下方给出的宪法为准。

可以在最上方添加 Metadata。

**不得删节 USER 宪法原则。**

不得因为 StickyMD 小而删除通用治理条款。

可以：

- 修复 Markdown 排版。
    
- 统一 heading 层级。
    
- 修复明显转义符。
    
- 将 `\` 强制换行转换成正常 Markdown 换行。
    

不得：

- 改变语义。
    
- 改变优先级。
    
- 重写为 Agent 自己的简化版本。
    

---

# 25. 01_terminology.md

必须明确至少以下术语。

每个术语包含：

```
Definition
Authority
Not equivalent to
Lifetime
```

术语：

```
StickyMD
Program Directory
Note Directory
Canonical Note
DocumentState
Document Snapshot
Generation
Dirty
Saved Generation
External File Fact
Conflict
Recovery Candidate
Managed Asset
User Asset
Trash Asset
Preview
Source View
Preview View
Split View
Docked
Collapsed
Floating
Monitor Identity
Runtime Config
Durable Config
Interaction Shell
Instruction Interface
Flow Coordination
Execution Domain
Object Plane
Adapter
Architecture Change
Implementation Drift
```

重点防止：

```
Preview == Document
Disk file == Runtime working state
Image directory == asset authority
UI state == business authority
```

---

# 26. 02_positioning_and_scope.md

必须清楚回答：

- 系统为什么存在。
    
- 系统本体是什么。
    
- 一张便签模型。
    
- portable directory identity。
    
- Windows 11 only。
    
- v1 scope。
    
- Non-Goals。
    
- 优先级。
    
- “极简”是什么意思。
    
- “低内存”为什么不是高于正确性。
    
- 什么情况下功能请求应该被拒绝。
    

必须明确产品优先级继承宪法：

```
正确性 / 功能实现
>
可用性 / 使用体验
>
根基兼容性
>
可维护性 / 可诊断性
>
性能
>
内存占用
>
外存占用
>
其它
```

同时在 StickyMD 语境下解释：

低内存是重要目标，但：

```
不能为了省 3 MB 内存破坏 IME
不能为了减少代码破坏 atomic save
不能为了性能绕过 conflict model
```

---

# 27. 03_system_architecture.md

这是 Phase 0 最重要的技术文档之一。

必须完成：

## Four Layers + Object Plane

给出图：

```
User
  ↓
Interaction Shell
  ↓
Instruction Interface
  ↓
Flow Coordination
  ↓
Execution Domain
  ↔
Object Plane
```

必须定义禁止跨层关系。

必须给出至少 5 个完整调用链实例：

### 输入文字

```
keyboard/IME
→ shell event
→ EditText intent
→ edit coordination
→ document capability
→ doc::text state delta
```

### Autosave

```
Document dirty
→ save schedule
→ save coordinator
→ atomic persistence
→ file::note_md
```

### Preview

```
dirty generation
→ preview coordinator
→ Comrak/RaTeX capability
→ render projection
```

### Image Paste

```
clipboard action
→ PasteClipboard intent
→ asset coordinator
→ image persistence
→ text delta
→ managed asset relation
```

### Dock Hide

```
focus state
→ window intent/timer
→ WindowDockCoordinator
→ platform adapter
→ window::placement
```

每条必须描述：

- input。
    
- state change。
    
- failure。
    
- authority。
    
- output。
    

---

# 28. 04_runtime_state_model.md

必须定义概念状态模型。

不写 Rust code，但可以使用 Rust-like pseudo type。

至少定义：

```
AppState
DocumentState
PreviewState
SaveState
ConflictState
RecoveryState
AssetState
WindowState
DockState
ImeState
ConfigState
```

重点写清：

- 谁拥有谁。
    
- 谁能修改。
    
- 谁不能修改。
    
- runtime authority。
    
- durable projection。
    
- generation semantics。
    
- stale result rejection。
    

必须定义核心 invariant，例如：

```
Preview never becomes document authority.
External disk change never mutates DocumentState without reconciliation.
Managed asset GC never deletes a user asset.
Stale preview generation never commits.
Autosave cannot overwrite unresolved external conflict.
IME preedit is not canonical document text.
```

---

# 29. 05_document_persistence.md

必须覆盖：

```
./note/note.md
./note/config.toml
UTF-8
BOM handling
CRLF/LF
atomic save
temp recovery
external file modification
conflict
portable write permission
single-instance directory identity
```

必须将失败路径作为一级内容。

不允许只写成功保存流程。

---

# 30. 06_markdown_math_rendering.md

必须覆盖：

```
Comrak authority
CommonMark/GFM
math delimiters
Raw HTML literal behavior
RaTeX authority
KaTeX-compatible scope
fonts
Owned AST projection
Preview debounce
generation
remote image no-network rule
Preview read-only
```

明确：

```
parser semantics belongs to Comrak
math semantics belongs to RaTeX
StickyMD only owns projection/layout integration
```

---

# 31. 07_editor_and_ime.md

必须覆盖：

```
Source editor responsibilities
IME preedit vs commit
Microsoft Pinyin
WeChat IME
caret
selection
undo grouping
font runs
RichEdit fallback governance
```

RichEdit fallback 是：

```
approved contingency
not default architecture
```

必须写清 fallback 启用审批条件。

---

# 32. 08_assets_and_export.md

必须覆盖：

```
managed vs user asset
hash naming
format preservation
screenshot PNG
reference tracking
.trash
undo/redo asset side effect
startup reconciliation
safe GC
remote image
export assets folder
path rewrite
```

核心安全 invariant：

```
StickyMD must never automatically delete a file that it cannot prove it owns.
```

---

# 33. 09_windows_shell.md

必须覆盖：

```
Windows 11 x64
platform adapter boundary
window lifecycle
tray
Always On Top
opacity
theme
dock states
multi-monitor
DPI
monitor identity
second-instance wake
```

必须把：

```
Windows implementation detail
```

与：

```
core behavior contract
```

分离。

---

# 34. 10_performance_reliability.md

不要在本阶段承诺未经测试的精确性能数字为事实。

应分成：

```
Target
Measurement Method
Hard Failure Condition
Future Benchmark Entry
```

至少定义：

- idle CPU 应接近 0。
    
- 无永久 redraw loop。
    
- bounded caches。
    
- bounded undo。
    
- preview worker stale-drop。
    
- no unbounded task/thread growth。
    
- no browser runtime。
    
- file safety 高于性能。
    
- IME correctness 高于内存。
    
- measurement 必须在 Release + Windows 11 下完成。
    

可以把此前性能目标作为：

```
Initial Engineering Targets
```

但必须显式标注：

> 尚未由实际实现验证，不得对外宣传。

---

# 35. 11_testing_and_release.md

Phase 0 只定义未来 contract。

包括：

```
unit
property
fuzz
golden
manual IME
multi-monitor
DPI
file failure injection
memory measurement
release verification
```

v1 发布：

```
portable ZIP
MIT
third-party notices
checksums
```

不创建 release workflow。

---

# 36. features/00_v1_product_behavior.md

这是一份纯用户行为描述。

禁止放内部架构。

按用户视角描述：

- 第一次运行。
    
- 输入 Markdown。
    
- Source。
    
- Preview。
    
- Split。
    
- 数学。
    
- 图片粘贴。
    
- 自动保存。
    
- Undo/Redo。
    
- 导出。
    
- 置顶。
    
- 贴边隐藏。
    
- Tray。
    
- Theme。
    
- Opacity。
    
- 外部修改冲突。
    
- 多显示器。
    

---

# 37. acceptance-cases/00_v1_acceptance.md

必须把功能写成可验证案例。

格式建议：

```
## AC-001 First Launch

### Preconditions
...

### Action
...

### Expected
...

### Failure Signals
...
```

至少建立 acceptance case ID：

```
AC-001 Portable First Launch
AC-002 Source Editing
AC-003 Microsoft Pinyin
AC-004 WeChat IME
AC-005 Autosave
AC-006 Manual Save
AC-007 External Clean Reload
AC-008 External Dirty Conflict
AC-009 Undo Redo
AC-010 Image Paste
AC-011 Managed Image Undo
AC-012 User Image Safety
AC-013 Markdown Preview
AC-014 Math Delimiters
AC-015 Math Error
AC-016 Raw HTML Safety
AC-017 Remote Image No Network
AC-018 Export
AC-019 Left Dock
AC-020 Right Dock
AC-021 Top Dock
AC-022 Input Focus Guard
AC-023 Tray Lifecycle
AC-024 Opacity
AC-025 Theme
AC-026 Same Directory Single Instance
AC-027 Different Directory Multi Instance
AC-028 Monitor Disconnect
AC-029 Mixed DPI
AC-030 Crash Recovery
```

本阶段只定义，不实现测试。

---

# 38. coverage-matrix.md

建立：

```
Plan Contract
↔
Feature Projection
↔
Acceptance Case
↔
Future Code Area
```

例如：

|Plan|Feature|Acceptance|Future Code|
|---|---|---|---|
|`05_document_persistence.md`|autosave/conflict|AC-005/007/008/030|`stickymd-core`, Windows I/O adapter|
|`06_markdown_math_rendering.md`|preview/math|AC-013/014/015/016|`stickymd-render`|
|`07_editor_and_ime.md`|source input|AC-002/003/004/009|editor backend|
|`08_assets_and_export.md`|paste/export|AC-010/011/012/018|asset subsystem|
|`09_windows_shell.md`|docking/tray|AC-019..029|Windows shell|

Future Code 可以是规划名称，不代表已经存在。

---

# 39. ADR

`docs/adr/0000-template.md` 必须至少包含：

```
Title
Status
Date
Context
Decision
Alternatives
Benefits
Costs
Risks
Affected Contracts
Migration
Rollback
Verification
USER Approval
```

ADR 只能解释：

```
why
```

当前架构真相仍由：

```
docs/plan
```

决定。

---

# 40. Phase 0 Task Document

创建：

```
docs/tasks/phase-00-repository-governance.md
```

记录：

```
Goal
Scope
Inputs
Deliverables
Verification
Out of Scope
Completion State
```

完成后将状态写成：

```
Completed
```

但不能声称任何运行时能力已经完成。

---

# 41. README.md

README 必须非常克制。

至少写：

```
StickyMD
一句产品定位
Current status: architecture/governance initialization
Target: Windows 11 x64
License: MIT
```

并提供：

```
Architecture docs
Engineering constitution
Agent instructions
```

链接。

明确：

> 当前仓库尚未进入正式功能实现阶段。

不要放假截图。

不要写尚未实现功能已经可用。

---

# 42. LICENSE

MIT。

如果仓库已有 USER 指定 copyright owner，保留。

否则使用：

```
Copyright (c) 2026 Develata
```

---

# 43. .gitignore

只建立当前合理的基础规则，例如：

```
/target/
*.pdb
*.ilk
*.exe
*.tmp
.DS_Store
Thumbs.db
```

不要把未来真正需要版本控制的配置模板误忽略。

不要因为未来 runtime 会生成 `note/` 就直接忽略所有名为 `note` 的目录，除非代码阶段确认测试策略。

---

# 44. .gitattributes

至少：

```
* text=auto
*.md text eol=lf
*.toml text eol=lf
*.rs text eol=lf
*.yml text eol=lf
*.yaml text eol=lf
```

仓库文本规范使用 LF。

这与运行时 `note.md` 默认 CRLF 是两个不同问题。

---

# 45. PR Template

`.github/pull_request_template.md` 至少要求：

```
Summary
Plan refs
Behavior change
Architecture impact
Failure paths
Verification
Performance impact
New dependencies
USER approval required?
```

并有 checkbox：

```
[ ] I did not change architecture contracts merely to match existing implementation.
[ ] Skeleton-level changes have explicit USER approval.
[ ] New dependencies were justified.
[ ] No cross-layer shortcut was introduced.
```

---

# 46. Engineering Constitution 原文

将以下内容作为：

```
docs/plan/00_engineering_constitution.md
```

的治理正文。

除 Metadata 与 Markdown 排版整理外，不得改变语义。

---

## 0. 适用前提

### 0.1 前提集合

本宪法建立在以下前提同时成立的条件下：

- 管理员 / USER 具备足够高的系统判断质量
    
- 骨架级分析报告具备足够高的事实质量、推理质量与利弊分析质量
    
- 骨架级决策以成熟分析为前提，而不以情绪、惯性或短期便利为依据
    

### 0.2 适用目标

本宪法的目标不是弥补低质量判断，而是在高质量判断成立的前提下，约束工程实现对该判断的忠实执行，从而避免以下结果：

- 系统背离本体
    
- 骨架被局部功能污染
    
- 结构性失稳在实现阶段累积
    
- 既有高质量判断在工程演化中被逐步削弱
    

---

## 1. 总体约束

### 1.1 总纲

系统设计满足以下总约束：

1. 必须先定义总骨架与边界，再定义模块，再定义实现。
    
2. 任何局部功能只能填入既定骨架，不得反向塑造骨架。
    
3. 系统优先保持整体结构长期稳定，而不优先追求局部极致最优。
    
4. 系统允许模块替换与实现迭代，但不允许日常功能迭代以侵蚀方式改写主骨架。
    

### 1.2 固定顺序

任一设计任务必须严格按以下顺序执行：

1. 定义总骨架与边界
    
2. 列出模块清单
    
3. 细化实现逻辑
    

禁止跳过前项直接进入后项。

### 1.3 骨架约束

系统演进时必须满足：

- 骨架不因日常功能迭代而重构
    
- 模块可替换
    
- 实现可迭代
    
- 接口尽量稳定
    
- 状态模型尽量稳定
    
- 数据模型慎改
    
- 配置模型早定
    

### 1.4 优先级排序

当多个目标发生冲突时，系统必须按如下严格优先级排序作取舍：

正确性 / 功能实现

> 可用性 / 使用体验  
> 根基兼容性  
> 可维护性 / 可诊断性  
> 性能  
> 内存占用  
> 外存占用  
> 其它次级因素（如组件复用、工程复杂度控制等）

低优先级目标不得破坏高优先级目标。

### 1.5 根基兼容性在优先级中的定义

“根基兼容性”仅指对系统底层、长期稳定且被广泛接受的外部根基生态的兼容能力，以及对若干可能在一至两年内变化的根基外部参数的适应能力，例如：

- CA 证书体系
    
- 系统网络协议
    
- 时区配置
    
- Git 生态
    
- 编程语言生态及其长期约定
    

### 1.6 示例：RSS 阅读器

对一个纯人类阅读向 RSS 阅读器，即使首版只实现“添加订阅源并阅读文章”，在进入页面与功能实现前，仍必须先确定：

- 订阅源模型
    
- 文章模型
    
- 阅读状态模型
    
- 呈现层与抓取 / 存储的解耦关系
    
- 同步与导入导出的骨架挂接方式
    

否则后续“分类、收藏、OPML、全文缓存、云同步”等能力将倾向于以局部补丁方式侵入系统。

---

## 2. 元约束

### 2.1 完美主义约束

系统设计必须以完美主义作为持续自我审查标准，但不得以追求绝对完美为理由：

- 破坏骨架稳定
    
- 延迟必要落地
    
- 引入无上限复杂化
    

### 2.2 完美主义的作用

完美主义在本宪法中的作用不是驱动系统无限膨胀，而是持续检验以下条件是否仍成立：

- 边界是否干净
    
- 抽象是否稳定
    
- 实现是否统一
    
- 替换点是否明确
    
- 失败路径是否完整
    
- 当前方案是否忠于系统本体
    

### 2.3 结论

完美主义在工程中的合法形式是：

- 严格约束
    
- 持续审查
    
- 持续收敛
    

而不是：

- 失控扩张
    
- 过度设计
    
- 无限推迟实现
    

---

## 3. 骨架构造约束

### 3.1 根基兼容性原则

对于已经形成长期稳定生态、并在系统底层被反复验证数十年的标准、协议、接口习惯、工程范式与工具生态，系统应优先兼容与接入，而不是重复造轮子。

### 3.2 根基对象的判定条件

一个外部生态仅当同时满足以下多数条件时，才应被视为“根基对象”：

- 长期存在
    
- 底层基础
    
- 大规模接受
    
- 稳定运转
    
- 工程上具有历史沉淀
    

### 3.3 兼容与内生化规则

- 对根基对象：优先兼容，不轻易重造
    
- 对尚未形成长期生态、实现简单、替代成本低的能力：优先内嵌、重构或借鉴后纳入自身系统，不依赖外部兼容
    

### 3.4 值得兼容对象的判定条件

下列特征越多，越应兼容：

- 长期稳定
    
- 基础性强
    
- 行业共识强
    
- 自建收益低
    
- 自建维护成本高
    
- 脱离它会损失通用性与生存能力
    

### 3.5 不值得兼容对象的判定条件

下列特征越多，越不应兼容：

- 仍在快速变化
    
- 实现简单
    
- 没有形成深度行业共识
    
- 未来失效概率高
    
- 接入收益小于绑定风险
    

### 3.6 工程实现约束

所有外部环境依赖必须通过兼容层或适配层进入系统。  
核心逻辑不得直接绑定环境细节。

### 3.7 简洁、有力、整洁、统一原则

系统骨架只围绕其本体所必需的核心对象、核心关系与高概率能力轴建立。  
首版可以只实现其中一部分，但骨架不得被首版功能误导，也不得为异质能力提前膨胀。

### 3.8 本体优先规则

系统定义应围绕本体对象、核心关系和能力轴，而不是围绕首个实现版本的功能清单。  
首版功能仅决定实现顺序，不定义系统本体。

### 3.9 抽象规则

抽象只能建立在长期稳定的本体对象、关系与能力边界上，不得建立在以下对象上：

- 首版实现
    
- 具体格式
    
- 局部技术细节
    

### 3.10 平级实现规则

属于同一变化族的具体实现，必须位于同一抽象之下，并以平级模块形式存在。  
新增支持应优先表现为新增模块，而不是侵入既有主流程。

### 3.11 替代规则

系统优先支持模块替代，而不是流程改写。  
实现应当可换，主干不应持续长出局部特判。

### 3.12 插件规则

插件机制不是架构起点，而是平级实现数量与替换需求增长后的自然承载形式。

### 3.13 能力纳入主骨架的三层判定

某能力进入主骨架，当且仅当其同时满足以下判定逻辑：

1. **本体必要性判定**：该能力直接服务于系统核心存在目的，或与之高度相关
    
2. **同域延伸性判定**：该能力属于同一问题域内自然延伸
    
3. **异质侵入性判定**：该能力不会引入一整套新的系统本体、依赖模型与复杂性，从而改变系统性质
    

若第三项不成立，则该能力不得进入主骨架。

### 3.14 统一性规则

系统允许复杂，但复杂必须以统一形式出现。  
统一性至少体现在：

- 相同职责使用相同层级表达
    
- 相同错误使用相同错误模型表达
    
- 相同输入输出使用相同数据模型表达
    
- 相同配置项使用相同命名规范表达
    
- 相同模块使用相同目录结构表达
    
- 相同异步任务使用相同调度接口表达
    

### 3.15 示例：RSS 阅读器

对纯人类阅读向 RSS 阅读器，其本体对象不是“RSS 拉取页面”或“首版文章展示功能”，而是：

- 订阅源
    
- 文章
    
- 阅读过程
    
- 阅读状态
    
- 内容来源
    
- 内容解析
    
- 内容呈现
    
- 内容组织
    

因此，“多订阅分组、分类、书签、检索、全文缓存、云同步”属于同域延伸；“AI 分析、AI 问答、TTS、社交分享”若改变系统性质，则不应纳入主骨架。

---

## 4. 工程蓝图约束

### 4.1 蓝图要求

任何架构输出都必须向下细化到实现逻辑，不允许停留在概念层。

### 4.2 最低完备项

一个可施工的工程蓝图至少必须明确以下内容：

- 模块职责
    
- 调用关系
    
- 输入输出
    
- 状态变化
    
- 错误处理
    
- 配置入口
    
- 生命周期
    
- 扩展点
    
- 替换点
    
- 性能关键路径
    

### 4.3 判定规则

若一个方案只能说明系统大致分层、模块大致存在、目标大致正确，而不能继续下钻到具体调用逻辑与运行过程，则该方案只构成概念草图，不构成工程蓝图。

### 4.4 一致性要求

高层骨架、模块边界与实现逻辑之间必须连续。  
不允许出现：

- 上层抽象与下层实现脱节
    
- 实现阶段临时发明破坏骨架的新结构
    

### 4.5 示例：RSS 阅读器

对“刷新订阅源”这一流程，工程蓝图必须能回答：

- 输入是什么
    
- 输出是什么
    
- 订阅源状态如何变化
    
- 新增文章数如何计算
    
- 网络失败、源格式异常、存储失败如何传播
    
- 抓取间隔、重试次数、超时时间如何进入配置
    
- 解析器、网络层、存储层分别从哪里替换
    
- 性能关键路径位于何处
    

---

## 5. 默认分层模型

### 5.1 四层架构 + 对象层

系统推荐采用“四层架构 + 对象层”的组织方式，并坚持壳核分离。

### 5.2 第一层：交互壳层

职责集合：

- CLI / GUI / Web 界面
    
- 动画、样式、按钮、输入控件
    
- 平台适配
    
- 用户动作捕获
    
- 标准指令转译
    

唯一职责：

**转译 + 呈现**

### 5.3 第二层：指令接口层

职责集合：

- 接收标准化指令
    
- 校验合法性
    
- 识别状态变化
    
- 决定需要调用的能力
    
- 组织返回给交互层的结果结构
    

其映射关系定义为：

- action → intent
    
- intent → state delta
    
- state delta → capability requests
    

### 5.4 第三层：流程协调层

职责集合：

- 任务拆分
    
- 调用顺序规划
    
- 前后依赖协调
    
- 状态推进
    
- 错误回滚或异常中止
    
- 后方任务分发
    

第三层不直接接触第五层对象。

### 5.5 第四层：能力执行层

职责集合：

- 数据计算
    
- 存储
    
- 读取
    
- 网络通信
    
- 文件系统操作
    
- 外部系统调用
    
- 后台处理
    
- 具体算法执行
    

### 5.6 第五层：对象层

对象层不属于骨架分层本身，但必须明确。  
其作用是定义系统中实际操作的最小数据元对象。

典型对象形式包括：

- `.md`
    
- `.pdf`
    
- `.jpg`
    
- `text::line`
    
- `text::byte`
    
- `sql::article`
    
- `sql::feed`
    
- `data::html`
    
- `data::xml`
    
- `data::opml`
    

### 5.7 表现变化与业务变化分离规则

界面变化不天然等于业务状态变化。  
只有在进入内部指令系统后，系统才判断哪些变化属于业务变化，哪些仅属于表现层变化。

### 5.8 示例：RSS 阅读器

当用户点击“打开某篇文章”时：

- 第一层只表达“打开文章”
    
- 第二层判断这是否涉及文章切换、已读状态更新、正文请求等状态变化
    
- 第三层组织具体流程
    
- 第四层执行读取、解析、状态写入
    
- 第五层明确所处理的对象是 `sql::article`、`sql::reading_state`、`data::html` 等
    

---

## 6. 一致性约束

### 6.1 边界封闭原则

一旦骨架划分完成，各层与各模块必须严格遵守职责边界。  
任何新增功能都只能通过既定边界接入，不允许跨层直连、跨模块写入或绕过统一入口。

### 6.2 单一真相源原则

对任何核心对象、核心状态与核心配置，系统必须明确唯一可信来源。  
同一事实不得在多个模块、多个缓存层、多个界面状态中并列作为最终依据。

### 6.3 示例：RSS 阅读器

文章“已读 / 未读”状态只能由统一的阅读状态模型作为真相源。  
界面组件可以缓存显示结果，但不得与数据库状态并列为最终依据。

---

## 7. 运行可靠性约束

### 7.1 失败—观测—验证机制

#### 7.1.1 失败优先定义原则

系统设计时，不能只定义成功路径，还必须优先定义失败路径。  
任何核心流程在进入实现前，都必须明确：

- 失败条件
    
- 失败传播方式
    
- 回退策略
    
- 重试边界
    
- 用户可见结果
    

#### 7.1.2 可观测性内建原则

日志、错误码、状态追踪、关键事件记录与性能指标采集，必须作为架构内建能力进入骨架。  
不能依赖事后补日志。

#### 7.1.3 验证先行原则

任何核心骨架、核心流程、核心边界在进入大规模实现前，都必须有对应验证方式。  
若一个原则无法被验证，则该原则在实现中极易失效。

### 7.2 版本与迁移原则

任何核心接口、核心数据模型、核心配置模型一旦变化，都必须回答：

- 旧版本如何兼容
    
- 现有数据如何迁移
    
- 失败后如何回退
    

凡沉淀到磁盘、数据库、缓存、同步链路或导入导出格式中的结构，都负有版本责任。

### 7.3 幂等与重复执行原则

任何可能被重复触发、重复调用、重复投递、重复恢复执行的核心流程，都必须具备幂等性或显式去重机制。

### 7.4 并发与时序一致性原则

任何核心状态在被多个流程同时读取、修改、同步或回写时，都必须明确：

- 谁先
    
- 谁后
    
- 谁覆盖谁
    
- 冲突如何解决
    

### 7.5 示例：RSS 阅读器

对“刷新订阅源”流程，系统必须同时定义：

- 抓取成功路径
    
- 网络失败、解析失败、部分写入失败路径
    
- 关键状态是否可追踪
    
- 同一文章是否会重复入库
    
- 已读状态是否会被刷新或同步错误覆盖
    
- 批量刷新时的性能瓶颈位置
    

---

## 8. 运行裁量约束

### 8.1 默认保守原则

在信息不足或不确定性较高时，系统默认选择更保守、更稳定、更容易回退的方案，而不是更激进、更复杂、耦合更深的方案。

### 8.2 退出与删除原则

系统不仅要定义对象如何产生和增长，也必须定义对象如何停用、删除、迁移、失效和清理。  
任何可被创建的核心对象，都必须有清晰退出路径。

---

## 9. 启动约束

### 9.1 实现最低原则

系统不得在草案尚未达到最低成熟度之前进入正式动工阶段。  
任何实现都必须建立在最低限度分析与骨架确认已经完成的前提下。

### 9.2 最低成熟条件

进入实现阶段的最低条件是：

- 系统本体已得到基本澄清
    
- 主骨架与主边界已得到初步确认
    
- 明显错误方向与异质能力已完成初步排除
    
- 首版目标与主闭环已明确
    
- 当前实现不会直接污染未来主干
    

### 9.3 探索性实现的限制

探索性验证可以存在，但不得：

- 伪装为正式骨架实现
    
- 未经收敛直接沉淀为长期主干
    

---

## 10. 纠偏与治理约束

### 10.1 证伪—纠偏—审批机制

#### 10.1.1 证伪即退出原则

任何方向、能力轴、抽象、边界划分或骨架判断，一旦经分析被认定为错误、失效或已被证伪，就必须立即退出。  
不得因沉没成本、既有投入、历史惯性或实现便利而继续占据主骨架。

#### 10.1.2 骨架即时纠偏原则

系统在任何阶段都不承认“骨架一旦形成便不得修正”。  
只要管理员 / USER 经分析认定某一方向错误、某一骨架判断被证伪，或某一骨架存在明显不完善漏洞，就必须立即进入骨架纠偏流程。

这里的“立即”定义为：立即启动分析、审查与决策流程，而不是绕过审查直接改动主骨架。

#### 10.1.3 骨架变更审批原则

任何骨架级变更都必须先提交具体分析报告，并由管理员 / USER 阅读后明确批准，方可执行。  
未经审批，不得擅自修改：

- 主骨架
    
- 核心边界
    
- 核心对象关系
    
- 主能力轴
    
- 关键接口结构
    

骨架变更分析报告至少必须包含：

- 变更原因
    
- 当前骨架存在的问题或已被证伪之处
    
- 变更后可获得的收益
    
- 变更后新增的风险与代价
    
- 对既有模块、状态模型、数据模型、配置模型的影响
    
- 是否涉及迁移、兼容、回退与验证
    
- 若不变更，会持续产生什么后果
    

### 10.2 决策复核与反证原则

任何管理员 / USER 的骨架级判断、方向性判断或变更性判断，都不因其审批地位而自动免于复核。  
只要存在充分理由认为该判断可能失误、偏颇、证据不足或忽略了关键代价，就必须提交高质量、客观、完整的复核分析报告，对该判断进行反证性审查。

复核分析报告至少必须包含：

- 当前判断成立的依据
    
- 当前判断可能忽略的问题
    
- 若维持原判断，可获得的收益
    
- 若维持原判断，可能承受的风险与代价
    
- 若修正原判断，可获得的收益
    
- 若修正原判断，可能引入的新问题
    
- 当前争议究竟属于实现问题、模块问题、边界问题还是骨架问题
    

复核的目的不是削弱管理员 / USER 的决策权，而是保证骨架级决策始终接受高质量反证与双面分析，从而持续逼近更高质量的系统判断。

---

## 11. 附录约定

### 11.1 实例附录

正文中仅保留最小必要实例。  
更详细的 RSS 阅读器映射说明应移入附录单独维护。

---

# 47. Phase 0 自检

所有文件完成后执行文档自检。

至少：

```
git diff --check
git status --short
```

检查：

- broken relative links。
    
- 同一术语多种名字。
    
- plan 与 feature 冲突。
    
- feature 与 acceptance 不对应。
    
- constitution 被删节。
    
- README 声称未实现功能已可用。
    
- 是否意外创建 Rust 实现。
    
- 是否引入 dependency。
    
- 是否出现 WebView/Tauri/Tokio 等禁止方向。
    
- 是否存在多个 architecture source of truth。
    
- 是否存在 docs/plan 之外的“更权威”文档。
    

如果本机有 Markdown link checker，可以运行。

没有则手工检查相对链接。

**不要为了检查 Markdown 引入大型 Node/npm 工具链。**

---

# 48. Phase 0 Review

完成编辑后必须进行一次架构 review。

重点回答：

1. 本体是否仍然是一张 Markdown 草稿纸？
    
2. 是否有任何文档暗示未来多文档/知识管理？
    
3. 四层 + Object Plane 是否连续？
    
4. Interaction Shell 是否过重？
    
5. 文件系统是否只通过未来 Execution Domain adapter 进入？
    
6. DocumentState 的 authority 是否明确？
    
7. Preview 是否严格只是 projection？
    
8. Managed asset ownership 是否安全？
    
9. 外部修改与 autosave 冲突是否有明确治理？
    
10. Windows-specific 内容是否与 core contract 分离？
    
11. IME fallback 是否没有污染主骨架？
    
12. 有没有为了未来可能性提前建立 plugin/framework？
    
13. 所有核心流程是否包含 failure path？
    
14. acceptance 是否覆盖所有 frozen v1 行为？
    
15. 是否出现与工程宪法冲突的设计？
    

发现问题直接修正文档。

如果问题属于骨架级不确定性而无法根据 Prompt 解决：

```
docs/report/phase-00-architecture-question.md
```

记录，不擅自发明。

---

# 49. Git 行为

本阶段：

- 不 push。
    
- 不 force。
    
- 不 rebase。
    
- 不修改 remote。
    
- 不删除 unrelated changes。
    

如果工作树开始时是 clean：

完成并验证后可以创建一个本地 commit：

```
docs: initialize StickyMD engineering governance
```

如果开始时工作树不是 clean：

不要自动 commit。

只报告建议 commit 内容。

---

# 50. 最终回复格式

完成后必须按以下结构回复 USER：

## Phase 0 Result

### Repository State Before Work

说明：

```
branch
clean/dirty
existing relevant files
```

### Files Created

逐项列出。

### Files Modified

逐项列出。

### Architecture Contracts Established

用短列表概括：

```
authority model
four-layer architecture
object plane
document authority
asset authority
platform boundary
verification model
```

### Constitution

明确说明：

```
docs/plan/00_engineering_constitution.md
```

是否完整落盘。

### Verification

逐条给出：

```
git diff --check
link checks
manual contract review
```

及结果。

### Scope Compliance

明确确认：

```
No runtime feature implementation was added.
No Rust dependency was introduced.
No WebView/Tauri/Tokio architecture was introduced.
```

### Risks / Open Questions

如果没有：

```
None blocking Phase 1.
```

有则给出对应 report path。

### Git

说明：

```
commit created? yes/no
commit SHA
push performed? MUST be no
```

### Next Phase

只写：

> Ready for Phase 1 technical foundation/spike after USER review.

不要自行开始下一阶段。

---

# 51. Definition of Done

Phase 0 只有全部成立才算完成：

- Root `AGENTS.md` 存在并清晰定义 Agent workflow。
    
- `docs/AGENTS.md` 存在。
    
- `docs/plan/AGENTS.md` 存在。
    
- USER 工程宪法完整进入 `00_engineering_constitution.md`。
    
- StickyMD terminology 已固定。
    
- 产品定位与 Non-Goals 已固定。
    
- 四层 + Object Plane 已映射。
    
- runtime state authority 已定义。
    
- persistence contract 已定义。
    
- Markdown/RaTeX contract 已定义。
    
- IME/RichEdit fallback contract 已定义。
    
- asset ownership/GC/export contract 已定义。
    
- Windows shell/platform boundary 已定义。
    
- performance/reliability contract 已定义。
    
- testing/release contract 已定义。
    
- feature projection 已建立。
    
- acceptance cases 已建立。
    
- coverage matrix 已建立。
    
- ADR template 已建立。
    
- Phase 0 task document 已建立。
    
- README 不夸大当前实现。
    
- MIT LICENSE 已建立或保留。
    
- `git diff --check` 通过。
    
- 没有正式 Rust 功能代码。
    
- 没有新增 runtime dependency。
    
- 没有自行进入 Phase 1。
    

完成后立即停止。


# StickyMD Phase 1 — Technical Foundation & Risk Spikes

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0 已经完成并经过 USER 审核。

你现在执行：

> **Phase 1：Technical Foundation & Risk Spikes**

本阶段目标不是开发 StickyMD v1 产品功能，而是通过最小、可测量、可丢弃的技术验证，确认 StickyMD 已批准的底层技术路线是否真的满足：

- Windows 11 原生窗口。
    
- 无 WebView。
    
- 纯 Rust 主体。
    
- 中文 IME 可用。
    
- 低空闲 CPU。
    
- 可控内存。
    
- Markdown AST 可直接复用。
    
- RaTeX 数学渲染可直接复用。
    
- Portable 文件模型可安全实现。
    
- Windows-specific 能力可以被隔离在薄 adapter 中。
    

本阶段的最终产物不是“一个基本可用 StickyMD”。

本阶段的最终产物是：

```
可编译工程骨架
+
4 组技术 Spike
+
实测数据
+
风险报告
+
是否允许进入正式实现阶段的结论
```

---

# 0. 第一原则

开始任何操作前，必须严格执行：

```
AGENTS.md
↓
docs/plan/00_engineering_constitution.md
↓
docs/plan/01_terminology.md
↓
本阶段相关 docs/plan/*
↓
docs/features/*
↓
docs/acceptance-cases/*
↓
当前代码
```

特别阅读：

```
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/05_document_persistence.md
docs/plan/06_markdown_math_rendering.md
docs/plan/07_editor_and_ime.md
docs/plan/09_windows_shell.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md
docs/tasks/phase-00-repository-governance.md
```

如果 Phase 0 文档文件名略有不同，按 coverage matrix 找对应文档。

---

# 1. 开始前仓库检查

首先执行：

```
git status --short
git branch --show-current
git log -5 --oneline
```

记录结果。

然后检查仓库结构：

```
find . -maxdepth 4 -type f | sort
```

Windows PowerShell 环境可使用等价命令。

必须确认：

- Phase 0 governance 文档存在。
    
- 工程宪法存在。
    
- coverage matrix 存在。
    
- 没有未批准的 Rust runtime 实现。
    
- 工作树是否 clean。
    

如果工作树不是 clean：

- 不 reset。
    
- 不 clean。
    
- 不覆盖 USER 修改。
    
- 本阶段可以继续，但最后不要自动 commit，除非所有已有修改都明确属于本任务。
    

---

# 2. 本阶段必须遵守的工程宪法约束

本阶段尤其受到以下原则约束：

## 2.1 Spike 不是正式骨架实现

探索性代码必须被明确标识为：

```
experimental
```

它的存在是为了验证架构，不得因为“已经能跑”就直接自然演化成正式代码。

只有经过本阶段结论确认后，下一阶段才能决定哪些实现：

- 保留。
    
- 重写。
    
- 提升为正式模块。
    
- 删除。
    

---

## 2.2 不允许因 Spike 方便而污染架构

禁止：

```
UI 直接写 note.md
Window event 直接调用 filesystem
Preview 成为 Document authority
Comrak AST 成为长期业务模型
RaTeX 类型泄漏到 Document core
windows::Win32 类型泄漏到 core
```

---

## 2.3 USER 已批准骨架不允许自行修改

本阶段 Agent 没有权限擅自改变：

- 四层 + Object Plane。
    
- Windows 11 only。
    
- 单 Note 模型。
    
- Portable 模型。
    
- Comrak 方向。
    
- RaTeX 方向。
    
- winit/cosmic-text/tiny-skia/softbuffer 优先路线。
    
- RichEdit 只允许最后 fallback。
    
- 无 WebView。
    
- 无 Tauri。
    
- 无 Tokio。
    
- 无数据库。
    
- 无网络。
    

如果某项被实际证伪：

**不要偷偷换技术。**

必须：

```
docs/report/phase-01-<topic>-risk.md
```

并停止该分支继续扩张。

---

# 3. Phase 1 总体目标

本阶段包含五部分：

```
Phase 1A — Rust workspace foundation
Phase 1B — Window / framebuffer spike
Phase 1C — Text / IME spike
Phase 1D — Markdown / math spike
Phase 1E — Portable reliability spike
```

顺序执行。

不要并行大量搭建。

优先先验证最危险的假设。

---

# 4. 目标仓库代码结构

本阶段建立最小 Rust workspace。

推荐：

```
.
├─ Cargo.toml
├─ Cargo.lock
├─ rust-toolchain.toml
│
├─ crates/
│  ├─ stickymd-core/
│  │  ├─ Cargo.toml
│  │  └─ src/
│  │     └─ lib.rs
│  │
│  └─ stickymd-render/
│     ├─ Cargo.toml
│     └─ src/
│        └─ lib.rs
│
├─ apps/
│  └─ stickymd-win/
│     ├─ Cargo.toml
│     └─ src/
│        └─ main.rs
│
└─ experiments/
   └─ phase-01/
      ├─ README.md
      ├─ window/
      ├─ ime/
      ├─ markdown-math/
      └─ persistence/
```

注意：

`experiments/phase-01/` 是技术验证目录。

正式生产代码不得为了完成 Spike 而大量塞进：

```
stickymd-core
stickymd-render
stickymd-win
```

Production crates 本阶段只建立：

- package skeleton。
    
- architecture boundaries。
    
- 最小公共类型占位。
    
- 编译结构。
    

---

# 5. Workspace 原则

创建：

```
Cargo.toml
```

Workspace：

```
[workspace]
resolver = "2"
members = [
    "crates/stickymd-core",
    "crates/stickymd-render",
    "apps/stickymd-win",
]
```

Edition：

```
2024
```

License：

```
MIT
```

不要一开始把所有未来 dependency 都塞进 workspace。

只添加本阶段确实使用的依赖。

---

# 6. Rust Toolchain

创建：

```
rust-toolchain.toml
```

优先：

```
[toolchain]
channel = "1.97.1"
profile = "minimal"
components = ["rustfmt", "clippy"]
targets = ["x86_64-pc-windows-msvc"]
```

但必须先检查本机：

```
rustc --version
cargo --version
rustup toolchain list
```

如果指定版本无法使用：

不要擅自更换长期规范。

在：

```
docs/report/phase-01-toolchain-report.md
```

记录问题。

---

# 7. Production crate 边界

## 7.1 stickymd-core

必须：

```
#![forbid(unsafe_code)]
```

本阶段只允许放：

- 平台无关基础类型。
    
- 错误类型。
    
- 最小模块 skeleton。
    
- future architecture comments。
    

不得加入：

- Windows API。
    
- winit。
    
- softbuffer。
    
- filesystem Win32。
    
- tray。
    
- UI。
    

每个正式模块未来必须使用：

```
//! plan_ref: docs/plan/<chapter>.md#<anchor>
```

Phase 1 如果创建正式 Rust module，也必须遵守。

---

## 7.2 stickymd-render

必须：

```
#![forbid(unsafe_code)]
```

未来职责：

```
Markdown projection
Owned AST
RenderTree
layout
math integration
preview selection
```

本阶段只创建最小结构。

不要提前实现完整 preview。

---

## 7.3 stickymd-win

是 Windows Interaction Shell + Windows adapters 的未来承载位置。

本阶段只保留：

- minimal Windows executable。
    
- experimental wiring。
    

Windows unsafe 未来只能存在于明确 platform module。

本阶段如果必须使用 Win32：

```
// SAFETY:
```

注释不可缺失。

---

# 8. Dependency 审计流程

本阶段每添加一个 crate 前必须记录：

```
Name
Version
Purpose
License
Direct dependencies
Why standard library/current dependency is insufficient
Runtime impact
Cross-platform impact
Replaceability
```

建立：

```
docs/report/phase-01-dependency-baseline.md
```

本阶段至少核实：

```
winit
cosmic-text
tiny-skia
softbuffer
comrak
RaTeX crates
windows
```

如果需要：

```
raw-window-handle
```

由依赖自然引入即可。

不要因为“以后可能用”添加：

```
notify
tray-icon
rfd
image
arboard
serde
toml
sha2
```

除非对应 Spike 真的需要。

---

# 9. 明确禁止依赖

运行：

```
cargo tree
```

检查不得直接或间接引入明显的：

```
tauri
wry
webview2
cef
electron
chromium
tokio
async-std
wgpu
iced
egui
slint
gtk
qt
```

如果某个必须 crate 的 transitive dependency 意外引入这些：

停止。

写 risk report。

---

# 10. Phase 1A — Workspace Foundation

## 10.1 目标

建立：

- Cargo workspace。
    
- 三个 production crates。
    
- Windows 11 target。
    
- 基础 CI。
    
- dependency governance。
    
- unsafe boundaries。
    

---

## 10.2 Cargo profile

建议：

```
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"
```

本阶段只作为工程 baseline。

之后性能 Spike 可以调整，但调整需要 measurement。

---

## 10.3 CI

创建：

```
.github/workflows/ci.yml
```

当前阶段只做：

### Windows

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```

### Linux

只验证：

```
stickymd-core
stickymd-render
```

如果 render 当前因为 Windows-only dependency 无法 Linux build：

视为边界设计问题。

不要把 Windows-only dependency 塞入 render/core。

---

## 10.4 Gate

Phase 1A 完成必须：

```
cargo fmt --check PASS
cargo clippy PASS
cargo test PASS
cargo build release PASS
```

并且：

```
core unsafe = 0
render unsafe = 0
```

---

# 11. Phase 1B — Window / Framebuffer Spike

目录：

```
experiments/phase-01/window/
```

---

## 11.1 验证目标

验证：

```
winit
+
softbuffer
+
tiny-skia
```

是否能满足 StickyMD 基础窗口需求。

只做一张测试窗口。

---

## 11.2 Spike UI

窗口只画：

```
StickyMD Window Spike

中文：测试文本
Latin: Times New Roman test

Opacity: 96%
DPI: xxx
FPS only while animating
```

不要做 editor。

不要做 Markdown。

---

## 11.3 必须验证

### Window

- 创建。
    
- resize。
    
- move。
    
- minimize。
    
- restore。
    
- close event。
    
- redraw。
    

### Rendering

- tiny-skia Pixmap。
    
- softbuffer present。
    
- 背景矩形。
    
- 简单路径。
    
- 文字暂时可先不用最终字体系统。
    

### Idle

关键验证：

窗口静止时：

```
不得持续 request_redraw()
```

不得建立：

```
16 ms permanent timer
60 FPS permanent loop
```

EventLoop 必须可以进入 wait。

---

## 11.4 DPI

至少测试：

```
100%
150%
200%
```

记录：

- logical size。
    
- physical size。
    
- scale factor。
    
- framebuffer size。
    

---

## 11.5 Resize allocation

观察 resize 时：

- 是否每帧无界 allocate。
    
- framebuffer 是否正常重用。
    
- 内存 resize 后是否持续增长。
    

---

## 11.6 Windows opacity Spike

允许在本 Spike 使用非常薄的 Win32 adapter 测试：

```
WS_EX_LAYERED
SetLayeredWindowAttributes
```

测试：

```
70
85
96
100
```

如果 winit 提供足够能力，优先 winit。

只有不足时使用 Win32。

记录：

```
why Win32 was necessary
API used
unsafe boundary
```

---

## 11.7 Windows 11 rounded corner

可验证：

```
DwmSetWindowAttribute
```

但只作为 Spike。

不做完整视觉系统。

---

## 11.8 数据记录

创建：

```
experiments/phase-01/window/RESULTS.md
```

记录：

```
Windows version
CPU
RAM
GPU
monitor
DPI
Rust version
winit version
softbuffer version
tiny-skia version
```

以及：

```
startup subjective result
idle CPU
private working set
resize behavior
opacity behavior
DPI behavior
known bugs
```

---

# 12. 内存测量方法

不要看一次 Task Manager 截图就下结论。

使用 PowerShell：

```
Get-Process <process-name> |
Select-Object Id, ProcessName, WorkingSet64, PrivateMemorySize64, CPU
```

至少：

1. 启动。
    
2. 等待 30 秒。
    
3. 确保没有动画。
    
4. 记录。
    
5. 重复 5 次。
    

记录：

```
median
max
```

如果可以获取：

```
Private Working Set
Commit Size
```

优先记录。

---

# 13. Phase 1C — Text / IME Spike

这是整个 Phase 1 风险最高部分。

目录：

```
experiments/phase-01/ime/
```

---

# 14. Text Spike 架构

优先：

```
winit
+
cosmic-text
+
自有 String buffer
```

不要首先使用 RichEdit。

不要首先使用 TextBox/WinUI。

---

# 15. 最小编辑功能

只实现足够验证 IME：

- 一行/多行 text buffer。
    
- caret。
    
- mouse click caret。
    
- selection。
    
- Backspace。
    
- Delete。
    
- Enter。
    
- arrow。
    
- Ctrl+A。
    
- Ctrl+C。
    
- Ctrl+X。
    
- Ctrl+V。
    
- Ctrl+Z 可暂时只验证一个简单 operation stack。
    
- scrolling。
    

不要做：

- Markdown。
    
- syntax highlighting。
    
- preview。
    
- image。
    
- autosave。
    

---

# 16. 文本 authority

即使 Spike 也必须保证：

```
String buffer
```

是 canonical source。

cosmic-text Buffer：

```
layout/shaping projection
```

不得让 cosmic-text 的内部文本状态与 String 并列成为两个 authority。

如果 cosmic-text API 迫使这种设计：

记录风险。

不要偷偷接受双 authority。

---

# 17. IME 模型

必须区分：

```
Canonical Text
IME Preedit
IME Commit
```

Preedit：

```
不是 canonical document
不进入 undo
不触发保存
```

Commit：

```
一次 commit
→ 一次 TextDelta
```

---

# 18. 必须验证的 winit IME 事件

检查：

```
Ime::Enabled
Ime::Preedit
Ime::Commit
Ime::Disabled
```

以及：

```
set_ime_cursor_area
```

candidate window 必须跟 caret。

---

# 19. Microsoft Pinyin 手工矩阵

至少测试：

### Pinyin 1

输入：

```
nihao
```

选择：

```
你好
```

Expected：

```
commit 一次
```

### Pinyin 2

输入：

```
这是 Rust 的 trait
```

连续中英文切换。

### Pinyin 3

composition 中：

- left/right。
    
- backspace。
    
- selection。
    
- Esc cancel。
    

### Pinyin 4

高 DPI：

```
150%
200%
```

candidate 窗口必须靠近 caret。

### Pinyin 5

composition commit 后：

```
Ctrl+Z
```

必须一次撤销完整 commit。

---

# 20. WeChat Input Method

重复同样矩阵。

如果微信输入法环境不可用：

不要声称 PASS。

记录：

```
NOT TESTED — environment unavailable
```

这项将保留为 Phase 1 blocking verification。

---

# 21. IME 透明度测试

窗口 opacity：

```
70
96
100
```

验证：

- candidate 正常。
    
- caret 正常。
    
- composition underline 正常。
    
- focus 正常。
    

---

# 22. IME Dock-like movement

不需要实现真正 docking。

但至少：

1. Window move。
    
2. Resize。
    
3. DPI change。
    
4. Refocus。
    

之后再次输入。

确认 candidate rect 未失效。

---

# 23. Font Spike

cosmic-text 验证：

中文：

```
仿宋_GB2312
```

Latin：

```
Times New Roman
```

代码字体可测试：

```
Consolas
```

必须记录：

- 系统字体是否能找到。
    
- 字体 family 名实际枚举结果。
    
- 缺仿宋_GB2312 时 fallback 行为。
    

不要嵌入仿宋字体文件。

---

# 24. Mixed Script Spike

测试：

```
这是 Rust 的 trait 示例 ABC 123。
```

目标：

- CJK run → 仿宋。
    
- Latin run → Times New Roman。
    
- fallback 不出现 tofu。
    
- caret mapping 正确。
    

---

# 25. IME Spike Stop Condition

如果出现：

```
Microsoft Pinyin blocking bug
```

不要立即使用 RichEdit。

先：

### Attempt 1

确认：

- winit API 使用正确。
    
- cursor area 正确。
    
- preedit model 正确。
    

### Attempt 2

最小复现并尝试第二轮修复。

如果仍不能稳定：

创建：

```
docs/report/phase-01-ime-risk.md
```

包括：

- 重现步骤。
    
- OS build。
    
- 输入法版本。
    
- winit version。
    
- cosmic-text version。
    
- root cause hypothesis。
    
- 两轮修复。
    
- RichEdit fallback implications。
    

**不要在 Phase 1 自动启用 RichEdit。**

是否启用 RichEdit必须交 USER 决策。

---

# 26. IME RESULTS

创建：

```
experiments/phase-01/ime/RESULTS.md
```

结果表：

|Test|Microsoft Pinyin|WeChat IME|
|---|---|---|
|basic CJK|PASS/FAIL/NT|...|
|mixed input|||
|preedit|||
|cancel|||
|selection|||
|undo commit|||
|150% DPI|||
|200% DPI|||
|opacity 70|||
|refocus|||

不得把 NOT TESTED 写成 PASS。

---

# 27. Phase 1D — Markdown / Math Spike

目录：

```
experiments/phase-01/markdown-math/
```

---

# 28. Comrak 验证

使用 Comrak。

目标不是 HTML output。

目标是：

```
Markdown
→ AST
→ 遍历 AST
→ owned diagnostic tree
```

建立最小：

```
enum SpikeNode {
    Paragraph,
    Text,
    Heading,
    Strong,
    Emphasis,
    Code,
    CodeBlock,
    Link,
    Image,
    Table,
    List,
    InlineMath,
    DisplayMath,
    RawHtml,
}
```

只用于验证 AST 是否包含所需信息。

不要设计正式 RenderTree。

---

# 29. Markdown fixture

创建：

```
experiments/phase-01/markdown-math/fixtures/all.md
```

包含：

````
# 标题

中文 English **bold** *italic* ~~strike~~

- item
- [ ] task

> quote

`inline code`

```rust
fn main() {}
````

|   |   |
|---|---|
|A|B|
|1|2|

[link](https://example.com/)

  

$E=mc^2$

$$  
\int_0^1 x^2 dx  
$$

(  
a^2+b^2=c^2  
)

[  
\sum_{n=1}^{\infty}\frac1{n^2}  
]

<div style="color:red">raw html</div> ```

验证 AST。

---

# 30. Comrak 必须确认的事实

记录：

- CommonMark option。
    
- GFM extension option。
    
- math dollar extension。
    
- LaTeX math extension。
    
- raw HTML AST representation。
    
- source position 信息。
    
- table representation。
    
- image representation。
    

创建：

```
experiments/phase-01/markdown-math/COMRAK_NOTES.md
```

---

# 31. Arena 生命周期验证

明确验证：

```
Arena AST
```

不能自然成为长期 cross-thread AppState。

实现：

```
Arena
→ parse
→ copy minimal owned structure
→ Arena drop
```

验证这种模式没有技术阻碍。

---

# 32. RaTeX Spike

验证：

```
RaTeX parse
→ layout
→ DisplayList
```

数学 fixture：

```
x^2
\frac{a}{b}
\sqrt{x}
\sum_{n=1}^{\infty}
\int_0^1
\left(\frac{x}{y}\right)
\begin{matrix}a&b\\c&d\end{matrix}
\begin{cases}x,&x>0\\-x,&x<0\end{cases}
\mathbb{R}
\mathbf{x}
\operatorname{rank}(A)
```

以及错误：

```
\frac{
```

---

# 33. RaTeX Rendering

第一 Spike 可以使用 RaTeX 当前最直接公开 renderer：

```
DisplayList
→ PNG
```

但必须明确记录：

> 该路径仅用于技术验证，不得成为正式 Preview 热路径。

同时调查：

```
是否可直接将 DisplayList painter 接到 tiny-skia
```

不要大规模 fork RaTeX。

---

# 34. 数学字体

必须验证：

- RaTeX fonts 如何加载。
    
- release 是否可 embed。
    
- KaTeX font license。
    
- 二进制大小增量。
    
- 首次公式渲染内存增量。
    

记录：

```
without formula memory
first formula memory
after 20 formulas memory
```

---

# 35. Math delimiter ownership

Comrak：

```
负责 delimiter
```

RaTeX：

```
负责 delimiter 内数学内容
```

验证：

```
$...$
$$...$$
\(...\)
\[...\]
```

不要在 Spike 自己重新正则识别 delimiter。

---

# 36. Math Error

错误公式：

- 不 panic。
    
- 返回 error。
    
- Spike output 保留原文。
    

---

# 37. Markdown / Math Benchmark

测试至少：

```
20 KiB
100 KiB
1 MiB
```

测试：

```
Comrak parse time
owned conversion time
math parse/layout time
```

不要追求优化。

只建立 baseline。

每类至少重复 20 次并报告：

```
median
p95
max
```

可以写一个简单 benchmark executable。

本阶段不必引入 Criterion，除非确有价值。

优先简单：

```
std::time::Instant
```

避免 dependency 膨胀。

---

# 38. Markdown Math RESULTS

创建：

```
experiments/phase-01/markdown-math/RESULTS.md
```

结论必须回答：

```
Can Comrak define our Markdown semantics? yes/no
Can Comrak expose all 4 math delimiters? yes/no
Can Arena be converted into owned projection? yes/no
Can RaTeX handle required math baseline? yes/no
Can math render without WebView? yes/no
What is first-math memory cost?
What is binary-size cost?
What remains risky?
```

---

# 39. Phase 1E — Portable Reliability Spike

目录：

```
experiments/phase-01/persistence/
```

只验证基础机制。

不要做 autosave UI。

---

# 40. Canonical Program Directory

实现 Spike：

```
current_exe
→ parent
→ canonicalize
→ normalized identity
→ SHA-256
```

测试：

```
D:\A\StickyMD.exe
D:\A\.\StickyMD.exe
```

产生同一 identity。

如果 junction/symlink 可测试，也测试。

---

# 41. Single Instance Spike

Windows：

```
Named Mutex
+
Named Event
```

目标：

第一个：

```
acquire mutex
wait
```

第二个：

```
mutex exists
signal event
exit
```

第一个收到：

```
SHOW_REQUEST
```

只需打印日志。

不必真正 show Window。

---

# 42. Writable Directory Spike

创建：

```
./note/
```

测试写：

```
.write-test
```

执行：

```
create
write
flush
delete
```

失败：

```
明确 error
```

不 fallback。

---

# 43. Atomic Save Spike

实现一个最小：

```
atomic_write(path, bytes)
```

流程：

```
same-dir tmp
write
flush
FlushFileBuffers
ReplaceFileW
fallback MoveFileExW if appropriate
```

必须：

- 有 typed error。
    
- unsafe 集中。
    
- `SAFETY` 注释。
    
- 文件 handle 正确释放。
    

---

# 44. Atomic Failure Injection

至少测试：

### Failure 1

temp write 前失败。

Expected：

```
original untouched
```

### Failure 2

temp write 后、replace 前模拟 failure。

Expected：

```
original untouched
temp remains recoverable
```

### Success

Expected：

```
new complete
old not half-written
```

---

# 45. Recovery Spike

启动检查：

```
note.md
note.md.tmp
```

比较：

- UTF-8 validity。
    
- mtime。
    
- hash。
    

输出：

```
RecoveryCandidate
```

不自动覆盖。

---

# 46. External File Change Spike

可以暂时不引入 `notify`。

Phase 1 只验证数据模型：

```
base_disk_hash
runtime_dirty
external_hash
```

测试逻辑：

### clean + external change

```
reload candidate
```

### dirty + external change

```
conflict
```

### same hash

```
ignore
```

真正 filesystem watcher 可以留后续阶段。

这样避免 Phase 1 引入额外 dependency。

---

# 47. Persistence RESULTS

创建：

```
experiments/phase-01/persistence/RESULTS.md
```

回答：

```
Can canonical directory identity be stable?
Can same-directory single-instance work?
Can portable write checks work?
Can atomic replacement guarantee no truncate-half-write?
Can crash temp be detected?
What Windows API remains necessary?
Can all Win32 detail stay behind adapter?
```

---

# 48. Windows Adapter 审计

本阶段结束后列出所有直接使用的 Windows API。

例如：

```
SetLayeredWindowAttributes
DwmSetWindowAttribute
CreateMutexW
CreateEventW
SetEvent
FlushFileBuffers
ReplaceFileW
MoveFileExW
```

创建：

```
docs/report/phase-01-windows-api-baseline.md
```

每个 API：

```
Purpose
Why Rust cross-platform abstraction was insufficient
Which future adapter owns it
Unsafe?
Expected future replacement path
```

目标：

**Win32 越少越好，但不是为了少而绕路。**

---

# 49. Phase 1 性能基线

创建：

```
docs/report/phase-01-performance-baseline.md
```

至少记录：

## Empty Window Spike

```
startup
idle private working set
idle CPU
binary size
```

## IME Spike

```
idle memory
text after 10 KiB
text after 100 KiB
```

## Markdown

```
20 KiB parse
100 KiB parse
1 MiB parse
```

## Math

```
first formula
20 formulas
100 formulas
memory delta
```

---

# 50. 性能判断原则

不要因为没达到最终目标就立刻换架构。

判断顺序：

1. 有没有 debug build？
    
2. 有没有持续 redraw？
    
3. 有没有 cache 没释放？
    
4. 有没有重复字体加载？
    
5. 有没有实验代码本身无优化？
    
6. 是 framework 固有成本还是 Spike bug？
    

只有确认属于根本性架构问题时才写 Risk Report。

---

# 51. 初始工程目标

这些是本阶段比较对象，不是对外承诺：

```
Source-only typical idle target:
    < 40 MiB hard exploratory threshold

Preview light fixture:
    < 52 MiB exploratory threshold

Idle CPU:
    < 0.1%

Cold startup:
    < 300 ms

20 KiB preview pipeline:
    < 100 ms desirable
```

如果达不到：

记录真实数据。

**禁止伪造 PASS。**

---

# 52. Logging

本阶段不引入大型日志框架。

实验程序可以：

```
eprintln!
```

Production skeleton 如果需要日志 abstraction，只定义非常薄边界。

不要为了未来日志引入复杂 tracing stack，除非有充分理由。

---

# 53. Error Handling

Library：

```
thiserror
```

可以使用。

App Spike：

允许：

```
anyhow
```

但是否正式纳入需要在 dependency report 说明。

不要：

```
unwrap()
expect()
```

出现在核心 runtime 路径。

Spike setup 可有限使用 `expect`，但必须明显是测试代码。

---

# 54. Tests

Phase 1 至少应形成：

```
cargo test
```

覆盖：

- core skeleton。
    
- owned Markdown conversion sample。
    
- math parse fixture。
    
- canonical path identity。
    
- atomic save。
    
- recovery candidate。
    
- external conflict logic。
    

Windows-only test：

使用：

```
#[cfg(target_os = "windows")]
```

---

# 55. 文档更新

Phase 1 完成后只允许对 plan 做：

- 技术事实补充。
    
- 已验证接口细化。
    
- typo。
    
- 明确实现约束。
    

不得因为 Spike 代码长成某个样子就修改 plan 去适配它。

如果 Spike 证伪 plan：

不要直接改 plan。

建立 Risk Report，请 USER 决策。

---

# 56. 创建 Phase 1 Task

创建：

```
docs/tasks/phase-01-technical-foundation.md
```

至少：

```
Status
Goals
Dependencies
Spikes
Deliverables
Verification
Risks
Result
```

开始：

```
Status: In Progress
```

全部完成后：

```
Status: Completed — awaiting USER architecture review
```

---

# 57. 创建 Phase 1 总报告

创建：

```
docs/report/phase-01-technical-spike-report.md
```

必须具有以下结构：

# Phase 1 Technical Spike Report

## Executive Decision

每项：

```
Window/render path:
PASS / CONDITIONAL / FAIL

IME path:
PASS / CONDITIONAL / FAIL

Comrak:
PASS / CONDITIONAL / FAIL

RaTeX:
PASS / CONDITIONAL / FAIL

Portable persistence:
PASS / CONDITIONAL / FAIL
```

---

## Environment

```
Windows edition
Windows build
CPU
RAM
GPU
monitors
DPI
Rust
Cargo
target
commit
```

---

## Dependency Baseline

表格。

---

## Spike Results

分别引用四个 RESULTS。

---

## Memory Baseline

表格。

---

## Performance Baseline

表格。

---

## Unsafe Baseline

所有 unsafe location。

---

## Windows API Baseline

表格。

---

## Architecture Findings

回答：

```
Did any Spike contradict docs/plan?
Did any platform abstraction leak?
Did any duplicate authority appear?
Did any dependency introduce unacceptable runtime cost?
```

---

## Blocking Risks

如果没有：

```
None.
```

不能把未测试 IME 写成 None。

---

## Recommendation

只能三种：

```
A. APPROVE Phase 2
B. APPROVE Phase 2 WITH CONDITIONS
C. STOP — architecture review required
```

Agent 只能推荐。

**USER 才批准进入下一阶段。**

---

# 58. Phase 1 架构 Review

完成所有实验后，必须自己做一次 review。

检查：

## Boundary

- core 是否 Windows-free？
    
- render 是否 Windows-free？
    
- Window shell 是否没有业务 authority？
    
- experimental code 是否与 production skeleton 隔离？
    

## Authority

- String source 是否唯一？
    
- cosmic-text 是否只是 projection？
    
- Comrak AST 是否只是 transient parser structure？
    
- RaTeX DisplayList 是否只是 math projection？
    
- Disk 是否没有与 runtime state 并列 authority？
    

## Dependency

- 是否加入了不必要 crate？
    
- 是否出现 framework creep？
    
- cargo tree 是否出现禁止组件？
    

## Performance

- 是否有永久 redraw？
    
- 是否有无限 cache？
    
- 是否有线程池？
    
- 是否有 async runtime？
    

## Safety

- Windows unsafe 是否集中？
    
- SAFETY 注释是否完整？
    
- atomic save failure path 是否测试？
    

## Governance

- 是否有 Spike code 偷偷成为正式业务实现？
    
- 是否为了代码方便改了 plan？
    

发现问题：

先修。

骨架问题：

写 report，不擅自修骨架。

---

# 59. 可删除性要求

所有：

```
experiments/phase-01/*
```

必须做到：

> 删除整个 experiments/phase-01 后，Production workspace 仍然可以构建。

这是 Spike 与正式主干分离的重要验证。

最终必须测试一次：

```
临时复制仓库
移除 experiments/phase-01
cargo build --workspace
```

或者使用不修改真实工作树的等价方式验证。

---

# 60. 不要提前实现的内容

Phase 1 严格禁止正式实现：

- StickyMD 主 UI。
    
- 顶部按钮。
    
- Theme selector。
    
- opacity selector UI。
    
- Source/Preview/Split 正式模式。
    
- Autosave scheduler。
    
- File watcher。
    
- Conflict banner。
    
- Image GC。
    
- Export。
    
- Tray menu。
    
- Docking state machine。
    
- Multi-monitor docking。
    
- Full Markdown Preview。
    
- Preview selection。
    
- Production Undo manager。
    
- Config parser。
    
- Release packaging。
    
- Application icon。
    
- Installer。
    

Spike 中为了验证可以有最小临时实现。

但不得演化成产品。

---

# 61. 本阶段提交粒度

如果仓库起始 clean，建议本阶段分几个本地 commit：

```
build: initialize Rust workspace foundation

spike: validate native window and software framebuffer

spike: validate cosmic-text IME path

spike: validate Comrak and RaTeX pipeline

spike: validate portable persistence primitives

docs: record phase 1 technical findings
```

不要 push。

如果 USER 希望单 commit，则最后 squash 由 USER 决定。

Agent 不自行 rebase 已有历史。

---

# 62. 验证命令

最终至少运行：

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
git diff --check
```

检查：

```
cargo tree
```

确认无：

```
tauri
wry
webview
tokio
wgpu
```

如果 PowerShell 可用，运行基础内存测量脚本。

---

# 63. 最终回复格式

完成后必须严格按：

# Phase 1 Result

## Repository State Before Work

```
branch
clean/dirty
starting commit
```

## Workspace Created

列出：

```
production crates
experimental spikes
```

## Dependencies Added

表格：

```
crate
version
purpose
license
runtime implication
```

## Spike A — Window / Framebuffer

```
PASS / CONDITIONAL / FAIL
```

关键数据。

## Spike B — IME

```
Microsoft Pinyin:
WeChat IME:
```

必须区分：

```
PASS
FAIL
NOT TESTED
```

## Spike C — Markdown / Math

分别：

```
Comrak
RaTeX
```

## Spike D — Persistence

```
canonical path
single instance
atomic write
recovery
```

## Performance Baseline

表格。

## Memory Baseline

表格。

## Windows API Used

列表。

## Unsafe Code

```
file
reason
SAFETY invariant
```

## Architecture Drift

```
None
```

或者明确列出。

## Risk Reports

列出路径。

## Verification

所有命令及结果。

## Git

```
local commits
push = no
```

## Recommendation

只能：

```
APPROVE Phase 2
APPROVE Phase 2 WITH CONDITIONS
STOP — architecture review required
```

最后：

> Awaiting USER review. Do not start Phase 2 automatically.

---

# 64. Phase 1 Definition of Done

只有以下全部完成才可停止：

- Phase 0 governance 已阅读。
    
- Cargo workspace 建立。
    
- core crate 无 unsafe。
    
- render crate 无 unsafe。
    
- Windows app skeleton 建立。
    
- dependency baseline 完成。
    
- cargo tree 无禁止架构。
    
- Window/softbuffer/tiny-skia Spike 完成。
    
- Idle redraw 行为验证。
    
- Opacity Spike 完成。
    
- DPI Spike 完成。
    
- cosmic-text editing Spike 完成。
    
- Microsoft Pinyin 被真实测试。
    
- WeChat IME 被测试或明确标记 NOT TESTED。
    
- IME candidate positioning 被验证。
    
- mixed CJK/Latin font shaping 被验证。
    
- Comrak AST Spike 完成。
    
- 四种数学 delimiter 验证。
    
- RaTeX parser/layout Spike 完成。
    
- 数学字体策略验证。
    
- Math 错误路径验证。
    
- portable writable check 完成。
    
- canonical directory identity 完成。
    
- same-dir single instance Spike 完成。
    
- atomic save Spike 完成。
    
- failure injection 完成。
    
- recovery candidate 完成。
    
- performance baseline 完成。
    
- memory baseline 完成。
    
- Windows API baseline 完成。
    
- unsafe baseline 完成。
    
- Phase 1 Technical Spike Report 完成。
    
- `experiments/phase-01` 可整体删除而不破坏 production build。
    
- 所有验证命令通过或明确记录阻塞失败。
    
- 没有擅自进入产品功能实现。
    
- 没有自动进入 Phase 2。
    

完成后停止。



# StickyMD Phase 2 — Core Document Model, TextDelta & Undo/Redo

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0 已完成治理与工程合同初始化。

Phase 1 已完成 Technical Foundation & Risk Spikes。

**只有在 USER 已明确批准 Phase 1 进入下一阶段时，才允许执行本 Prompt。**

---

# 0. Phase 2 的唯一目标

本阶段正式实现 StickyMD 的核心文档状态模型：

```
Canonical Document Text
        │
        ▼
   DocumentState
        │
        ├── Generation
        ├── Dirty / Persisted Generation
        ├── TextDelta
        ├── DocumentSnapshot
        └── Undo / Redo
```

本阶段要第一次建立真正的 runtime authority：

> **程序运行期间，只有** `**DocumentState**` **是 Markdown 工作文本的唯一权威。**

后续：

- Source editor。
    
- IME。
    
- Autosave。
    
- Preview。
    
- File conflict。
    
- Image asset GC。
    
- Export。
    

全部只能通过这一核心模型操作文本。

---

# 1. 本阶段明确不做什么

Phase 2 **不是 UI 阶段**。

禁止实现：

```
正式 Source Editor UI
Preview
Split View
Comrak production integration
RaTeX production integration
Autosave worker
note.md atomic persistence
notify file watcher
external conflict UI
image clipboard
image GC
export
tray
docking
opacity UI
theme UI
multi-monitor
Windows RichEdit
```

Phase 1 的 Spike 代码也不得直接复制成为 production implementation，除非：

1. 对应内容属于本 Phase 正式范围；
    
2. 已重新按 production contract 审查；
    
3. 不携带实验性 shortcut。
    

本阶段主要修改：

```
crates/stickymd-core/
```

其它 production crate 除必要编译适配外不应增长业务逻辑。

---

# 2. 开始前必须读取

严格遵循根 `AGENTS.md`。

至少读取：

```
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md

docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/05_document_persistence.md
docs/plan/07_editor_and_ime.md
docs/plan/08_assets_and_export.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md

docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md

docs/report/phase-01-technical-spike-report.md
docs/report/phase-01-performance-baseline.md
```

如果实际文件名略有变化：

以 `docs/coverage-matrix.md` 为索引找到对应合同。

---

# 3. Phase 1 前置 Gate

首先检查 Phase 1 总报告。

允许继续的情况只有：

```
APPROVE Phase 2
```

或者：

```
APPROVE Phase 2 WITH CONDITIONS
```

且条件已经被 USER 明确接受。

如果 Phase 1 是：

```
STOP — architecture review required
```

立即停止。

不得因为本 Prompt 已存在而绕过前置 Gate。

如果仍有未关闭的 blocking risk：

创建或更新：

```
docs/report/phase-02-precondition-blocked.md
```

然后停止。

---

# 4. 开始前仓库状态

执行：

```
git status --short
git branch --show-current
git log -8 --oneline
```

记录：

```
branch
starting commit
clean / dirty
```

检查：

```
cargo metadata --no-deps
cargo tree -p stickymd-core
```

如果已有 USER 未提交修改：

- 不 reset。
    
- 不 clean。
    
- 不覆盖。
    
- 不自动 commit 混合修改。
    

---

# 5. 工程宪法中的核心约束

本阶段尤其需要遵守：

```
先骨架
→ 模块
→ 实现
```

以及：

```
正确性
>
可用性
>
根基兼容性
>
可维护性 / 可诊断性
>
性能
>
内存
```

因此不得出现：

> “这样少写一点代码，所以牺牲状态一致性。”

也不得：

> “这样少分配几 KB，所以允许 stale edit 覆盖新文本。”

---

# 6. Phase 2 在四层架构中的位置

本阶段主要实现：

```
Instruction Interface 的领域输入合同
+
Flow Coordination 可依赖的核心状态机
+
Execution Domain 中的平台无关 Document capability
+
Object Plane 的 doc::* 对象
```

不实现 Interaction Shell。

大致关系：

```
Future UI Action
      │
      ▼
Instruction Interface
      │
      ▼
EditRequest / Undo / Redo
      │
      ▼
DocumentState
      │
      ├── TextStore
      ├── TextDelta
      ├── UndoManager
      └── Generation
```

---

# 7. Single Source of Truth

必须在代码中维持以下 invariant：

```
DocumentState
=
runtime canonical document authority
```

以下都不能成为另一份文本 authority：

```
cosmic-text Buffer
Preview AST
Preview RenderTree
disk note.md
clipboard
IME preedit
UI widget state
worker snapshot
```

后续这些只能：

```
observe
project
request mutation
```

不能直接维护一份可独立写回的“真文本”。

---

# 8. 目标模块结构

根据 cohesion 可以微调，但建议：

```
crates/stickymd-core/
└─ src/
   ├─ lib.rs
   ├─ document.rs
   ├─ text_store.rs
   ├─ edit.rs
   ├─ selection.rs
   ├─ generation.rs
   ├─ undo.rs
   ├─ snapshot.rs
   ├─ state.rs
   └─ error.rs
```

不要为了满足文件数量机械拆分。

遵守治理阈值：

```
~250 行 → cohesion review
~500 行 → hard review
```

---

# 9. unsafe 禁止

`stickymd-core` 必须保持：

```
#![forbid(unsafe_code)]
```

本 Phase 不存在任何需要 unsafe 的理由。

最终验证：

```
rg "unsafe" crates/stickymd-core
```

除了：

```
forbid(unsafe_code)
文档文本
测试描述
```

之外不得有 runtime unsafe。

---

# 10. plan_ref

每个正式 production module 必须添加：

```
//! plan_ref: docs/plan/<chapter>.md#<stable-anchor>
```

例如：

```
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate
```

如果 plan 中没有稳定 anchor：

允许在对应 `docs/plan/` 文件添加**仅用于稳定引用的 anchor**。

例如：

```
<a id="documentstate"></a>
```

不得借机重写 plan。

---

# 11. 核心对象：Generation

实现：

```
pub struct Generation(u64);
```

可以使用 stronger derives：

```
Copy
Clone
Debug
Eq
PartialEq
Ord
PartialOrd
Hash
```

根据实际需要决定。

---

# 12. Generation 语义

Generation 表示：

> canonical document state 的单调递增版本。

初始：

```
Generation(0)
```

以下操作必须递增：

```
canonical text edit
Undo
Redo
未来 external reload
未来 recovery replacement
```

以下不得递增：

```
selection movement
caret movement
Preview refresh
theme change
opacity
window movement
mark persisted
worker completion
IME preedit
```

---

# 13. Generation 绝不能倒退

即使：

```
edit
→ undo
```

文本恢复到了以前内容：

generation 仍必须继续增长。

例如：

```
Initial:
text = A
gen = 10

edit:
text = B
gen = 11

undo:
text = A
gen = 12
```

不能：

```
undo
→ gen = 10
```

原因：

Generation 是 stale-result ordering token，不是历史快照 ID。

---

# 14. Generation Overflow

虽然现实中几乎不可能达到：

```
u64::MAX
```

仍不得使用 silent wrapping。

必须：

```
checked_add
```

失败：

```
DocumentError::GenerationExhausted
```

不需要设计复杂恢复。

Fail closed 即可。

---

# 15. TextStore 抽象

定义稳定、极小的文本存储边界。

例如：

```
pub trait TextStore {
    fn as_str(&self) -> &str;
    fn len_bytes(&self) -> usize;
    fn is_empty(&self) -> bool;

    fn replace(
        &mut self,
        range: Range<usize>,
        replacement: &str,
    ) -> Result<(), TextStoreError>;
}
```

实际 trait shape 根据 Rust ergonomics 可以小幅调整。

---

# 16. 第一实现必须是 String

实现：

```
StringTextStore
```

内部：

```
String
```

不得在 Phase 2 引入：

```
ropey
xi-rope
piece table
gap buffer
CRDT
```

理由：

- 单个临时便签。
    
- v1 典型文本较小。
    
- Phase 1 已做性能基线。
    
- 当前首要目标是状态正确性。
    

未来如果基准证伪 String：

应通过保持上层 Document API 稳定的方式替换存储。

---

# 17. 不要把 TextStore trait 动态化

`DocumentState` 不需要：

```
Box<dyn TextStore>
```

也不要为了“可替换”加入 runtime dispatch。

可以：

```
struct DocumentState {
    text: StringTextStore,
}
```

对外 API 不泄漏其内部表示。

未来替换实现时，DocumentState public API 保持稳定。

这符合：

> 模块可替换，而不是提前为替换付 runtime complexity。

---

# 18. UTF-8 Index Model

Phase 2 内部 edit range 使用：

```
UTF-8 byte offsets
```

原因：

- Rust String 原生模型。
    
- Markdown source position 最终也是 source byte orientation。
    
- 文件持久化是 UTF-8。
    
- 与 parser/source ranges 容易统一。
    

但所有 mutation 必须验证：

```
text.is_char_boundary(range.start)
text.is_char_boundary(range.end)
```

---

# 19. Core 不负责 grapheme navigation

这很重要。

以下行为属于未来 editor layer：

```
Backspace 删除一个 grapheme
ArrowLeft 移动一个 visual character
emoji cluster navigation
combining-mark caret movement
```

Core 的职责只是：

> 对一个已经合法的 UTF-8 range 执行可靠 mutation。

不要在 Phase 2 把 UI caret semantics 塞入 DocumentState。

---

# 20. Selection 模型

定义平台无关 selection：

```
pub struct TextPosition {
    pub byte: usize,
}

pub struct Selection {
    pub anchor: TextPosition,
    pub active: TextPosition,
}
```

或者等价 clean design。

必须提供：

```
is_collapsed
start
end
normalized_range
```

---

# 21. CursorSnapshot

Undo/Redo 需要恢复 editor selection。

定义：

```
pub struct CursorSnapshot {
    pub selection: Selection,
}
```

Phase 2 不加入：

```
pixel x
viewport scroll
IME candidate position
preferred visual column
```

这些属于 editor/session presentation state。

---

# 22. Selection Validation

所有进入 Document mutation 的 selection/cursor：

必须满足：

```
0 <= byte <= text.len()
char boundary
```

非法：

```
DocumentError::InvalidTextPosition
```

Selection 自身可以：

```
anchor > active
```

即反向 selection 是合法的。

normalized 只用于 range 操作。

---

# 23. TextDelta

核心类型：

```
pub struct TextDelta {
    range: Range<usize>,
    deleted: String,
    inserted: String,
    cursor_before: CursorSnapshot,
    cursor_after: CursorSnapshot,
}
```

如果 Phase 1 dependency strategy 证明 `Arc<str>` 更合理，可以使用 `Arc<str>`。

但必须基准说明。

默认优先简单：

```
String
```

---

# 24. TextDelta 语义

`range` 指：

> **编辑发生前**文档中的 byte range。

例如：

```
ABCDEF
```

把：

```
CD
```

换成：

```
中文
```

则：

```
range = 2..4
deleted = "CD"
inserted = "中文"
```

---

# 25. Deleted 内容不能由 UI 决定

推荐 public edit API：

```
pub struct EditRequest {
    pub range: Range<usize>,
    pub inserted: String,
    pub cursor_before: CursorSnapshot,
    pub cursor_after: CursorSnapshot,
    pub meta: EditMeta,
}
```

然后：

```
DocumentState
```

自己从 canonical text 读取：

```
deleted
```

构造 `TextDelta`。

这样 UI 无法声称：

```
“我删除的是 X”
```

而 canonical text 实际上是 Y。

---

# 26. Optimistic stale protection

如果未来需要直接应用已经构建好的 `TextDelta`：

必须验证：

```
current_text[range] == delta.deleted
```

否则：

```
DeletedTextMismatch
```

拒绝 mutation。

这为未来 worker / reconciliation 防止 stale write 提供基础。

---

# 27. EditMeta

为 Undo grouping 使用纯数据 metadata。

不要让核心自己读取 wall clock。

建议：

```
pub struct EditMeta {
    pub kind: EditKind,
    pub timestamp_ms: u64,
}
```

timestamp 由 Interaction/Coordination 层传入 monotonic runtime time。

这样 grouping：

- 可重复测试。
    
- 不依赖 `Instant::now()` 隐式全局时间。
    

---

# 28. EditKind

至少：

```
pub enum EditKind {
    Typing,
    Backspace,
    DeleteForward,
    Paste,
    ImeCommit,
    Newline,
    SelectionReplace,
    Other,
}
```

不要为了未来所有情况做几十个 variant。

---

# 29. canonical mutation API

推荐：

```
impl DocumentState {
    pub fn edit(
        &mut self,
        request: EditRequest,
    ) -> Result<EditOutcome, DocumentError>;

    pub fn undo(
        &mut self,
    ) -> Result<UndoOutcome, DocumentError>;

    pub fn redo(
        &mut self,
    ) -> Result<RedoOutcome, DocumentError>;
}
```

具体名字可以调整。

但必须只有极少数 mutation gateway。

禁止公开：

```
fn text_mut(&mut self) -> &mut String
```

否则 single source of truth 边界失效。

---

# 30. Read API

允许：

```
text()
len_bytes()
generation()
saved_generation()
is_dirty()
can_undo()
can_redo()
```

不得暴露内部 stack 可修改引用。

---

# 31. No-op Edit

以下必须视为 no-op：

```
range 为空
inserted 为空
```

以及可考虑：

```
deleted == inserted
```

推荐 `deleted == inserted` 也视为无内容变化。

No-op：

```
不递增 generation
不加入 Undo
不清 Redo
```

Cursor-only change 由 editor 自己处理。

---

# 32. UndoManager

实现 bounded Undo：

```
struct UndoManager {
    undo: ...
    redo: ...
    undo_bytes: usize,
}
```

可以用：

```
VecDeque
Vec
```

选择最清晰的结构。

---

# 33. Undo Limits

冻结值：

```
MAX_UNDO_ENTRIES = 256
MAX_UNDO_BYTES = 4 * 1024 * 1024
```

必须是核心常量。

本阶段不放 config。

用户不可修改。

---

# 34. Undo Memory 定义

4 MiB 是：

> Undo payload 的工程近似预算。

不是 allocator 实际 heap 精确 measurement。

必须在代码注释和 plan/report 中定义计费方式。

建议：

```
deleted.len()
+
inserted.len()
+
small fixed metadata estimate
```

例如：

```
+ 128 bytes / entry
```

如果你选择其它固定 overhead：

必须记录。

要求：

- deterministic。
    
- O(1) 易计算。
    
- 不试图做 allocator introspection。
    

---

# 35. Entry 太大

如果单个 UndoEntry 自身：

```
> 4 MiB
```

不能为了保留它导致预算失效。

推荐行为：

1. 编辑本身仍然成功。
    
2. 清空旧 Undo/Redo。
    
3. 此超大 edit 不进入 undo history。
    
4. 返回：
    

```
EditOutcome {
    undo_recorded: false
}
```

不得因此拒绝 canonical edit。

因为正确输入高于 Undo 能力。

---

# 36. 新 Edit 清空 Redo

规则：

```
Undo
→ Redo stack populated

new canonical edit
→ clear redo
```

必须测试。

---

# 37. Undo generation

Undo 是 canonical mutation。

因此：

```
generation += 1
```

Redo 也是。

历史 entry 不存储“恢复 generation”。

---

# 38. Undo Cursor

Undo 后：

返回：

```
cursor_before
```

供未来 editor 恢复。

Redo 后：

返回：

```
cursor_after
```

DocumentState 本身不持有 caret。

---

# 39. Undo Failure Atomicity

这是 hard invariant。

如果 Undo 失败：

```
Document text
Undo stack
Redo stack
Generation
```

全部必须保持失败前状态。

不得：

```
先 pop
→ mutation fail
→ entry 丢失
```

推荐：

```
peek / validate
→ apply
→ success 后 move stack
```

或者等价 transactional order。

Redo 同理。

---

# 40. Undo Grouping 总规则

连续编辑可以合并，但只针对明显安全情况。

默认 grouping window：

```
750 ms
```

---

# 41. Typing Grouping

允许：

```
a
b
c
```

成为一条：

```
"abc"
```

条件：

- 两次都是 `Typing`。
    
- 时间差 ≤750 ms。
    
- 没有 selection replacement。
    
- 后一次 insert 恰好接在前一次 inserted text 后面。
    
- 都不是换行。
    
- 都不是 IME commit。
    
- 没有 side-effect。
    
- cursor 连续。
    

任何不确定情况：

```
不要 merge
```

保守优先。

---

# 42. Backspace Grouping

例如：

```
abc
```

连续 backspace：

```
c
b
a
```

可以合并为一次 Undo 恢复：

```
abc
```

必须正确处理：

```
deleted string order
range expansion direction
cursor before/after
```

需要专项测试。

---

# 43. DeleteForward Grouping

连续 Delete：

可以合并。

必须保证：

```
deleted content 顺序
```

与原文一致。

---

# 44. 不允许 Grouping 的情况

以下永远单独 entry：

```
Paste
ImeCommit
Newline
SelectionReplace
Other
```

特别：

```
一次 IME commit
=
一次 UndoEntry
```

未来 Microsoft Pinyin：

```
"你好"
```

一次 commit 后：

```
Ctrl+Z
```

必须整体撤销。

---

# 45. 不跨类型 Merge

例如：

```
Typing
Backspace
Typing
```

不得组成一条。

---

# 46. Line Break

按 Enter：

```
EditKind::Newline
```

单独 Undo entry。

不要和前后文字 typing 合并。

---

# 47. Saved Generation

DocumentState 至少包含：

```
generation: Generation
saved_generation: Generation
```

初始从 durable note load 创建：

```
generation = 0
saved_generation = 0
```

Phase 2 不读文件，但 model 按此语义建立。

---

# 48. Dirty 定义

当前 v1 采用安全、保守定义：

```
is_dirty =
generation != saved_generation
```

因为 generation 单调递增。

注意：

```
edit
→ undo 回到与磁盘完全相同的内容
```

generation 仍然不同，因此仍可能显示 dirty。

这是允许的保守行为。

未来 Autosave 会写回相同内容并重新 clean。

不要为了优化这个边界：

- 保存完整 saved text 副本。
    
- 引入复杂 incremental hash。
    
- 降低 generation。
    

正确性和简单性优先。

---

# 49. Persist Acknowledgement Contract

Phase 2 不写磁盘，但必须给 Phase 4 留正确接口。

例如：

```
pub fn acknowledge_persisted(
    &mut self,
    generation: Generation,
) -> Result<PersistAck, DocumentError>;
```

语义：

### 情况 A

当前：

```
current gen = 10
I/O 保存 gen 10
```

ack：

```
saved_generation = 10
clean
```

### 情况 B

当前：

```
save worker 正在保存 gen 10

用户继续编辑
current gen = 12
```

I/O 回来：

```
persisted gen 10
```

结果：

```
saved_generation = 10
current = 12
仍 dirty
```

不能错误清除 dirty。

---

# 50. 不允许 future generation ack

如果：

```
persisted_generation > current_generation
```

这是逻辑错误。

返回：

```
InvalidPersistedGeneration
```

不得 silent accept。

---

# 51. Snapshot

实现：

```
pub struct DocumentSnapshot {
    text: Arc<str>,
    generation: Generation,
    line_ending: LineEnding,
}
```

如果 Phase 0 plan 将 LineEnding 放在 persistence 层而非 core：

遵循 plan。

---

# 52. Snapshot Authority

Snapshot：

```
immutable projection
```

不是 authority。

它用于未来：

```
Preview worker
Save worker
Export
```

后台 worker 不得持有：

```
&mut DocumentState
```

---

# 53. Snapshot 成本

String → `Arc<str>` 会产生一次 O(n) copy。

这是 Phase 2 当前可接受方案。

不得为了避免 snapshot copy 提前引入 rope/shared mutable architecture。

必须测量：

```
20 KiB
100 KiB
1 MiB
```

snapshot 时间。

如果 1 MiB 出现异常问题：

记录。

不立即重构。

---

# 54. LineEnding

如果 `docs/plan` 已将以下类型定义为核心对象：

实现：

```
pub enum LineEnding {
    Lf,
    Crlf,
}
```

Phase 2 不实现 disk detection。

只是 Document metadata。

内部 canonical text：

```
\n
```

durable conversion 留给 persistence Phase。

---

# 55. 内部文本不做 Unicode Normalization

不得自动：

```
NFC
NFD
NFKC
NFKD
```

用户输入的 Unicode bytes 在合法 UTF-8 范围内保持原样。

---

# 56. Error Model

建立 typed errors。

推荐类别：

```
pub enum DocumentError {
    RangeOutOfBounds,
    InvalidCharBoundary,
    InvalidTextPosition,
    DeletedTextMismatch,
    GenerationExhausted,
    InvalidPersistedGeneration,
    UndoUnavailable,
    RedoUnavailable,
}
```

可以有：

```
TextStoreError
UndoError
```

然后在 Document boundary 统一。

不要创建巨大“万能 AppError”。

---

# 57. thiserror

如果 Phase 1 已审计并批准 `thiserror`：

允许继续使用。

如果 Phase 1 没有引入：

默认优先 std 手写 error。

不要仅为了少写几个 Display impl 新增 runtime dependency。

---

# 58. Production unwrap / expect

核心 mutation 路径：

禁止：

```
unwrap()
expect()
```

测试代码可以合理使用。

内部“不可能失败”如果需要：

优先：

```
debug_assert
typed invariant
```

而不是 panic。

---

# 59. State.rs 的范围

Phase 2 不建立整个最终 AppState。

只建立确实属于 core Document domain 的 state。

不要提前创建：

```
WindowState
ThemeState
PreviewState full implementation
TrayState
DockState
```

这些已有 plan contract，但不属于当前代码 Phase。

---

# 60. 不引入通用 Event Bus

不要加入：

```
EventBus
MessageBroker
GenericCommandBus
Plugin Dispatcher
```

Phase 2 只需要清楚的函数调用与 typed outcome。

---

# 61. Outcome Types

建议：

```
pub struct EditOutcome {
    pub generation: Generation,
    pub dirty: bool,
    pub undo_recorded: bool,
    pub grouped: bool,
}
```

Undo：

```
pub struct UndoOutcome {
    pub generation: Generation,
    pub cursor: CursorSnapshot,
}
```

Redo 类似。

具体字段可根据代码简化。

要求：

> 上层不需要读取内部 stack 才知道发生了什么。

---

# 62. Mutation 必须集中

任何改变 canonical text 的函数最终必须经过统一内部 primitive，例如：

```
apply_delta_forward
apply_delta_reverse
```

或者等价设计。

不要分别在：

```
edit()
undo()
redo()
```

复制三份 String replace 逻辑。

---

# 63. Invariants 文档

在：

```
crates/stickymd-core/src/document.rs
```

或模块文档中明确写：

```
1. text is always valid UTF-8.
2. all ranges are UTF-8 char boundaries.
3. generation is monotonically increasing.
4. canonical text can only change through DocumentState mutation APIs.
5. new edits clear redo.
6. failed mutations leave state unchanged.
7. snapshot is immutable and non-authoritative.
8. persisted acknowledgements cannot acknowledge future generations.
```

---

# 64. Managed Image Undo 现在怎么处理

Phase 2 **不实现图片文件事务**。

不要实现：

```
MoveToTrash
RestoreFromTrash
filesystem side effect
```

但必须确认 Undo 模型未来可以增加 managed-asset side effect，而无需：

```
改变 DocumentState public mutation model
重写 Undo stack 架构
```

方式可以是：

- `UndoEntry` 为 crate-private，将来增加字段；
    
- 或预留清晰 internal extension point。
    

禁止为了未来 side effect 创建：

```
通用插件 effect system
通用 transaction VM
dynamic boxed callback
```

在 Phase 2 report 中写明：

```
Phase 8 can extend private UndoEntry with managed asset effects without changing public document API.
```

即可。

---

# 65. Tests：TextStore

至少：

```
empty text
ASCII insert
CJK insert
emoji insert
replace
delete
start insertion
end insertion
full replacement
out of bounds
inside UTF-8 codepoint boundary rejection
```

例如：

```
"中"
```

UTF-8 3 bytes。

不得允许 range：

```
1..2
```

---

# 66. Tests：Selection

至少：

```
collapsed
forward selection
reverse selection
start/end normalization
end of document
invalid boundary
```

---

# 67. Tests：Generation

至少：

```
edit increments
undo increments
redo increments
no-op does not increment
persist ack does not increment
future persist ack rejected
```

---

# 68. Tests：Undo

至少：

```
edit → undo restores original
undo → redo restores edited
new edit clears redo
empty undo returns typed error
empty redo returns typed error
```

---

# 69. Tests：Typing Group

输入：

```
a
b
c
```

时间：

```
0
100
200 ms
```

必须：

```
one undo
→ remove abc
```

---

# 70. Typing Timeout

输入：

```
a @ 0ms
b @ 1000ms
```

必须：

```
two undo entries
```

---

# 71. IME Group Test

模拟：

```
ImeCommit("你好")
```

然后：

```
Typing("a")
```

必须：

```
two independent undo entries
```

一次 Undo：

```
remove a
```

第二次：

```
remove 你好
```

---

# 72. Newline Group Test

```
abc
Enter
def
```

至少形成：

```
Typing abc
Newline
Typing def
```

三组。

---

# 73. Backspace Group Test

例如：

```
abcd
```

连续 Backspace：

```
d
c
b
```

然后一次 Undo：

恢复：

```
abcd
```

必须验证 deleted string 顺序没有反。

---

# 74. DeleteForward Group Test

例如 caret 在：

```
a|bcd
```

连续 Delete：

```
b
c
d
```

一次 Undo：

恢复：

```
abcd
```

---

# 75. SelectionReplace

例如：

```
hello world
```

选择：

```
world
```

替换：

```
中国
```

Undo：

```
hello world
```

CursorSnapshot 同时恢复。

---

# 76. 4 MiB Budget

测试：

- 小 entry 累积。
    
- 超过 256 entries。
    
- 超过 4 MiB。
    
- 最旧 entry 正确淘汰。
    
- redo bookkeeping 不打破预算。
    
- single oversized entry。
    

---

# 77. Redo 与 Budget

Undo entry 移动到 redo 时：

不得被重复计算为两份内存。

你必须明确：

```
budget 是 undo+redo 总历史 payload
```

推荐如此。

如果采用其它规则：

必须写 plan/report 理由。

默认：

```
Undo + Redo combined history <= 4 MiB
```

---

# 78. Failure Atomicity Tests

人为构造：

```
stale delta
invalid boundary
invalid range
```

确认失败后：

```
text unchanged
generation unchanged
undo unchanged
redo unchanged
dirty unchanged
```

---

# 79. Deterministic Randomized Test

在不引入 `rand` 的情况下，建议实现一个测试内小型 deterministic PRNG，例如简单 LCG。

固定 seed。

生成：

```
ASCII
中文
emoji
combining sequences
newline
```

进行合法 char-boundary edits。

记录最终文本。

然后：

```
undo all
```

必须回到初始文本。

再：

```
redo all
```

必须回到最终文本。

注意：

因为 256 / 4 MiB history limit 会淘汰旧 entry：

此 randomized roundtrip 应限制操作数量和 payload，使历史不触发淘汰。

另外单独测试淘汰行为。

---

# 80. Unicode Fixture

至少包含：

```
ASCII
中文测试
é
e\u{301}
🙂
👨‍👩‍👧‍👦
数学 ∑ ∫ α β
```

Core 不需要理解 grapheme。

但必须：

```
永远保持 valid UTF-8
```

---

# 81. Property Dependency

默认不要新增 `proptest`。

如果 Agent 强烈认为 property-testing crate 对本阶段价值明显：

必须：

1. 只作为 dev-dependency。
    
2. 写 dependency analysis。
    
3. 说明为什么 deterministic randomized test 不足。
    
4. 不得增加 runtime binary dependency。
    

默认方案：

```
std-only deterministic randomized tests
```

---

# 82. Performance Smoke Baseline

不要引入 Criterion。

建立一个：

```
ignored performance smoke test
```

或者：

```
example / tool
```

使用：

```
std::time::Instant
```

---

# 83. Benchmark 场景

至少：

## 20 KiB

测：

```
append 1 char
middle insert
middle delete
snapshot
undo
redo
```

## 100 KiB

同上。

## 1 MiB

同上。

---

# 84. Measurement

每个 operation：

- warm-up。
    
- 重复至少 100 次，小操作可 1000 次。
    
- 记录 median。
    
- p95。
    
- max。
    

不要把 Debug build 数据作为结论。

使用：

```
cargo test --release ...
```

或 Release benchmark executable。

---

# 85. 性能目标

Phase 2 的主要目标不是绝对性能，而是确认 String model 没有被证伪。

初始目标：

```
1 MiB common edit p95 < 50 ms
```

理想：

```
< 16–33 ms
```

特别关注：

```
middle insert
middle delete
snapshot
```

如果某一个 1 MiB worst-case >50 ms：

不要直接引入 rope。

先：

1. 重复测量。
    
2. 检查 benchmark 是否包含 allocation/setup。
    
3. 检查 Release。
    
4. 记录真实 UI relevance。
    
5. 写 Phase 2 report。
    

只有出现明确 blocker 才创建：

```
docs/report/phase-02-text-store-risk.md
```

交 USER 决策。

---

# 86. 不做 micro-optimization

禁止：

```
unsafe memcpy
custom allocator
SIMD
small-string magic
手写 UTF-8 parser
```

除非已经有 measurement 证明必要。

本阶段没有这样的预授权。

---

# 87. Snapshot Memory

确认：

```
snapshot()
```

只在显式调用时复制。

不得在：

```
generation()
text()
is_dirty()
can_undo()
```

这种 read API 中发生隐式全文 clone。

---

# 88. Clone Audit

本阶段 review 中重点搜索：

```
rg "\.clone\(\)" crates/stickymd-core/src
```

逐个确认大 String clone 是否必要。

不要求零 clone。

要求：

> clone 必须发生在明确 projection/snapshot 边界。

---

# 89. Dependency Audit

最终：

```
cargo tree -p stickymd-core
```

目标：

`stickymd-core` runtime dependency 应极少。

理想：

```
std only
```

或最多：

```
thiserror
```

如果 core 直接依赖：

```
winit
cosmic-text
comrak
ratex
windows
softbuffer
tiny-skia
```

这是架构错误。

立即修复。

---

# 90. Cross-platform Build Gate

即使 v1 是 Windows 11：

`stickymd-core` 必须：

```
cargo check -p stickymd-core
cargo test -p stickymd-core
```

在非 Windows host 正常。

Core 中不得：

```
#[cfg(target_os = "windows")]
```

除非只是极特殊 compile test，原则上应该完全没有。

---

# 91. Documentation 更新

本阶段不能随意改 plan。

允许更新：

- stable anchors。
    
- 已验证实现映射。
    
- 术语 cross-link。
    
- coverage matrix Future Code → Actual Code。
    
- Phase task。
    
- report。
    

如果发现 plan 本身被实际实现事实证伪：

停止相关实现。

写：

```
docs/report/phase-02-architecture-review-required.md
```

不要把 plan 改成迁就代码。

---

# 92. Coverage Matrix

更新：

```
docs/coverage-matrix.md
```

至少映射：

```
runtime document state
text mutation
undo / redo
generation
snapshot
```

到实际代码。

例如：

|Plan|Feature|Acceptance|Code|
|---|---|---|---|
|runtime state|Source editing|AC-002|`stickymd-core/src/document.rs`|
|editor/IME|Undo/Redo behavior|AC-009|`stickymd-core/src/undo.rs`|
|persistence|dirty generation|AC-005 future|`stickymd-core/src/generation.rs`|

不要声称 AC-002 或 AC-009 已完整完成。

因为 UI 尚未实现。

可标：

```
Core contract implemented; end-to-end acceptance pending.
```

---

# 93. Phase 2 Task Document

创建：

```
docs/tasks/phase-02-core-document-model.md
```

结构：

```
Status
Purpose
Prerequisites
Scope
Out of Scope
Modules
Invariants
Deliverables
Verification
Performance Baseline
Risks
Result
```

开始：

```
Status: In Progress
```

结束：

```
Status: Completed — awaiting USER review
```

---

# 94. Phase 2 Report

创建：

```
docs/report/phase-02-core-document-report.md
```

必须包含：

# Phase 2 Core Document Report

## Executive Result

```
Document authority: PASS / FAIL
TextStore model: PASS / CONDITIONAL / FAIL
TextDelta: PASS / FAIL
Generation: PASS / FAIL
Undo/Redo: PASS / CONDITIONAL / FAIL
1 MiB String model: PASS / CONDITIONAL / FAIL
```

---

## Repository Baseline

```
starting commit
Phase 1 decision
```

---

## Final Module Map

列出文件和职责。

---

## Public API

列出主要 public types/functions。

---

## Invariants Proven

至少：

```
single mutation gateway
monotonic generation
UTF-8 boundaries
failure atomicity
bounded history
redo invalidation
stale persisted-generation protection
```

---

## Unicode Verification

列出 fixtures。

---

## Undo Grouping

表格：

|   |   |   |
|---|---|---|
|Kind|Merge?|Rule|
|Typing|Yes|adjacent + ≤750ms|
|Backspace|Yes|contiguous reverse delete|
|DeleteForward|Yes|contiguous forward|
|Paste|No|standalone|
|IME|No|commit atomic|
|Newline|No|standalone|
|SelectionReplace|No|standalone|

---

## Memory Budget

说明：

```
256
4 MiB
approximation formula
oversized entry behavior
```

---

## Performance

表：

|   |   |   |   |   |   |   |
|---|---|---|---|---|---|---|
|Size|Append|Middle Insert|Delete|Snapshot|Undo|Redo|
|20 KiB|||||||
|100 KiB|||||||
|1 MiB|||||||

记录：

```
median
p95
max
```

---

## Dependencies

最终 core runtime dependency。

---

## Architecture Drift

如果无：

```
None.
```

---

## Deferred Responsibilities

明确：

```
grapheme navigation → Phase Editor
IME preedit → Phase Editor
disk persistence → Phase Persistence
managed asset side effects → Phase Assets
Preview snapshot consumer → Phase Preview
```

---

## Recommendation

只能：

```
APPROVE next phase
APPROVE next phase WITH CONDITIONS
STOP — architecture review required
```

不要自行开始下一阶段。

---

# 95. Tests 命名

测试名称应表达 invariant。

例如：

```
edit_rejects_non_char_boundary()
undo_restores_text_and_cursor()
redo_restores_edited_text()
generation_never_decreases()
new_edit_invalidates_redo()
persist_ack_for_stale_generation_keeps_document_dirty()
persist_ack_for_current_generation_marks_clean()
oversized_edit_succeeds_without_recording_undo()
undo_failure_is_state_atomic()
typing_within_window_groups()
ime_commit_never_groups()
```

不要写：

```
test1
test_edit
works
```

---

# 96. Documentation Test

Public core types 应有简短 rustdoc。

不要写大段重复 plan。

Rustdoc 应回答：

```
这个类型是什么
authority 是什么
关键 invariant 是什么
```

详细设计仍引用 plan。

---

# 97. Clippy

不得：

```
#[allow(clippy::...)]
```

随意关闭 lint。

如果确有合理原因：

局部 allow + 注释。

---

# 98. Format & Baseline

最终必须：

```
cargo fmt --check

cargo clippy \
  --workspace \
  --all-targets \
  -- -D warnings

cargo test --workspace --locked

cargo build --workspace --release --locked

cargo test -p stickymd-core --release --locked

git diff --check
```

---

# 99. Core Dependency Check

执行：

```
cargo tree -p stickymd-core
```

最终回复中贴摘要。

---

# 100. Forbidden Architecture Check

执行搜索：

```
rg \
  "winit|cosmic_text|comrak|ratex|windows::|softbuffer|tiny_skia|tokio|tauri|wry" \
  crates/stickymd-core
```

预期：

- `plan_ref` / doc 文本可能出现。
    
- runtime import 不应出现。
    

---

# 101. Production Code Review

完成后必须逐文件 review：

```
document.rs
text_store.rs
edit.rs
selection.rs
generation.rs
undo.rs
snapshot.rs
state.rs
error.rs
```

检查：

### Cohesion

每个文件是否只承担一个稳定责任。

### Coupling

是否出现：

```
Undo knows UI
Document knows Windows
Selection knows pixel
Generation knows persistence worker
```

### Authority

是否有任何 bypass mutation。

### Failure

所有 validation 是否在 mutation 前完成。

### Clone

大文本 clone 是否只有明确 snapshot/undo payload。

### Future replacement

String → Rope 是否可在不改变 public Document API 前提下替换。

---

# 102. Review Subagent

如果本地 Agent 支持 review subagent：

最多使用 3 个。

建议分工：

### Reviewer 1

```
State invariants + failure atomicity
```

### Reviewer 2

```
Unicode + Undo grouping correctness
```

### Reviewer 3

```
Architecture boundaries + performance
```

主 Agent 必须自己决定是否接受 findings。

不能把 review 判断权外包。

如果没有 subagent：

进行显式 self-review。

---

# 103. 不要人为追求“零分配”

Document edit 必然会有：

```
insert/delete ownership
undo history
snapshot
```

当前目标：

```
bounded
predictable
measured
```

不是：

```
zero allocation
```

---

# 104. Debug Assertion

内部可以用：

```
debug_assert!
```

检查不可被外部触发的 invariant。

但任何 USER/runtime 输入导致的非法 state：

必须返回 typed error。

---

# 105. Panic Policy

`stickymd-core` runtime public API：

目标：

```
invalid input → Result
```

而不是：

```
panic
```

`Index` 等标准 trait 如果没有必要：

不要实现会产生 panic 的接口。

---

# 106. Public API 最小化

默认：

```
pub(crate)
```

只有上层真实需要使用的才：

```
pub
```

不要因为“以后可能需要”把内部 UndoEntry 全部公开。

特别：

```
UndoEntry
Undo stack representation
StringTextStore internals
group merge algorithm
```

应尽量 crate-private。

---

# 107. Serialization

Phase 2 不给 DocumentState 加：

```
Serialize
Deserialize
```

DocumentState 是 runtime state，不是磁盘格式。

未来：

```
note.md
config.toml
```

有自己的 durable representation。

避免把 runtime model 直接变成 persistence schema。

---

# 108. Thread Safety

不要为了未来 worker 强制所有类型：

```
Arc<Mutex<...>>
Send + Sync everywhere
```

当前 DocumentState 设计原则：

> 单 owner mutable state。

未来 UI/Main coordination thread 持有 DocumentState。

Worker 获取：

```
immutable DocumentSnapshot
```

不要共享可变 DocumentState。

---

# 109. 禁止全局状态

不得出现：

```
static mut
lazy global DocumentState
global Mutex<DocumentState>
```

DocumentState 由未来 AppState 显式拥有。

---

# 110. Phase 2 可接受的正式代码量

不要为了“完整框架”制造数千行。

本阶段期望：

```
核心实现：
约 800–1800 行 Rust

tests：
约 800–2000 行
```

这不是硬限制。

如果明显超过：

review 是否过度设计。

---

# 111. Git Commit 建议

如果起始 clean：

可以分：

```
feat(core): establish canonical document state

feat(core): add bounded undo and redo

test(core): verify unicode and document invariants

docs: record phase 2 core document contract
```

不要 push。

如果 USER 更希望单 commit：

可以最终保留一个：

```
feat(core): establish StickyMD document model
```

不要自行 rebase unrelated history。

---

# 112. 最终 Git Status

结束时：

```
git status --short
git log -5 --oneline
```

如果所有修改属于本阶段且初始 clean：

可以本地 commit。

如果初始 dirty：

默认不 commit。

---

# 113. 最终回复格式

必须严格：

# Phase 2 Result

## Preconditions

```
Phase 1 recommendation
USER approval
starting commit
```

## Repository State Before Work

```
branch
clean/dirty
```

## Files Created

列出。

## Files Modified

列出。

## Core Model Implemented

### DocumentState

说明 authority。

### TextStore

说明 String 实现。

### TextDelta

说明 range semantics。

### Generation

说明 monotonic semantics。

### Undo/Redo

说明 limits/grouping。

### Snapshot

说明 projection。

## Invariants

逐项 PASS/FAIL：

```
single authority
no mutable text bypass
UTF-8 boundaries
monotonic generation
failure atomicity
redo invalidation
bounded undo
stale persistence ack safety
```

## Test Results

给出：

```
total tests
passed
failed
ignored performance tests
```

## Unicode Tests

说明：

```
CJK
emoji
combining
mixed script
```

## Undo Grouping

结果表。

## Performance Baseline

表：

```
20 KiB
100 KiB
1 MiB
```

## Core Dependencies

贴：

```
cargo tree -p stickymd-core
```

摘要。

## Unsafe

必须：

```
stickymd-core unsafe code: 0
```

## Architecture Drift

```
None
```

或列出 report。

## Verification

逐条：

```
cargo fmt
cargo clippy
cargo test
cargo build release
cargo test core release
git diff --check
```

## Documentation

说明：

```
task doc
report
coverage matrix
plan anchors
```

## Git

```
commit(s)
push = no
```

## Recommendation

只能：

```
APPROVE next phase
APPROVE next phase WITH CONDITIONS
STOP — architecture review required
```

最后：

> Awaiting USER review. Do not start the next phase automatically.

---

# 114. Phase 2 Definition of Done

只有全部满足才完成：

- Phase 1 已经 USER 批准。
    
- 所有适用治理/plan 文档已读。
    
- `stickymd-core` 仍 `forbid(unsafe_code)`。
    
- DocumentState 成为唯一 canonical runtime authority。
    
- StringTextStore 实现。
    
- TextStore 上层接口不泄漏内部 String mutation。
    
- UTF-8 byte range model 明确。
    
- 非 char-boundary mutation 被拒绝。
    
- Selection / CursorSnapshot 建立。
    
- TextDelta 建立。
    
- deleted text 由 canonical state 产生。
    
- generation 单调递增。
    
- generation overflow fail closed。
    
- no-op 不产生 generation。
    
- Undo/Redo 实现。
    
- Undo + Redo 总预算 ≤4 MiB。
    
- history ≤256 entries。
    
- oversized edit 不破坏 canonical edit。
    
- new edit 清空 redo。
    
- typing grouping ≤750ms。
    
- Backspace grouping 正确。
    
- DeleteForward grouping 正确。
    
- Paste 不 grouping。
    
- IME commit 不 grouping。
    
- Newline 不 grouping。
    
- SelectionReplace 不 grouping。
    
- Undo/Redo cursor snapshot 正确。
    
- Undo/Redo generation 继续递增。
    
- failure atomicity 测试通过。
    
- persisted generation ack contract 实现。
    
- stale saved generation 不错误 mark clean。
    
- future generation ack 被拒绝。
    
- immutable DocumentSnapshot 实现。
    
- Snapshot 不成为 authority。
    
- deterministic Unicode randomized roundtrip 测试通过。
    
- 1 MiB String performance 被测量。
    
- core 无 Windows dependency。
    
- core 无 winit/cosmic/comrak/ratex dependency。
    
- core 无 global mutable state。
    
- runtime state 未直接 Serialize。
    
- coverage matrix 已更新。
    
- Phase 2 task 完成。
    
- Phase 2 report 完成。
    
- 所有 baseline command 通过。
    
- 没有进入 Editor/UI/Persistence/Preview 正式开发。
    
- 没有自行开始下一阶段。
    

完成后停止。


# StickyMD Phase 3 — Native Source Editor, IME & Interaction Pipeline

你现在位于 StickyMD 本地 Git 仓库根目录。

Phase 0 已完成工程治理与架构合同。

Phase 1 已完成技术基础与高风险 Spike。

Phase 2 已完成核心 DocumentState / TextDelta / Generation / Undo/Redo。

USER 已批准进入 Phase 3，但附带一个前置条件：

> **在正式 Source Editor 实现前，补齐 Phase 2 缺失的 Release-profile 文本性能基线。**

本阶段名称：

> **Phase 3 — Native Source Editor, IME & Interaction Pipeline**

---

# 0. 本阶段核心目标

本阶段正式实现 StickyMD 的源码编辑能力。

最终形成：

```
Windows / winit events
        │
        ▼
Interaction Shell
        │
        ▼
Typed Intent
        │
        ▼
Instruction Interface
        │
        ▼
Editor Coordination
        │
        ▼
DocumentState
        │
        ▼
Document Mutation Result
        │
        ▼
Editor Projection
        │
        ▼
cosmic-text
        │
        ▼
tiny-skia
        │
        ▼
softbuffer
```

并正式解决：

```
keyboard input
mouse selection
caret
scroll
clipboard text
Undo/Redo
Microsoft Pinyin
WeChat Input Method
CJK / Latin font runs
IME preedit
IME commit
IME candidate positioning
```

---

# 1. Phase 3 明确不做什么

本阶段禁止正式实现：

```
note.md load/save
autosave
atomic file write production integration
external file watcher
file conflict
crash recovery UI

Markdown preview
Comrak production pipeline
RaTeX production pipeline
Preview mode
Split mode

image paste
image GC
asset trash
export

tray
docking
auto-hide
multi-monitor restore
theme selector
opacity selector
Always-on-top control

RichEdit fallback
```

注意：

Phase 1 中可能已经存在相关 Spike。

**Spike 存在 ≠ Phase 3 可以顺手正式集成。**

---

# 2. 当前阶段的产品状态

Phase 3 结束后，开发构建应该表现为：

> 一个 Windows 11 原生、只存在于内存中的 Markdown 源码文本编辑窗口。

它可以：

- 输入文本。
    
- 输入中文。
    
- 选择。
    
- 编辑。
    
- Copy/Cut/Paste text。
    
- Undo/Redo。
    
- 滚动。
    

它不能：

- 保存。
    
- 打开 `note.md`。
    
- Preview。
    
- 渲染 Markdown。
    
- 使用图片。
    
- Dock。
    
- Tray。
    

为了避免 USER 误把 Phase 3 build 当成可用便签，窗口标题必须明确包含类似：

```
StickyMD — Phase 3 Dev Build — NOT PERSISTED
```

不得假装数据已经持久化。

---

# 3. 开始前必须读取

严格执行根 `AGENTS.md`。

至少读取：

```
AGENTS.md
docs/AGENTS.md
docs/plan/AGENTS.md

docs/plan/00_engineering_constitution.md
docs/plan/01_terminology.md
docs/plan/02_positioning_and_scope.md
docs/plan/03_system_architecture.md
docs/plan/04_runtime_state_model.md
docs/plan/07_editor_and_ime.md
docs/plan/09_windows_shell.md
docs/plan/10_performance_reliability.md
docs/plan/11_testing_and_release.md

docs/features/00_v1_product_behavior.md
docs/acceptance-cases/00_v1_acceptance.md
docs/coverage-matrix.md

docs/report/phase-01-technical-spike-report.md
docs/report/phase-01-performance-baseline.md
docs/report/phase-02-core-document-model.md
docs/tasks/phase-02-core-document-model.md
```

如果 Phase 1 有：

```
phase-01-ime-risk.md
```

或任何 `CONDITIONAL` 项：

必须先读。

---

# 4. Phase 1 IME Gate

在写正式 Source Editor 前，从 Phase 1 报告提取：

```
Microsoft Pinyin:
PASS / CONDITIONAL / FAIL / NOT TESTED

WeChat IME:
PASS / CONDITIONAL / FAIL / NOT TESTED
```

规则：

### Microsoft Pinyin = FAIL

立即停止。

创建：

```
docs/report/phase-03-precondition-ime-blocked.md
```

不得开始正式 Source Editor。

### Microsoft Pinyin = CONDITIONAL

确认 USER 是否已经接受对应风险条件。

没有 USER 明确接受：

停止。

### WeChat = NOT TESTED

允许开始实现，但：

> Phase 3 不能最终标记 Completed。

必须在 Phase 3 完成前进行真实 WeChat Input Method 手工验证。

### WeChat = FAIL

可以继续实现通用编辑器，但 Phase 3 最终 Recommendation 必须是：

```
STOP — IME architecture review required
```

除非本阶段成功修复。

---

# 5. Phase 2 Release Performance Backfill Gate

这是本阶段开始后的第一个实际任务。

Phase 2 Agent 只提供了 Debug profile：

```
1 MiB append ≈ 2.2 µs
1 MiB middle insert ≈ 11.6 µs
```

这些数据不能作为正式性能结论。

---

# 6. 补测要求

在修改 DocumentState 实现前，先使用当前 Phase 2 commit 执行 Release baseline。

至少测：

```
20 KiB
100 KiB
1 MiB
```

操作：

```
append
middle insert
middle delete
snapshot
undo
redo
```

要求：

- `--release`
    
- warm-up
    
- deterministic fixture
    
- setup 不计入 operation time
    
- 每项至少 100 次
    
- 快操作可 ≥1000 次
    
- 报告 median / p95 / max
    

---

# 7. StringTextStore Gate

结果满足：

```
1 MiB common edit p95 < 50 ms
```

则记录：

```
StringTextStore Phase 3 Gate: PASS
```

继续。

如果：

```
p95 >= 50 ms
```

不要立即改 Rope。

必须：

1. 检查 benchmark 是否正确。
    
2. 检查 setup 是否混入计时。
    
3. 检查 Release。
    
4. 重复测试。
    
5. 分析 UI 实际路径。
    

确认属于结构性问题后创建：

```
docs/report/phase-03-text-store-review.md
```

停止 Source Editor 实现，等待 USER 决策。

---

# 8. 更新 Phase 2 Report

把 Release baseline 追加到：

```
docs/report/phase-02-core-document-model.md
```

不要重写历史结论。

新增章节：

```
## Phase 3 Preflight Release Baseline
```

注明：

```
Measured during Phase 3 preflight.
```

---

# 9. 开始前仓库状态

执行：

```
git status --short
git branch --show-current
git log -8 --oneline
cargo metadata --no-deps
```

记录 starting commit。

如果 dirty：

- 不 reset。
    
- 不 clean。
    
- 不覆盖 USER 文件。
    
- 不自动混合 commit。
    

---

# 10. 四层架构在 Phase 3 的正式落地

Phase 3 是第一次真正实现：

```
Interaction Shell
→ Instruction Interface
→ Flow Coordination
→ Execution Domain/Core
```

不得把全部逻辑塞进：

```
fn event_loop(...)
```

---

# 11. Interaction Shell 职责

Interaction Shell 只负责：

```
winit event capture
mouse position
keyboard state
IME event capture
visual editor session
caret/selection presentation
window redraw requests
scroll presentation
```

Shell 可以拥有：

```
EditorSession
EditorProjection
WindowPresentationState
```

Shell 不得拥有：

```
另一个 authoritative String
另一个 authoritative DocumentState
业务文件状态
```

---

# 12. Instruction Interface

建立极薄 typed intent 层。

本阶段只实现当前真正需要的 Intent。

建议：

```
enum AppIntent {
    Edit(EditRequest),
    Undo,
    Redo,

    CopySelection,
    CutSelection,
    PasteText,
}
```

如果现有计划术语略有不同，遵循 plan。

不要提前加入几十个未来 Intent。

---

# 13. 不需要 Intent 的表现变化

以下属于纯 Interaction Shell presentation：

```
mouse hover
caret blink
selection drag visual state
scroll offset
window resize
```

这些不需要进入业务 Intent。

这体现工程宪法：

> 表现变化不天然等于业务状态变化。

---

# 14. Flow Coordination

建立明确的：

```
EditorCoordinator
```

或等价模块。

职责：

```
Intent
→ canonical validation
→ DocumentState mutation
→ platform capability if needed
→ typed result/effect
```

它是唯一允许 Interaction 层请求 Document mutation 的路径。

---

# 15. 不允许 Interaction Shell 直接：

```
document.edit(...)
document.undo(...)
document.redo(...)
```

如果 Shell 能拿到：

```
&mut DocumentState
```

架构失败。

必须通过 Coordinator / Instruction boundary。

---

# 16. 推荐调用模型

```
Winit Key Event
      │
      ▼
InteractionShell::translate()
      │
      ▼
AppIntent::Edit(...)
      │
      ▼
InstructionDispatcher
      │
      ▼
EditorCoordinator
      │
      ▼
DocumentState::edit()
      │
      ▼
EditorEffect::DocumentChanged(...)
      │
      ▼
Interaction Shell updates projection
```

---

# 17. AppEffect

不要让 Shell 读取内部 Undo stack 或自己推断 mutation 结果。

建立 typed effect。

例如：

```
enum AppEffect {
    DocumentEdited {
        generation: Generation,
        range: Range<usize>,
        inserted: String,
        cursor: CursorSnapshot,
    },

    DocumentResync {
        snapshot: DocumentSnapshot,
        cursor: CursorSnapshot,
    },

    ClipboardWritten,

    ClipboardUnavailable,

    NoOp,
}
```

具体 shape 可以优化。

原则：

> Effect 是执行结果，不是第二份业务状态。

---

# 18. DocumentState 所有权

推荐：

```
AppRuntime / EditorCoordinator
    owns DocumentState
```

Interaction Shell：

```
不得拥有 DocumentState
```

Render projection：

```
不得拥有 DocumentState
```

需要读取时：

通过：

```
read-only accessor
snapshot
typed query
effect
```

---

# 19. Source Editor 状态分离

必须区分：

```
Document State
Editor Session State
Editor Projection State
```

---

# 20. Document State

来自 Phase 2：

```
text
generation
saved_generation
undo
redo
```

它是 canonical authority。

---

# 21. Editor Session State

属于 Interaction Shell。

建议：

```
struct EditorSession {
    selection: Selection,
    scroll: ScrollState,
    preferred_x: Option<f32>,
    ime: ImeCompositionState,
    dragging_selection: bool,
    caret_visible: bool,
}
```

不要包含 canonical text。

---

# 22. Editor Projection

属于 render/presentation。

建议概念：

```
struct SourceProjection {
    projected_generation: Generation,
    ...
}
```

内部可以有：

```
cosmic-text Buffer
font/layout caches
visual line layout
```

但：

> 它只是一份可以随时从 DocumentState 重建的 projection。

---

# 23. Projection invariant

任何时刻：

```
projected_generation <= document_generation
```

完成同步时：

```
projected_generation == document_generation
```

如果发现：

```
projection cannot apply incremental delta
```

必须：

```
full rebuild from canonical snapshot
```

不得：

```
让 projection 的 text 反向覆盖 DocumentState
```

---

# 24. cosmic-text authority rule

`cosmic-text Buffer` 内部会包含用于 shaping 的文本。

这在工程上是允许的重复表示，但必须满足：

```
non-authoritative
generation-tagged
replaceable
rebuildable
never persisted directly
never used as source of truth for save
```

必须在代码模块文档中写清。

---

# 25. Phase 3 模块建议

根据已有代码适当调整，但建议：

```
apps/stickymd-win/src/
├─ main.rs
├─ app.rs
├─ interaction/
│  ├─ mod.rs
│  ├─ keyboard.rs
│  ├─ mouse.rs
│  └─ ime.rs
├─ instruction/
│  ├─ mod.rs
│  └─ intent.rs
├─ flow/
│  ├─ mod.rs
│  ├─ editor.rs
│  └─ clipboard.rs
└─ platform/
   └─ windows/
      ├─ mod.rs
      └─ clipboard.rs
```

`stickymd-render`：

```
crates/stickymd-render/src/
├─ lib.rs
├─ source/
│  ├─ mod.rs
│  ├─ projection.rs
│  ├─ layout.rs
│  ├─ fonts.rs
│  ├─ hit_test.rs
│  └─ paint.rs
```

不要机械拆分。

如果 6 个小文件合成 3 个更内聚文件更合理，优先 cohesion。

---

# 26. stickymd-render 边界

必须仍：

```
#![forbid(unsafe_code)]
```

可以依赖：

```
cosmic-text
tiny-skia
```

不得依赖：

```
windows
winit platform extensions
filesystem
DocumentState mutable internals
```

---

# 27. softbuffer 的位置

`softbuffer` 属于：

```
Interaction / platform surface
```

不是 `stickymd-render` 领域模型。

推荐：

```
stickymd-render
→ produce/draw Pixmap
→ stickymd-win
→ present using softbuffer
```

这样 render crate 未来仍可跨平台。

---

# 28. Windows API 边界

Phase 3 尽量不新增 Win32 API。

优先使用：

```
winit
cosmic-text
```

可能需要 Windows-specific 的只有：

```
clipboard fallback
IME edge behavior workaround
```

新增 Win32 API 前必须：

1. 检查 winit / Rust crate 是否已有可靠 abstraction。
    
2. 写入 dependency/API report。
    
3. 隔离在 `platform/windows/`。
    
4. 有 SAFETY 注释。
    

---

# 29. Text Input 输入来源

普通字符输入必须来自 winit 正确的 text input path。

不要：

```
KeyCode → 手工映射 ASCII
```

因为这会破坏：

- keyboard layout。
    
- dead key。
    
- international input。
    

---

# 30. IME 与普通 Text 输入不能重复提交

这是 Phase 3 最重要 invariant 之一。

Windows IME 环境下，可能同时观察到：

```
KeyboardInput
IME events
```

必须确保：

> 一个用户输入只能形成一次 canonical Document mutation。

特别：

```
Ime::Commit("你好")
```

发生时不得又通过 KeyboardInput 插入：

```
"你好"
```

第二次。

---

# 31. IME State

正式实现：

```
enum ImeCompositionState {
    Inactive,

    Enabled,

    Preediting {
        text: String,
        cursor: Option<Range<usize>>,
        replacement: Selection,
    },
}
```

具体名称可调整。

---

# 32. IME preedit 不属于 DocumentState

当收到：

```
Ime::Preedit
```

只更新：

```
EditorSession.ime
```

禁止：

```
DocumentState::edit()
generation += 1
Undo push
preview dirty
future autosave dirty
```

---

# 33. Preedit visual model

视觉上：

```
canonical text before selection
+
temporary preedit
+
canonical text after selection
```

但不能真的拼成 authoritative String。

可以：

### 方案 A

使用临时 cosmic-text composition projection。

### 方案 B

paint 阶段额外绘制 preedit run。

### 方案 C

建立临时 layout-only virtual string。

选择最稳定方案。

关键要求：

```
IME preedit destruction
→ canonical text 不变
```

---

# 34. IME replacement selection

如果 composition 开始时用户选中：

```
ABC
```

IME preedit 视觉上应替换该 selection。

但 canonical Document：

直到 Commit 前仍保留原文本。

Commit 时：

```
one EditRequest
range = captured selection
inserted = committed text
kind = ImeCommit
```

---

# 35. IME Commit

收到：

```
Ime::Commit(text)
```

必须：

```
one commit
→ one AppIntent
→ one DocumentState mutation
→ one UndoEntry
→ one generation increment
```

---

# 36. IME Empty Commit

如果 commit text 为空：

根据 winit 实际语义判断。

默认：

```
不产生 canonical edit
```

除非它明确代表 selection deletion。

不要凭猜测修改。

需要测试。

---

# 37. IME Cancel

composition 被取消：

```
preedit cleared
Document unchanged
generation unchanged
Undo unchanged
```

---

# 38. Candidate Window

每当以下任一变化：

```
caret moves
selection changes
scroll changes
window moves
window resize
DPI changes
font metrics change
IME preedit changes
```

都必须更新：

```
window.set_ime_cursor_area(...)
```

候选框位置应基于：

```
current visual caret rectangle
```

---

# 39. Candidate coordinate model

必须明确：

```
editor local DIP
→ window logical/physical coordinate
→ winit IME cursor area
```

不要混用：

```
DIP
physical px
document byte offset
```

---

# 40. DPI

至少验证：

```
100%
125%
150%
200%
```

125% 是现实 Windows 常见配置，应加入正式测试。

---

# 41. Focus

获得 focus：

```
enable IME as needed
caret visible
```

失去 focus：

```
preedit state must settle safely
```

不得出现：

```
focus loss
→ half committed composition enters Document
```

如果平台行为复杂：

记录实际 winit event sequence。

---

# 42. Source Editor 字体规则

正式实现：

### 中文/CJK

首选：

```
仿宋_GB2312
```

fallback：

```
仿宋
FangSong
system CJK fallback
```

### Latin

首选：

```
Times New Roman
```

fallback：

```
system serif
```

---

# 43. 不内置仿宋

禁止：

```
把仿宋字体文件复制到 repo
embed proprietary font
```

只使用本机系统字体。

---

# 44. Script segmentation

Source text 必须按 script/font family 建立 run。

可以使用成熟、小型 Unicode crate。

优先考虑：

```
unicode-script
```

或已验证等价 crate。

不要手写庞大 Unicode range table。

---

# 45. Dependency audit

如果 Phase 3 新增：

```
unicode-script
unicode-segmentation
clipboard crate
```

必须更新：

```
docs/report/phase-03-dependency-delta.md
```

包含：

```
crate
version
license
purpose
runtime cost
transitive deps
why required
replaceability
```

---

# 46. Grapheme navigation

Phase 2 明确把 grapheme behavior 留给 Editor。

Phase 3 必须正式解决。

使用成熟：

```
Unicode grapheme cluster segmentation
```

优先：

```
unicode-segmentation
```

不要自己实现 Unicode grapheme algorithm。

---

# 47. ArrowLeft / ArrowRight

必须按：

```
grapheme boundary
```

移动。

例如：

```
e + combining acute
```

视觉上作为一个 grapheme。

Family emoji：

```
👨‍👩‍👧‍👦
```

也不得让 caret 停在 ZWJ 中间的任意 UTF-8 byte。

---

# 48. Backspace

如果 selection 非空：

```
delete selection
EditKind::SelectionReplace 或等价语义
```

如果 selection collapsed：

删除前一个 grapheme。

---

# 49. Delete

如果 selection collapsed：

删除后一个 grapheme。

---

# 50. Home / End

至少实现：

```
Home → current visual/source line start
End → current visual/source line end
```

必须在 plan/report 中说明是：

```
logical source line
```

还是：

```
wrapped visual line
```

建议 v1 Source editor：

```
Home/End = visual wrapped line
Ctrl+Home/Ctrl+End = document start/end
```

如果 cosmic-text hit model 对 visual line 支持成熟，使用这个行为。

否则 Phase 3 可先：

```
Home/End = logical line
```

但必须明确记录。

不要模糊。

---

# 51. Up / Down

需要实现基本 vertical caret movement。

应维护：

```
preferred_x
```

使连续：

```
Down
Down
Down
```

尽量保持视觉列。

不要把 `preferred_x` 放 DocumentState。

它属于 EditorSession。

---

# 52. Mouse click

单击：

```
hit test
→ byte position
→ collapsed selection
```

必须只返回合法 UTF-8/grapheme position。

---

# 53. Drag selection

Mouse down + drag：

```
anchor fixed
active follows hit test
```

允许反向 selection。

拖到 viewport 上/下边缘：

可以实现有限 auto-scroll。

如果复杂度过高：

至少保证 viewport 内拖选正确。

但必须在 report 标明是否支持 edge auto-scroll。

---

# 54. Double click

Phase 3 可以不实现复杂 Unicode word selection。

不是 gate。

不要为了 double-click 引入 Word Boundary 大系统。

---

# 55. Triple click

不实现。

---

# 56. Selection painting

必须清楚显示：

- active selection。
    
- inactive selection 可以稍淡。
    
- Light dev theme 即可。
    

不做最终 Theme system。

---

# 57. Caret

caret：

```
1–2 physical px
```

或合理 DPI-scaled width。

必须：

- 不低于 1 physical pixel。
    
- 跟随当前 text run line height。
    
- blink。
    

---

# 58. Caret Blink

不得创建永久 60 FPS redraw。

使用低频 timer：

例如：

```
500–600 ms
```

只在切换 blink state 时 request redraw。

输入后：

```
caret immediately visible
blink deadline reset
```

---

# 59. Idle CPU

由于 caret blink 存在，窗口会低频 redraw。

仍必须：

```
idle CPU average < 0.1%
```

不要使用：

```
continuous animation loop
```

---

# 60. Scrolling

Source Editor 必须支持：

```
mouse wheel
caret-follow scroll
selection scroll
```

scroll state：

```
EditorSession
```

不进入 DocumentState。

---

# 61. Horizontal scroll

建议 Source Editor：

```
soft wrap = ON
```

因此默认不需要一般横向滚动。

Markdown 源码可以 wrap。

这更符合便签本体。

如果 Phase 0 plan 已冻结其它行为，以 plan 为准。

---

# 62. Soft Wrap

建议使用 viewport width 进行 wrap。

窗口 resize：

```
re-layout projection
DocumentState unchanged
generation unchanged
```

这是典型 presentation-only change。

---

# 63. Scroll 不改变 generation

必须测试：

```
scroll
resize
caret move
selection change
```

全部：

```
Document generation unchanged
```

---

# 64. Keyboard Shortcuts

Phase 3 正式实现：

```
Ctrl+A
Ctrl+C
Ctrl+X
Ctrl+V
Ctrl+Z
Ctrl+Y
```

Windows 通常用户也期待：

```
Shift + Arrow
Ctrl + Shift + Arrow
```

但复杂 word-navigation 不属于硬 gate。

至少：

```
Shift + Arrow
```

必须工作。

---

# 65. Ctrl+S

本阶段没有 Persistence。

不要做假的保存。

建议：

```
Ctrl+S
→ no-op + dev diagnostic
```

或者完全不处理。

不得：

```
写任何 note.md
```

窗口 title 已明确 NOT PERSISTED。

---

# 66. Ctrl+Shift+S

导出尚未实现。

不处理。

---

# 67. Copy

Copy selection 流程：

```
Interaction
→ CopySelection Intent
→ Coordinator
→ read canonical selected text
→ ClipboardPort::write_text
```

不要从：

```
cosmic-text internal buffer
```

复制。

Canonical source 必须来自 DocumentState。

---

# 68. Cut

Cut 必须具备失败安全。

正确顺序：

```
read canonical selection
→ clipboard write
→ if clipboard write succeeds
→ canonical delete
```

如果 clipboard 写入失败：

```
Document 不得删除
```

---

# 69. Paste

流程：

```
PasteText Intent
→ ClipboardPort::read_text
→ if text exists
→ replace current selection
```

Phase 3：

```
只处理 text
```

Clipboard 是图片：

```
不处理
```

不得提前进入 Phase 8。

---

# 70. ClipboardPort

定义薄 capability，例如：

```
trait ClipboardPort {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError>;
    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError>;
}
```

不要让 Coordinator 直接使用 Win32。

---

# 71. Clipboard implementation

优先：

- Phase 1 已经验证的 crate。
    
- 或经过本 Phase dependency audit 的小型 crate。
    

如果使用：

```
arboard
```

必须检查：

- 版本。
    
- 默认 features。
    
- 是否因 image support 拉入不必要依赖。
    
- 能否关闭不需要 feature。
    

Phase 3 只要 text clipboard。

---

# 72. 不要为了 Clipboard 进入 Win32，除非有必要

如果成熟 Rust crate：

- 足够小。
    
- 行为可靠。
    
- 无重型 dependency。
    

优先 crate。

如果 crate 引入明显大依赖：

可以考虑薄 Win32 adapter。

但必须记录比较。

---

# 73. Normal text typing

普通文本输入：

```
keyboard text
→ current selection replacement
→ EditKind::Typing
```

如果输入是：

```
"\r"
```

canonical Document 内必须仍使用：

```
"\n"
```

---

# 74. Enter

Enter：

```
insert "\n"
EditKind::Newline
```

不要插 `\r\n` 到 runtime DocumentState。

---

# 75. Tab

Phase 3 不需要高级 Markdown indent engine。

建议：

```
Tab → insert 4 spaces
```

如果 plan 未定义且 Agent认为不应新增行为：

也可以先不处理 Tab。

必须在 report 明确。

不要自动实现复杂列表缩进。

---

# 76. Undo

Ctrl+Z：

```
Intent::Undo
→ DocumentState::undo
→ cursor returned
→ EditorProjection resync
→ EditorSession selection restored
```

---

# 77. Redo

同理。

---

# 78. Undo projection strategy

普通 typing：

推荐：

```
Document successful edit
→ projection applies equivalent incremental replacement
```

Undo/Redo：

第一正式实现可以：

```
DocumentState undo/redo
→ obtain current snapshot
→ full projection rebuild
```

因为 Undo/Redo 频率低。

不要为了减少一次全文 rebuild 提前改 Phase 2 public API。

---

# 79. Projection incremental failure

如果 incremental cosmic-text projection update 失败或出现 generation mismatch：

必须：

```
discard projection state
→ full rebuild from DocumentSnapshot
```

不得：

```
panic
continue with diverged text
```

---

# 80. Projection verification

Debug/test build 应提供：

```
projection_text == canonical_text
```

的验证能力。

不一定每个 release keypress 都执行全文比较。

测试必须执行。

---

# 81. Source Editor 字号

Phase 3 可以采用预定默认：

```
16 DIP
```

行高：

```
约 1.55
```

不要加入用户配置。

---

# 82. Dev Visual Style

本阶段只需要：

```
Light
white-ish paper
dark text
selection color
caret
IME underline
```

不要实现：

```
三态主题
圆角设置
最终 toolbar
opacity slider
```

---

# 83. Window Chrome

如果 Phase 1 有 minimal window skeleton：

复用经过验证的 window foundation。

但不要开始做最终 frameless paper chrome。

本阶段可以使用普通系统窗口框架。

因为：

> Source Editor correctness > final window aesthetics。

---

# 84. IME preedit style

Preedit 至少：

```
underline
```

并与 selection/caret 可区分。

如果 IME 提供 preedit subrange：

应尽可能显示当前 composition cursor。

不要求复杂 candidate styling。

---

# 85. Font fallback logging

启动时 Debug build 记录：

```
CJK selected family
Latin selected family
fallback status
```

例如：

```
CJK: 仿宋_GB2312 → found
Latin: Times New Roman → found
```

如果仿宋不存在：

```
CJK: 仿宋_GB2312 missing
fallback: FangSong
```

不得 panic。

---

# 86. Font lookup cache

Font family resolution：

```
once per FontSystem initialization
```

不要每个 glyph / keypress 查系统 font database。

---

# 87. Font run segmentation cache

不要提前做复杂全局缓存。

对当前 visible paragraph / layout 做合理 segmentation 即可。

先测量。

---

# 88. IME Manual Acceptance — Microsoft Pinyin

必须真实执行。

### IME-MP-001 Basic

输入：

```
nihao
```

选：

```
你好
```

Expected：

```
Document contains exactly "你好"
generation increases once
one undo removes entire "你好"
```

---

# 89. IME-MP-002 Mixed

输入：

```
这是 Rust 的 trait 示例
```

检查：

- 无重复字符。
    
- 无丢字符。
    
- CJK font 正确。
    
- Latin font 正确。
    
- caret 位置正确。
    

---

# 90. IME-MP-003 Preedit

输入拼音但暂不 commit。

检查：

```
preedit visible
Document text unchanged
generation unchanged
Undo unchanged
```

---

# 91. IME-MP-004 Cancel

开始 composition。

按：

```
Esc
```

取消候选。

Expected：

```
preedit disappears
Document unchanged
```

注意未来产品 Esc 还用于 dock collapse。

Phase 3 没有 docking，因此这里只测试 IME 语义。

---

# 92. IME-MP-005 Selection Replacement

Document：

```
hello world
```

选中：

```
world
```

IME 输入：

```
世界
```

Expected：

```
hello 世界
```

一次 Undo：

```
hello world
```

---

# 93. IME-MP-006 Candidate Position

测试：

```
100%
125%
150%
200%
```

Candidate window：

```
near visual caret
```

不能漂移到：

- 窗口左上。
    
- 上一行。
    
- 其它 monitor。
    

---

# 94. IME-MP-007 Scroll

把 caret 放到需要滚动的底部。

开始输入。

Candidate：

```
仍跟随 visible caret
```

---

# 95. IME-MP-008 Refocus

composition 完成后：

```
Alt+Tab away
Alt+Tab back
继续输入
```

不能：

- 丢焦点。
    
- 重复 commit。
    
- candidate 错位。
    

---

# 96. WeChat Input Method

完整重复：

```
IME-WX-001 ... IME-WX-008
```

不得抽样只做一两个。

---

# 97. NOT TESTED 不算通过

如果机器没有微信输入法：

最终：

```
WeChat IME: NOT TESTED
```

则 Phase 3：

```
不得标记 fully completed
```

Recommendation：

```
APPROVE NEXT PHASE WITH CONDITION
```

或：

```
STOP
```

由风险程度决定。

但不得写 PASS。

---

# 98. IME Bug 修复规则

出现 blocking IME bug：

先做：

```
Root Cause Pass 1
```

检查：

- event ordering。
    
- preedit state。
    
- duplicate keyboard text。
    
- cursor area。
    
- focus。
    
- selection replacement。
    

再做：

```
Root Cause Pass 2
```

做最小复现。

---

# 99. RichEdit 仍然禁止直接启用

两轮修复后仍有 blocking bug：

创建：

```
docs/report/phase-03-ime-fallback-review.md
```

包含：

```
Reproduction
OS build
IME version
winit version
cosmic-text version
Observed event sequence
Attempt 1
Attempt 2
Likely root cause
RichEdit fallback architecture
Expected memory impact
Expected maintenance impact
Cross-platform impact
```

然后停止。

虽然 USER 已原则允许 RichEdit 作为最终 fallback：

**实际启用 fallback 仍应由 USER 查看报告后明确批准。**

---

# 100. Synthetic IME Tests

除了人工测试，必须建立平台无关的 composition state-machine unit tests。

模拟：

```
Enabled
Preedit("ni")
Preedit("nihao")
Commit("你好")
```

Expected：

```
only Commit mutates DocumentState
```

---

# 101. Duplicate event test

模拟：

```
Keyboard text candidate
Ime Preedit
Ime Commit
```

确保最终只插入一次。

如果真实 winit event ordering 不容易合成：

至少抽象 translator 层后测试。

---

# 102. Composition replacement tests

覆盖：

```
collapsed caret
forward selection
reverse selection
CJK selection
emoji selection
```

---

# 103. Grapheme Tests

必须包括：

```
"a"
"中"
"é"
"e\u{301}"
"🙂"
"👨‍👩‍👧‍👦"
"🇨🇳"
```

---

# 104. Backspace Grapheme

每个 fixture：

一次 Backspace：

```
删除完整 grapheme
```

不能产生 invalid UTF-8。

---

# 105. Arrow Grapheme

Caret 不允许停在：

```
UTF-8 codepoint 中间
combining sequence 中间
ZWJ family emoji 内
```

---

# 106. Mouse Hit Test

需要至少测试：

```
line beginning
middle
line end
wrapped line
CJK glyph
Latin glyph
mixed font boundary
```

hit result：

必须映射 canonical byte offset。

---

# 107. Selection + Mixed Font

例如：

```
这是 Rust 测试
```

拖选从：

```
是
```

到：

```
u
```

Copy：

必须得到正确 UTF-8 substring。

---

# 108. Clipboard Tests

使用 mock `ClipboardPort` 做自动测试。

至少：

```
copy success
copy failure
cut success
cut clipboard failure
paste success
paste empty
paste read failure
```

---

# 109. Cut failure invariant

测试：

```
clipboard write fails
```

Expected：

```
Document unchanged
generation unchanged
undo unchanged
```

---

# 110. Clipboard真实 Windows smoke

手工：

```
StickyMD → Notepad
Notepad → StickyMD
Chinese
Latin
emoji
multiline
```

---

# 111. Performance baseline — Source Editor

Release build。

文档尺寸：

```
20 KiB
100 KiB
1 MiB
```

测试：

```
single Latin char input
single CJK IME commit
Backspace
Delete
selection replace
Undo
Redo
projection update
full projection rebuild
```

---

# 112. 输入 latency 测量范围

从：

```
translated canonical input intent
```

到：

```
frame ready for presentation
```

至少可以拆：

```
Document mutation
projection update
layout
paint
```

不要只测 DocumentState，然后声称 editor latency。

---

# 113. 初始 Phase 3 目标

Initial engineering target：

|Fixture|p95|
|---|---|
|20 KiB normal typing|≤16 ms|
|100 KiB normal typing|≤25 ms|
|1 MiB normal typing|≤50 ms|
|100 KiB full projection rebuild|≤50 ms desirable|
|1 MiB full projection rebuild|≤200 ms desirable|

这些是工程目标。

不是对外宣传数据。

---

# 114. 如果 1 MiB incremental typing 慢

先检查：

```
是不是每次 keypress full rebuild
是不是每次 clone whole document
是不是每次重新初始化 FontSystem
是不是每次扫描完整 Unicode scripts
是不是每次重建 framebuffer
```

不要第一反应：

```
换 Rope
换 GPU framework
```

---

# 115. Memory baseline

正式 Windows Release build。

测试：

### Empty

窗口 + editor，无文本。

### 20 KiB

混合中文/英文。

### 100 KiB

### 1 MiB

---

# 116. Measurement

启动后：

```
等待 30 秒
无输入
无 resize
只有正常 caret blink
```

记录至少 5 次。

报告：

```
median
max
```

优先：

```
Private Working Set
Private Bytes / Commit
```

---

# 117. Source Editor Memory Goal

本阶段 exploratory hard threshold：

```
20 KiB Source Editor <= 40 MiB
```

如果稍超：

不要立即 FAIL。

先分析：

- debug artifacts？
    
- font cache？
    
- unused Spike crates？
    
- duplicate buffers？
    
- softbuffer allocation？
    
- cosmic font database？
    

如果明显超过且为架构固有成本：

写 risk report。

---

# 118. Idle CPU

平均：

```
< 0.1%
```

测试至少 60 秒。

Caret blink 不得造成持续高 CPU。

---

# 119. Redraw audit

记录 redraw 触发来源：

```
input
selection
scroll
resize
caret blink
IME preedit
```

不存在：

```
permanent frame timer
```

---

# 120. Framebuffer allocation audit

确认：

```
keypress
```

不会重新创建整个 surface。

Resize 才允许 resize buffer。

---

# 121. Clone Audit

执行：

```
rg "\.clone\(\)" \
  apps/stickymd-win/src \
  crates/stickymd-render/src
```

逐项 review 可能的大型：

```
String
Arc<str>
Vec
Pixmap
glyph buffers
```

不要求零 clone。

要求没有：

```
每按一个字复制 1 MiB 文档多次
```

---

# 122. Projection Correctness Audit

在 Debug/Test build 增加辅助检查：

```
canonical hash
projection source hash
```

或等价。

在：

```
random edit sequence
undo
redo
IME commit
paste
```

后检查一致。

---

# 123. Editor Randomized Test

使用固定 seed。

操作：

```
insert
delete
move caret
select
replace
undo
redo
```

包含：

```
ASCII
CJK
emoji
combining
newline
```

每步检查：

```
valid UTF-8
valid selection
projection generation <= canonical generation
projection text == canonical when synchronized
```

---

# 124. 不引入 Markdown parser

虽然输入的是 Markdown 文本：

Source Editor 不需要 Comrak。

Phase 3 不得：

```
为了 **bold** 上色而引入 parser
```

Source 是纯源码编辑。

---

# 125. 不做 Syntax Highlight

明确禁止：

```
Markdown syntax highlighting
heading color
code fence color
math color
```

字体 run 只按 Unicode script。

这符合极简与性能要求。

---

# 126. 不做 Preview

任何：

```
Comrak
RaTeX
```

只能仍存在于 Phase 1 experiments。

不得被 Phase 3 production app 初始化。

检查：

```
cargo tree -p stickymd-win
```

如果 Source-only build 因正式依赖关系自动加载 RaTeX/Comrak 大量 runtime：

需要审查 dependency topology。

---

# 127. Production dependency topology

目标：

```
stickymd-win
├─ stickymd-core
├─ stickymd-render
├─ winit
├─ softbuffer
└─ thin clipboard/platform deps
```

`stickymd-render` Source path：

```
cosmic-text
tiny-skia
```

Markdown/Math dependency 可以未来作为 render crate feature 或模块，但：

> Source-only 启动不得主动初始化数学字体。

---

# 128. 不要为了未来 feature flags 过度设计

如果 Phase 1 workspace 已经合理组织：

保持。

不要现在构建：

```
editor-core
editor-backend
editor-plugin
render-backend-trait
window-provider
```

一堆抽象 crate。

---

# 129. Logging

本阶段需要基本可诊断性。

可以使用 Phase 1 已批准的轻日志方案。

至少 Debug 能记录：

```
IME event sequence
projection resync
font fallback
clipboard errors
generation mismatch
```

不得打印：

```
用户完整便签内容
```

尤其不要把用户文本写入 crash/log。

---

# 130. Privacy

任何日志：

不得记录：

```
full document text
clipboard content
IME committed text
```

可以记录：

```
length
event kind
generation
range
error code
```

例如：

```
IME Commit len=6 generation=42
```

而不是：

```
IME Commit "秘密内容"
```

---

# 131. Error Handling

UI runtime 出错：

例如 clipboard failure。

不得 panic。

通过：

```
typed error
→ coordinator effect
→ transient dev diagnostic
```

Phase 3 不需要正式 toast UI。

可以状态栏/标题 debug message。

---

# 132. IME Error

IME 本身不是 Result API。

如果 state sequence 不符合预期：

```
fail conservative
clear preedit if necessary
never invent canonical text
```

---

# 133. Selection invalidation

Document mutation 后：

如果当前 Interaction selection 不再合法：

必须通过 mutation outcome 设置到合法 cursor。

不要 clamp 到 byte 中间。

---

# 134. Scroll caret into view

每次：

```
typing
IME commit
arrow navigation
undo/redo
mouse selection
```

若 active caret 超出 viewport：

滚动使其可见。

---

# 135. Window Resize

Resize：

```
projection relayout
selection preserved by byte offsets
Document unchanged
generation unchanged
```

---

# 136. DPI Change

DPI：

```
font/layout rebuild
selection preserved
scroll adjusted if needed
Document unchanged
```

---

# 137. Mouse Wheel while IME active

允许滚动。

但 candidate area 必须跟着更新。

Document 不变。

---

# 138. Undo while IME preediting

这是危险边界。

建议规则：

如果 IME 当前 preedit：

```
Ctrl+Z
```

优先交给 IME / cancel composition，不直接操作 Document history。

必须根据真实 Windows/winit event behavior验证。

不要让：

```
Document undo
+
preedit still floating
```

出现。

---

# 139. Clipboard while IME preediting

Copy：

可以复制 canonical selection。

Cut/Paste：

建议先结束/cancel composition，避免 range anchor 失效。

实际策略必须：

- 一致。
    
- 测试。
    
- 写入 editor/IME contract。
    

不要静默产生混乱 state。

---

# 140. Mouse selection while IME preediting

点击其它位置：

建议：

```
cancel/settle composition
→ then move caret
```

实际 behavior 依据 winit/Windows。

必须人工验证。

---

# 141. IME State Transition Documentation

更新：

```
docs/plan/07_editor_and_ime.md
```

仅添加实际验证后的实现细节，不修改骨架。

应该有状态图：

```
Inactive
  ↓ enable
Enabled
  ↓ preedit
Composing
  ├─ preedit update → Composing
  ├─ commit → Enabled
  ├─ cancel → Enabled
  └─ disable → Inactive
```

---

# 142. Architecture Effect Diagram

更新：

```
docs/overview/architecture.md
```

加入已实现 Phase 3 path：

```
Winit
↓
Interaction
↓
Intent
↓
EditorCoordinator
↓
DocumentState
↓
Effect
↓
SourceProjection
↓
tiny-skia
↓
softbuffer
```

Overview 是 projection。

Plan 仍是 authority。

---

# 143. coverage matrix

更新实际 code path：

```
AC-002 Source Editing
AC-003 Microsoft Pinyin
AC-004 WeChat IME
AC-009 Undo/Redo
```

状态不要夸大。

例如：

```
AC-002: core + source shell implemented
AC-003: verified
AC-004: verified
AC-009: source integration verified
```

---

# 144. Phase 3 Task

创建：

```
docs/tasks/phase-03-source-editor-ime.md
```

至少：

```
Status
Prerequisites
Phase 2 Release Backfill
Scope
Out of Scope
Architecture Mapping
Modules
IME Gate
Clipboard
Font Model
Performance Gates
Manual Verification
Risks
Result
```

开始：

```
Status: In Progress
```

结束：

```
Status: Completed — awaiting USER review
```

如果 WeChat 未测：

```
Status: Implementation Complete — verification incomplete
```

不要伪造 Completed。

---

# 145. Phase 3 Report

创建：

```
docs/report/phase-03-source-editor-ime.md
```

结构必须包括：

# Phase 3 Source Editor & IME Report

## Executive Result

```
Phase 2 Release Backfill:
PASS / FAIL

Interaction Pipeline:
PASS / CONDITIONAL / FAIL

Source Editor:
PASS / CONDITIONAL / FAIL

Microsoft Pinyin:
PASS / CONDITIONAL / FAIL / NOT TESTED

WeChat Input Method:
PASS / CONDITIONAL / FAIL / NOT TESTED

Unicode Grapheme Editing:
PASS / FAIL

Clipboard Text:
PASS / FAIL

Source Performance:
PASS / CONDITIONAL / FAIL

Source Memory:
PASS / CONDITIONAL / FAIL
```

---

# 146. Architecture Implementation Map

必须列：

|   |   |
|---|---|
|Layer|Implementation|
|Interaction Shell|...|
|Instruction Interface|...|
|Flow Coordination|...|
|Core capability|`DocumentState`|
|Object Plane|`doc::text`, etc|
|Render Projection|...|
|Windows Adapter|...|

---

# 147. Authority Audit

明确回答：

```
Who owns canonical text?
Who owns selection?
Who owns IME preedit?
Who owns cosmic-text Buffer?
Can cosmic-text write DocumentState directly?
Can UI mutate DocumentState directly?
Can clipboard read from projection instead of canonical text?
```

正确答案必须符合 plan。

---

# 148. IME Event Evidence

报告至少给一个真实 Microsoft Pinyin event sequence。

不要包含用户输入文本本身。

例如：

```
Ime::Enabled
Ime::Preedit len=2
Ime::Preedit len=5
Ime::Commit len=6
```

同时列出：

```
generation before
generation after
undo entries delta
```

---

# 149. Font Evidence

报告：

```
仿宋_GB2312 found? yes/no
selected CJK family
Times New Roman found? yes/no
selected Latin family
fallback behavior
```

---

# 150. Manual Matrix

必须完整表：

|   |   |   |
|---|---|---|
|Case|Microsoft|WeChat|
|basic|||
|mixed CJK/Latin|||
|preedit|||
|cancel|||
|selection replace|||
|undo commit|||
|100% DPI|||
|125% DPI|||
|150% DPI|||
|200% DPI|||
|scroll|||
|refocus|||

---

# 151. Performance Report

表：

|   |   |   |   |   |
|---|---|---|---|---|
|Fixture|Typing p50/p95/max|IME Commit|Undo|Projection|
|20 KiB|||||
|100 KiB|||||
|1 MiB|||||

---

# 152. Memory Report

```
empty editor
20 KiB
100 KiB
1 MiB
```

至少：

```
median
max
```

---

# 153. Idle CPU

记录：

```
60s average
caret blink interval
redraw count approximate
```

---

# 154. Dependency Delta

列 Phase 3 新依赖。

尤其：

```
unicode segmentation
script segmentation
clipboard
```

必须说明 runtime/size impact。

---

# 155. Unsafe Report

列：

```
file
API
reason
SAFETY invariant
```

目标：

```
stickymd-core unsafe = 0
stickymd-render unsafe = 0
```

Windows adapter 中也应尽量少。

---

# 156. Architecture Drift

如果：

```
None
```

明确写。

如果发现：

- shell 需要业务 authority。
    
- cosmic-text 无法只做 projection。
    
- IME 必须双文本 authority。
    
- winit 无法提供稳定 IME。
    

这些属于真正风险。

必须 report。

---

# 157. Phase 3 自动化测试

至少增加：

```
intent mapping tests
editor coordinator tests
projection generation tests
projection resync tests
grapheme tests
selection tests
IME state-machine tests
duplicate input tests
clipboard mock tests
cut failure atomicity
paste tests
undo projection tests
DPI-independent coordinate transformation tests
```

---

# 158. Windows Manual Test

自动 CI 不能证明真实 IME。

必须人工运行：

```
Microsoft Pinyin
WeChat Input Method
```

并记录 Windows build。

---

# 159. CI

Phase 3 后 CI 至少仍：

### Windows

```
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --locked
cargo build --workspace --release --locked
```

### Linux

仍只要求平台无关 crate：

```
stickymd-core
stickymd-render
```

如果 render 现在依赖 cosmic-text/tiny-skia：

必须仍能 Linux check/build，除非技术事实证伪。

---

# 160. Experiment Cleanup

如果 Phase 1 editor/IME Spike 仍存在：

不要把它删掉来掩盖差异。

可以保留为历史 evidence。

正式实现不得 import：

```
experiments/phase-01/*
```

---

# 161. No Experiment Dependency

运行：

```
rg "experiments/phase-01" \
  apps crates
```

production 不应引用 experiment。

---

# 162. Cargo Tree Review

执行：

```
cargo tree -p stickymd-win
cargo tree -p stickymd-render
cargo tree -p stickymd-core
```

检查：

```
tauri
wry
webview
tokio
wgpu
```

不得出现未经批准的新架构。

---

# 163. Runtime Network Check

不得加入：

```
reqwest
hyper
curl
ureq
```

Source Editor 不需要网络。

---

# 164. File I/O Check

Phase 3 正式 app：

不得：

```
read note.md
write note.md
create note/
```

验证：

```
运行 Phase 3 dev build
→ 工作目录没有生成 note/
```

---

# 165. Data Loss Warning

因为不持久化：

窗口标题必须有：

```
NOT PERSISTED
```

README 当前状态也必须说明。

不要让测试人员输入重要数据后以为会保存。

---

# 166. Documentation Status

README 可更新：

```
Current development phase:
Native Source Editor / IME validation
```

同时：

> Persistence not implemented yet.

---

# 167. Review Subagents

如果支持，最多 3 个。

### Reviewer 1

```
IME event correctness
duplicate insertion
composition state
```

### Reviewer 2

```
authority boundaries
shell → intent → coordinator
projection divergence
```

### Reviewer 3

```
Unicode / grapheme / performance / memory
```

主 Agent必须自己判断 findings。

---

# 168. Self Review Questions

完成后逐项回答：

1. UI 是否能直接 mutate DocumentState？
    
2. cosmic-text 是否变成了第二 authority？
    
3. IME preedit 是否进入 canonical text？
    
4. IME commit 是否恰好一次 mutation？
    
5. Clipboard Copy 是否来自 canonical source？
    
6. Cut 失败是否会丢数据？
    
7. Undo 是否经过 core？
    
8. Projection stale 是否可以反写？
    
9. 1 MiB keypress 是否出现全文 clone？
    
10. FontSystem 是否每次 keypress 重建？
    
11. Idle 是否持续 redraw？
    
12. Window resize 是否改 generation？
    
13. Selection 是否错误进入 Document authority？
    
14. Win32 是否泄漏到 render/core？
    
15. 是否提前实现 persistence/preview？
    

---

# 169. Git Diff Review

执行：

```
git diff --stat <starting-commit>
git diff <starting-commit>
git diff --check
```

逐文件检查。

---

# 170. 推荐 Commit

如果起始 clean，可按：

```
feat(editor): establish source interaction pipeline
feat(editor): integrate native text layout and grapheme editing
feat(editor): integrate Windows IME composition
feat(editor): add clipboard and source editing shortcuts
test(editor): verify IME and projection invariants
docs: record phase 3 source editor results
```

或者保持更少 cohesive commits。

不要 push。

---

# 171. Baseline 验证

最终至少：

```
cargo fmt --check

cargo clippy \
  --workspace \
  --all-targets \
  -- -D warnings

cargo test \
  --workspace \
  --locked

cargo build \
  --workspace \
  --release \
  --locked

cargo test \
  -p stickymd-core \
  --release \
  --locked

cargo test \
  -p stickymd-render \
  --release \
  --locked

git diff --check
```

---

# 172. Forbidden Architecture Scan

至少：

```
rg \
  "tauri|wry|webview|tokio|wgpu|reqwest|hyper" \
  Cargo.toml \
  apps \
  crates
```

根据 false positive 人工判断。

---

# 173. Unsafe Scan

```
rg "\bunsafe\b" crates/stickymd-core
rg "\bunsafe\b" crates/stickymd-render
```

预期：

```
runtime unsafe = 0
```

---

# 174. Persistence Scan

```
rg \
  "note\.md|config\.toml|create_dir|File::create|OpenOptions" \
  apps/stickymd-win/src \
  crates/stickymd-render/src
```

任何命中都 review。

Phase 3 不应正式写盘。

---

# 175. Final User-visible Dev Smoke

手工启动 Release build。

完成：

1. 输入英文。
    
2. 输入中文。
    
3. mixed script。
    
4. arrow。
    
5. mouse selection。
    
6. Backspace。
    
7. Delete。
    
8. Enter。
    
9. Ctrl+A。
    
10. Ctrl+C。
    
11. Ctrl+X。
    
12. Ctrl+V。
    
13. Ctrl+Z。
    
14. Ctrl+Y。
    
15. resize。
    
16. scroll。
    
17. Microsoft Pinyin。
    
18. WeChat Input Method。
    
19. 125% DPI。
    
20. 200% DPI。
    

确认窗口标题：

```
NOT PERSISTED
```

---

# 176. Phase 3 Recommendation

只有三种：

```
APPROVE Phase 4
```

```
APPROVE Phase 4 WITH CONDITIONS
```

```
STOP — architecture review required
```

Phase 4 预定方向：

> **Portable Persistence, Autosave, Crash Recovery & External File Conflict**

但：

**不要自动开始 Phase 4。**

---

# 177. 最终回复格式

必须严格使用：

# Phase 3 Result

## Preconditions

```
Phase 1 IME status
Phase 2 result
USER approval
starting commit
```

## Phase 2 Release Backfill

表：

```
20 KiB
100 KiB
1 MiB
```

明确：

```
StringTextStore Gate: PASS / FAIL
```

## Repository State Before Work

```
branch
clean/dirty
```

## Files Created

列出。

## Files Modified

列出。

## Interaction Architecture

说明：

```
Shell
Intent
Coordinator
DocumentState
Effect
Projection
```

## Authority

明确：

```
Canonical text owner:
Selection owner:
IME preedit owner:
Projection owner:
Clipboard source:
```

## Source Editor

列出：

```
typing
selection
caret
scroll
grapheme
undo/redo
clipboard
```

## IME

### Microsoft Pinyin

```
PASS / CONDITIONAL / FAIL / NOT TESTED
```

### WeChat Input Method

同上。

附 matrix。

## Fonts

```
CJK family
Latin family
fallback
```

## Unicode

说明：

```
CJK
combining
emoji
ZWJ
flag
```

## Performance

完整表。

## Memory

完整表。

## Idle CPU

数据。

## Dependencies Added

表：

```
crate
version
license
purpose
runtime impact
```

## Windows APIs Added

如果无：

```
None.
```

## Unsafe

```
core = 0
render = 0
Windows adapter = ...
```

## Architecture Drift

```
None
```

或列 report。

## Verification

逐命令：

```
PASS / FAIL
```

## Documentation

```
task
report
coverage
plan refinement
README
```

## Git

```
commit(s)
push = no
```

## Recommendation

三选一。

最后：

> Awaiting USER review. Do not start Phase 4 automatically.

---

# 178. Phase 3 Definition of Done

只有全部成立才完成：

- Phase 2 Release profile baseline 已补齐。
    
- StringTextStore Gate PASS，或 USER批准替代路线。
    
- Phase 1 IME gate 已确认。
    
- Interaction Shell 不直接 mutate DocumentState。
    
- Typed Intent 层建立。
    
- EditorCoordinator 建立。
    
- Typed Effect 建立。
    
- DocumentState 仍是唯一 canonical text authority。
    
- EditorSession 不拥有 canonical text。
    
- cosmic-text Buffer 明确是 projection。
    
- SourceProjection 有 generation。
    
- stale projection 可以安全 full-resync。
    
- winit + cosmic-text 正式 Source Editor 工作。
    
- tiny-skia + softbuffer 正式绘制工作。
    
- normal Unicode text input 工作。
    
- CJK font run 工作。
    
- Latin font run 工作。
    
- 仿宋缺失 fallback 工作。
    
- grapheme ArrowLeft/Right 工作。
    
- grapheme Backspace/Delete 工作。
    
- CJK / combining / emoji / ZWJ 测试通过。
    
- mouse click hit-test 正确。
    
- drag selection 正确。
    
- scroll 正确。
    
- caret-follow scroll 正确。
    
- caret blink 不使用 permanent frame loop。
    
- Ctrl+A 工作。
    
- Ctrl+C 工作。
    
- Ctrl+X 失败安全。
    
- Ctrl+V text 工作。
    
- Ctrl+Z/Y 与 core 正确集成。
    
- image clipboard 未提前实现。
    
- IME preedit 不改 Document。
    
- IME commit 是单一 canonical mutation。
    
- IME commit 是单独 UndoEntry。
    
- IME selection replacement 正确。
    
- duplicate IME/text insertion 不存在。
    
- candidate position 正确。
    
- Microsoft Pinyin 真实测试。
    
- WeChat Input Method 真实测试或明确阻塞。
    
- 100% DPI 测试。
    
- 125% DPI 测试。
    
- 150% DPI 测试。
    
- 200% DPI 测试。
    
- window resize 不改变 Document generation。
    
- scrolling 不改变 Document generation。
    
- Source 20 KiB memory 被测量。
    
- Source 100 KiB memory 被测量。
    
- Source 1 MiB memory 被测量。
    
- 20/100/1MiB input latency 被测量。
    
- Idle CPU 被测量。
    
- 无 permanent redraw loop。
    
- core unsafe = 0。
    
- render unsafe = 0。
    
- Phase 3 不读写 `note.md`。
    
- Phase 3 不创建 `note/`。
    
- Phase 3 不初始化 Comrak production pipeline。
    
- Phase 3 不初始化 RaTeX production pipeline。
    
- 没有 Preview。
    
- 没有正式 Persistence。
    
- 没有 Tray/Dock。
    
- Phase 3 task 文档完成。
    
- Phase 3 report 完成。
    
- coverage matrix 更新。
    
- 所有 baseline checks 通过。
    
- 没有自动进入 Phase 4。
    

完成后停止。