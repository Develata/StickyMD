# 人工验收复用与产品域拆细：设计提案（2026-10-10）

> 状态：**提案，待 USER 批准**（第 1 轮审查后的修正见文末"修订"一节）。根因 3 修改 plan 11 的人工验收合同，属于骨架级变化；按 AGENTS.md 先报告、
> 经 USER 批准写入 `docs/plan/` 后才实施。本文不是契约。

## 背景与授权

USER 要求"如果一个模块没有被动过，那么对应的测试就不需要再跑一遍"，并在 2026-10-09 批准按四个根因修复；
根因 1、2（U1）已推送为 `fix/qualification-ledger-reuse`（`972aad4..fcf99dc`，九轮审查，末轮 PASS）。
2026-10-10 USER 指示："之后再去把根因3和根因4做好，同样的，还是和gpt交互多轮迭代优化清楚以后再分批
commit + push。"

- 根因 3：24 项人工验收（P12-Mxx 中的 Manual 项）不在账本里，绑定当前候选，换候选必须全部重做。
- 根因 4：产品域划分过粗（`EDITOR`、`PREVIEW` 进入几乎所有模块），复用很少触发。

早先方案审查对人工复用给出的阻断条件：依赖闭包必须有调用链依据（`app/input.rs` 同时分发窗口、Preview 与
滚轮）；环境身份必须完整（含补丁号）；同一指纹后来的 `MANUAL_FAIL` 必须优先；M34 Clean VM 保持精确制品
绑定；保留 Tier C 语义、候选绑定的 waiver 与当前候选汇总。

## 已核实现状

- **合同**（plan 11）：`MANUAL PASS` 指"当前提交上的正式人工矩阵已执行"；"一次性终端命令、未提交脚本、
  主观观察、旧 commit 收据均不能把人工项从 NOT TESTED 提升为 PASS"；"最终 manual PASS 仍额外绑定 Promoted
  Candidate"；waiver 只接受具体 `WAIVER-P12-Mxx`，身份来自同一 Source Freeze。
- **收据**：单文件 `dist/evidence/manual-acceptance.json`，绑定候选的 source/EXE/ZIP/version；续录与 readiness
  共用 `manual_receipt::validated_cases`（U1 第八、九轮）。
- **人工项**（24 项）：Tier A：M01、M02（输入法视觉）、M05（Alt+Tab 后恢复并输入）、M11/M12（左右 dock）、
  M18–M20（220×120 三种模式）、M21（缩放）、M22（透明度）、M23（主题）、M24–M26（Markdown/公式/图片视觉）；
  Tier B：M34（Clean VM）、M35–M37（多屏与拔插）、M38–M40（125/150/200% DPI）；Tier C：M41–M43（睡眠、RDP、
  负坐标显示器）。
- **域映射**（`module_ledger/fingerprint.rs`）：`app/input.rs` 只归 `EDITOR`，但它分发 IME、按键（缩放、剪贴板、
  保存/导出快捷键）、鼠标按键、光标移动与滚轮。现有自动化模块都包含 `EDITOR`，因此这一低估目前没有造成漏判；
  一旦人工项使用更细的域，它就会漏判。
- **发布历史**：v0.1.2→v0.1.3 的产品改动只有 `flow/editor.rs` 与 render crate 的 `lib.rs`、`math_text.rs`；
  v0.1.1→v0.1.2 改了约 44 个产品文件，集中在 Preview 与 source。v0.1.1、v0.1.2 发布时人工项均为 NOT TESTED，
  即目前没有任何可复用的人工 PASS；第一次采用新机制的版本仍需完整人工基线。

## 根因 3：人工项进入账本

### 模型

1. 每个人工项是一个账本模块 `manual-p12-mXX`，复用共享存储、指纹定位、锁与严格记录。evidence 是该项的单条
   观察文档（case、状态、备注、观察时的候选身份、环境身份、记录时间）。
2. **人工项指纹** = 该项声明的产品域输入 + harness 输入（人工记录器与引导会话代码、`phase-12.md` 中该行、
   `docs/plan/11` 与相关 plan 章节）+ **环境身份**。
3. **环境身份**按项声明所需维度，写入指纹：Windows 完整版本（含 UBR 补丁号）、显示器拓扑（数量、排列、每屏
   DPI）、系统主题设置、输入法 profile 与版本（M01/M02/M22/M36 等涉及 IME 的项）。补丁号变化会使声明了 Windows
   版本的项失效；这是审查要求的保守取舍，代价如实接受。
4. **记录最新观察，而非最新成功**：每个 `(项, 指纹)` 只保存最近一次正式观察。`MANUAL_FAIL` 覆盖此前的
   `MANUAL_PASS`，readiness 遇到 FAIL 即阻断；只有之后的 PASS 能再覆盖它。
5. **M34 Clean VM 不复用**：其指纹包含候选 ZIP 的 SHA-256，等价于逐候选重做（它验证的就是这份包在干净机器
   上运行）。
6. **Tier C 与 waiver 不变**：Tier C 在自动化覆盖全部通过时允许 NOT_TESTED；waiver 仍是绑定 Source Freeze 版本
   的 `WAIVER-P12-Mxx`，不进入账本、不跨版本复用。
7. **候选绑定汇总**：readiness 为当前候选写出人工汇总，逐项标明 `RAN PASS`、`REUSED PASS (origin candidate)`、
   `WAIVED`、`NOT TESTED` 或 `FAILED`。发布记录仍绑定当前候选，复用只改变"观察来自哪次运行"。
8. **记录器**：引导会话只列出 `RUN REQUIRED` 的项；每录入一项立即登记到账本（原子发布），中断不丢已录项。

### 域闭包（初始保守）

在根因 4 完成前，人工项的产品域取**全部产品域**，与审查要求一致。这意味着：任何产品代码改动都会使全部人工
项失效；只有"仅工具/文档/版本号"的版本能复用人工结果。v0.1.3 这类改了编辑器代码的版本仍需全部重测。

### plan 11 需要的修改（待批准）

- 将"旧 commit 收据均不能把人工项从 NOT TESTED 提升为 PASS"改为：人工项的正式观察按"项指纹（产品域、harness、
  环境身份）"复用；指纹不变的最近一次正式观察有效，FAIL 优先；M34 逐候选。
- `MANUAL PASS` 的定义从"当前提交上执行"改为"与当前项指纹兼容的最近一次正式观察为 PASS"，并保留 RAN/REUSED
  区分。
- 新增人工项的环境身份维度与采集方式。

## 根因 4：产品域拆细

### 步骤

1. **分类诚实化**（无产品代码改动）：所有分发/路由文件按其实际分发的全部域归类（例如 `app/input.rs` →
   `EDITOR|SHELL|PREVIEW|PERSISTENCE|EXPORT`：鼠标路径到达工具栏、滚动条与窗口（SHELL）、Preview 拖选
（PREVIEW）与 Source（EDITOR），快捷键到达保存与导出）。新增"每个产品文件都必须被显式分类"的守卫测试，
   不再依赖 `ALL_PRODUCT` 兜底。
2. **按域拆分分发**（产品代码重构，行为不变）：把 `app/input.rs`（713 行，已超过 500 行审视线）拆为编辑输入
   （IME、文本键、撤销）、窗口/壳层输入（鼠标拖动、dock 相关）、缩放、快捷键路由；路由层只做分派并归入所有
   目标域，但改动频率低。`preview_input.rs` 已独立。
3. **逐项声明人工项与自动化模块的域**：以调用链为依据，例如 M11/M12（dock）→ `SHELL`；M24（Markdown 视觉）→
   `PREVIEW`；M25 → `PREVIEW|MATH`；M26 → `PREVIEW|IMAGES`；M35/M37 → `SHELL`；涉及输入的项包含 `EDITOR`。

### 预期收益（以 v0.1.3 回放，环境不变且此前有 PASS 为前提）

v0.1.3 的改动落在 `EDITOR`（`flow/editor.rs`）与数学文本转换（`math_text.rs`，拆细后归 `EDITOR|MATH`）。按上面
的声明，M11、M12、M19、M23、M24、M26、M35、M37 约 8 项可复用，其余 16 项仍需重测。没有根因 4，全部 24 项重测。
这一数字依赖逐项域声明，需在审查中逐项核对。

### 风险

- 共享状态耦合：同一进程内不同域通过 `StickyApp` 状态互相影响，路径域只是近似；以调用链与已有自动化覆盖为依据，
  无法证明时归入更宽的域。
- 重构风险：拆分 `input.rs` 改动产品代码，需全部单元测试与 smoke 覆盖，且下一个候选必然重跑全部模块。

## 实施分批（批准后）

1. 根因 4 第 1 步（分类诚实化 + 守卫测试）。
2. 根因 3（人工项账本、环境身份、记录器、readiness 汇总、plan 11 修改）。
3. 根因 4 第 2、3 步（`input.rs` 拆分与逐项域声明）。

每批经 gpt-6-astra 多轮审查后提交并推送。

## 对规格的影响

plan 11 `#module-success-ledger` 与人工验收相关段落、`phase-12.md` 人工项说明、`phase-14.md` 新增验收项、
coverage matrix。

## 修订（第 1 轮审查后，2026-10-10）

gpt-6-astra（max）第 1 轮给出 4 个 BLOCKER、3 个 SHOULD_FIX，逐条对照代码与合同核实后**全部成立**。本节修正
上文；与上文冲突处以本节为准。

### 收益更正（先读）

- **v0.1.3 回放 = 0 项可复用**，不是"约 8 项"。`v0.1.2..972aad4` 除 `flow/editor.rs`、render `lib.rs`、
  `math_text.rs`、`math_text/tests.rs` 外还改了 `docs/plan/07_editor_and_ime.md` 与 `phase-11-b.md`；按现行规则
  （plan 11 `#module-success-ledger`："`docs/plan/`、`docs/acceptance-cases/` 因跨域约束仍对全部功能模块保守传播"，
  实现见 `fingerprint.rs:194`），在任何产品域推理之前全部模块已失效。即使忽略合同变化，render `lib.rs` 归
  `PREVIEW|EDITOR|MATH|IMAGES|EXPORT`，M19/M24/M26 不成立；M23 断言公式、图片与控件，也不成立。
- **拆细后的上限**：只有 M11/M12/M35/M37 有可能，且每项都要先证明 `window_guards`
  （`window_runtime.rs:252`）读到的 composition、搜索、导出、恢复、资产事务、保存状态的写入方都在闭包内；
  按现状这些写入方覆盖大部分产品域，实际可能接近 0。
- **补丁窗口**：环境身份含 UBR，Windows 每次累积更新（每月一次，另有可选预览更新）都使所有声明 Windows 版本的项
  失效。现实的复用场景是：**同一台机器、同一套显示器、同一补丁窗口内，且该版本没有改动该项的任何输入**——
  典型是只改发布流程、release notes、README 或非全局工具代码后重建候选。
- 因此根因 3 的主要价值不是"产品改动后少测"，而是**候选重建时不重测**，以及让下一次完整人工基线以可复用的
  格式落盘。

### 新核实的事实

- plan 11:316"可模块化 manual observation 使用 Last Successful Module Receipt"来自 `748c3d6`（2026-08-30），
  早于 U1，与 :237/:243（旧 commit 收据不能提升人工项）矛盾；"可模块化"在全仓库没有定义。这是现存矛盾。
- 账本合同"记录只允许 `PASSED`"，`01_terminology.md` 的 Last Successful Module Receipt"失败或中止不得覆盖"；
  `record.rs` 只接受 `PASSED`，`store.rs` 每模块保留 8 份、没有条件写入。记录 FAIL 是第二种证据策略，需要新术语。
- waiver：plan 11:258 只接受逐项 key，:652–655 允许 Tier B 组 waiver，代码（`manual_readiness.rs`、
  `decisions.rs`）接受 `WAIVER-TIER-B-v{version}`。这也是现存矛盾，上文"waiver 不变"的现状描述不准确。
- 记录器：逐项记录器（`manual.rs` `record`）覆盖全部 24 项；引导记录器只覆盖 14 个 Tier A 项，且 G2-04
  （M11/M12）、G2-05（M18–M20）一次回答写给多项。
- `windows_build::is_known` 接受三段版本；`ver` 实际输出四段（第四段即 UBR）。
- 工具已有一处 FFI 先例（`atomic_evidence.rs` 的 `MoveFileExW`），没有 COM。
- 产品字体按候选族名顺序取第一个存在的（`fonts.rs`，首选 `仿宋_GB2312`，通常需用户自行安装），因此相同
  Windows 版本不等于相同字体环境。
- plan 11:91 要求"当前与前一个受支持 Windows 11 版本"，现有工具不建模这一要求；这是现存缺口，不由根因 3 解决，
  单独提请注意。

### 修正后的设计

1. **一个注册表、共用存储设施、两种证据策略。** 自动化模块保持 Last Successful Module Receipt。人工项采用新
   术语 **Latest Manual Observation**：每个 `(项, 指纹)` 保存最近一次**完成**的观察，状态只能是 `MANUAL_PASS`
   或 `MANUAL_FAIL`；独立记录类型与校验；readiness 对人工项走人工策略，不进入"所有模块必须 PASS"的通用分支。
2. **生命周期。** `NOT_TESTED`、跳过、中止从不写入账本，也从不覆盖已有记录。`MANUAL_FAIL` 不参与清理（PASS
   仍按每项 8 份清理），直到同一指纹上一次完整 PASS 替换它。FAIL 优先于 waiver 与 Tier C 的非阻断规则。
   写入用条件更新：记录器把展示给观察者的当前记录版本（evidence 摘要或"无"）随提交带回，在写锁内比较，不一致
   即拒绝并要求重新观察；不用时间戳决定先后。写入失败使命令失败，不输出汇总。提供对已有 PASS 显式重测的入口，
   用于登记后来发现的 FAIL。引导会话按项提交，一步覆盖多项时逐项询问状态。
3. **指纹。** 继承现有共享/GLOBAL 规则（manifest、lock、toolchain、registry、指纹算法、证据校验器等），版本号
   规范化沿用现有窄规则；再加该项产品域、harness（记录器、引导步骤、场景表、环境 schema）、合同输入与环境身份。
   上文"只有仅工具/文档/版本号的版本能复用"改为"**不影响该项输入**的改动可复用"。
4. **环境分三类，每项逐一声明。**
   - **主机事实**（readiness 时与当前主机比较，工具经子进程或文件读取自动采集，不新增 FFI）：Windows 四段版本
     （`ver`，含 UBR；人工与 exact evidence 一并收紧为四段，`windows_build.rs` 属全局输入）、系统与应用主题
     （`reg query`）、字体目录清单摘要（系统与用户字体目录的文件名+大小，不读字体字节）。采集失败或 `UNKNOWN`
     即不可复用。
   - **场景参数**（逐场景记录，只检查是否覆盖该项要求的全部场景，不与当前主机比较）：DPI 档位、显示器拓扑、
     主题模式与运行时切换、M21 的缩放档位×视图。由观察者声明并标注来源为"声明"。场景表放在工具内，按
     plan 07/11 与 `phase-12.md` 逐项列出（例如 M01：微软拼音 × 100/150/200%；M23：Light、Dark、System、
     运行时切换）；任一场景 FAIL 则该项 FAIL，缺场景则不能登记 PASS。
   - **工具无法可靠识别的外部依赖 → 不复用，绑定精确制品**：第三方输入法会静默自动更新，TSF profile 不含产品
     版本，微软拼音还有"使用以前版本"开关，相同 profile 名不能证明行为相同。因此断言 IME 行为的项——M01、M02、
     M05、M18、M22、M36、M41——与 M34 一样每份新 ZIP 重测。以后若加入可靠的 IME 采集再放开。
5. **M34** 绑定 ZIP 的 SHA-256 与声明的 Clean VM 基线：同一份字节重新晋升可复用；新 ZIP 即使 EXE 相同也重测。
6. **waiver**：保留现有 Tier B 组 waiver，解析为具体的 Tier B 项集合；把 plan 11:258 改为与 :652–655 一致
   （逐项 key 或绑定版本与 Source Freeze 的 Tier B 组 key，不接受跨 tier 或全局 blanket）。waiver 不写成 PASS，
   不能越过已知 FAIL，不跨版本继承。
7. **候选汇总是派生结果，不是第二份权威。** 每次 readiness 重新读取最新记录；汇总列出完整候选身份，以及逐项
   指纹、场景覆盖、观察记录摘要、origin 与 waiver 引用。旧格式人工收据缺少环境与场景字段，不自动导入。
8. **根因 4 的域证据。** `app/input.rs` 的诚实分类还要包括 `MATH`（工具栏 `ConvertMath`）与 `IMAGES|ASSETS`
   （粘贴），即接近全部产品域。缩窄必须给出"入口 → 调用/回调/Effect → 共享状态读写 → 验收断言"的依据，共享
   状态的写入方进入闭包；无法证明时保持宽域。显式分类守卫是附加检查，运行时遇到未分类文件仍保守回退到全部
   产品域。拆文件只改善职责边界，不能当作解耦证明。

### plan 修改范围（整条人工证据链）

- plan 11：:237 `MANUAL PASS` 定义（与当前项指纹兼容、场景完整、最新完成观察为 PASS，并区分 RAN/REUSED）；
  :243 旧收据规则；:258–259 waiver；:316 把"可模块化 manual observation"改为指向人工策略；
  `#module-success-ledger` 增加人工策略（记录 FAIL、FAIL 不清理、条件写入、NOT_TESTED 不写入）；:336–345
  记录器与 Windows 四段版本；环境三分类与 IME 项、M34 的绑定规则。
- `01_terminology.md`：新增 Latest Manual Observation。
- 投影：`phase-12.md` 人工项说明、`phase-14.md` 新验收项、coverage matrix。
- **单独一项，不捆绑**：若要让 `docs/plan/` 的修改只影响相关章节的模块（如 07 只影响编辑/输入项），需要另改
  plan 11 中"`docs/plan/`、`docs/acceptance-cases/` 对全部功能模块保守传播"一句。这是唯一能让 v0.1.3 这类
  "改了一章 plan"的版本不全量失效的改动，但它放宽合同传播，需单独评估。

### 成本

| 项 | 成本 |
| --- | --- |
| 人工记录类型、人工策略、readiness 分支 | 中 |
| FAIL 保留与条件写入（存储层新增） | 小到中 |
| 场景表与逐场景记录（引导与逐项两个记录器） | 中 |
| 主机事实采集（`ver`、`reg query`、字体目录清单） | 小 |
| plan 11、术语、投影文档 | 中 |
| 根因 4 第 1 步：分类诚实化 + 守卫 | 小 |
| 根因 4 第 2 步：拆分 `input.rs` | 中；下一个候选全量重跑；收益只在职责边界 |
| 根因 4 第 3 步：逐项缩窄 | 证据成本高，按上文分析收益可能接近 0 |

### 选项与建议

- **选项 1（完整）**：上述修正后的根因 3 + 根因 4 全部三步。
- **选项 2（最小，建议）**：根因 3 按修正后的设计实施（产品域保守取全部产品域，IME 项与 M34 绑定 ZIP）；根因 4
  只做第 1 步。`input.rs` 拆分以后按职责边界单独做，不以复用为理由；逐项缩窄等有低成本的证据方法再议。

建议选项 2。理由：正确性优先；有时效性的是**记录格式**——下一次完整人工基线（无论落在 v0.1.3 重建还是
v0.1.4）只有按新格式记录，之后才可能复用；而缩窄产品域的收益小、证明成本高。

分批（选项 2）：根因 4 第 1 步 → 根因 3（plan/术语/投影文档一批，工具实现一批）。每批经 gpt-6-astra 审查后
提交并推送。

## 修订（第 2 轮审查后，2026-10-10）

第 2 轮：第一轮 7 项中 5 项判为已修、2 项部分修复；新增 2 个 BLOCKER 与若干 SHOULD_FIX，核实后全部成立：
字体按内部 family 名解析（`fonts.rs`；fontdb 0.23.0 `load_system_fonts` 读取 `%SYSTEMROOT%\Fonts` 与用户
`AppData\Local`、`AppData\Roaming` 下的 `Microsoft\Windows\Fonts`，cosmic-text 0.19.0 按系统 locale 选择回退），
文件名+大小不是内容身份；产品主题取 winit 的有效主题（`lifecycle.rs:86`），不是两个注册表偏好值；plan 11:319、
:433、`docs/acceptance-cases/AGENTS.md:16` 与 `phase-12.md` 前言仍写着"当前提交/exact candidate 收据"。
本节修正第 1 轮修订；冲突处以本节为准。

### 环境模型：被测环境、目标环境、readiness 所在机器三者分开

第 1 轮修订把"readiness 时与当前主机比较"作为环境判定，这是错的：开发机、Clean VM、前一个 Windows 版本的
机器各自产生合法观察，不可能同时等于执行 readiness 的那台机器。修正为：

1. **被测环境身份**：记录器在被测机器上采集，随观察写入，并在提交时重新采集比对（不一致即拒绝登记）。字段：
   Windows 功能版本与 edition（`reg query` 读 `DisplayVersion`、`EditionID`）、四段构建号（`cmd /d /c ver`，
   恰好四个数字段）、系统 locale、高对比度状态、**字体内容摘要**（fontdb 读取的三个目录下全部字体文件，按排序后
   的相对路径流式计算 SHA-256，不整份读入内存；本机约 618 MB + 97 MB，实现时实测耗时）。任一字段采集失败、
   超时或格式不符时，这次观察不能登记为有效 PASS。
2. **目标环境档案**：每次 Source Freeze 后，观察者在每台目标机器上运行一次档案采集，生成绑定该 Source Freeze
   的档案收据（字段同上）。这份收据就是"本次发布要求覆盖的环境"的当前陈述；需要哪些档案由合同决定
   （见下文待决事项），不能从已有 PASS 反推。
3. **readiness**：对每个"项 × 必需档案"，有效当且仅当存在指纹兼容、场景完整、被测环境身份等于该档案本次
   采集身份、状态为 PASS 的最新观察。readiness 在哪台机器运行不影响判定。

因此 Windows 打补丁后，下次档案采集的身份不同，该档案下的项需要重测；没打补丁、没装卸字体、没改 locale
时可以复用。

### 不复用的项（绑定当前候选 ZIP）

- 断言 IME 行为的 7 项：M01、M02、M05、M18、M22、M36、M41（第 2 轮确认清单与 `phase-12.md` 一致）。
- M34：ZIP 只证明制品身份，SmartScreen 信誉随下载历史变化；同字节重新晋升也重测。
- M42：RDP 客户端版本与连接设置不在采集范围内。

其余 15 项可按上面的环境模型复用。

### 主题

M23 的场景是 Light、Dark、System 与 System 下的运行时切换（两个方向）及实际结果；最终停在哪个主题不影响覆盖。
其他视觉项把主题作为前置条件（Light，非高对比度）记录在场景里。注册表的主题偏好值只作观察元数据。高对比度
开启时该次观察不能登记为有效 PASS（合同没有覆盖高对比度）。

### 逐项场景表

"必需场景"全部完成且 PASS 才能登记 PASS；任一场景 FAIL 立即登记该项 FAIL。场景参数由观察者声明，来源标为
"声明"。

| 项 | 必需场景 | 前置条件 | 合同来源 | 复用 |
| --- | --- | --- | --- | --- |
| M01 | 微软拼音 × 100/150/200% | Light | plan 07 Verification；功能由 G4-06 自动化 | 否 |
| M02 | WeChat/WeType × 100/150/200%；环境缺失记 NOT TESTED | Light | 同上 | 否 |
| M05 | Alt+Tab 离开后分别经点击、托盘、传感区恢复并输入 | — | plan 09 Tool Window/焦点/IME | 否 |
| M11 | 真实 mixed-DPI 双屏，左 dock：失焦收起、传感区展开、DIP/物理换算 | 双屏 mixed DPI | plan 09 dock、plan 11 :608 | 是 |
| M12 | 同上，右 dock | 同上 | 同上 | 是 |
| M18 | 220×120 Source 输入与滚动，caret/selection/IME 不被遮挡 | — | plan 09 220×120 | 否 |
| M19 | 220×120 Preview 滚动、选择、链接 | — | 同上 | 是 |
| M20 | 220×120 Split 输入与两栏滚动 | — | 同上 | 是 |
| M21 | 50/100/300% × Source/Preview/Split；滚轮缩放与 reset | — | phase-12 M21 | 是 |
| M23 | Light、Dark、System、System 下双向运行时切换 | 非高对比度 | plan 09 主题 | 是 |
| M24 | 代表性 Markdown fixture 的 Preview 视觉与选择/链接 | Light | phase-12 M24 | 是 |
| M25 | 正确与错误公式 fixture | Light | phase-12 M25 | 是 |
| M26 | PNG/JPEG/WebP/GIF、超限图片、视口下方图片的滚动显示 | Light | phase-12 M26 | 是 |
| M34 | Clean Windows 11 VM 解压运行、无额外 runtime、信誉提示与 README 一致 | Clean VM 基线（声明） | plan 11 | 否 |
| M35 | 双屏同 DPI：拖动、dock、托盘恢复 | 双屏同 DPI | plan 09 双显示器 | 是 |
| M36 | 双屏 mixed DPI：拖动、dock、IME、Preview | 双屏 mixed DPI | 同上 | 否 |
| M37 | 运行中断开当前显示器后恢复窗口 | 双屏 | plan 09 拔线 | 是 |
| M38 | 125%：输入、Preview、dock | 单屏 125% | plan 11 :91 DPI | 是 |
| M39 | 150%：同上 | 单屏 150% | 同上 | 是 |
| M40 | 200%：同上 | 单屏 200% | 同上 | 是 |
| M41 | 睡眠恢复后输入、保存、dock | — | plan 09 sleep/resume | 否 |
| M42 | RDP 重连后输入、保存、dock；无环境记 NOT TESTED | RDP | plan 09 RDP | 否 |
| M43 | 副屏在左（x<0）与在上（y<0）各一次：恢复、dock、比例保存 | 物理负坐标布局 | plan 09 双显示器左侧/上方 | 是 |

场景表放在工具内（带 `plan_ref`），属于 harness 输入；改动场景表即使相关项失效。

### 生命周期（补全）

- 状态分四种：`Absent`（无记录）、`Pass`、`Fail`、`Invalid`（记录存在但 evidence 不可读、摘要不符或校验失败）。
- 当前指纹 × 当前档案下的 `Fail` 与 `Invalid` 都阻断，且优先于 waiver 与 Tier C；`Invalid` 不回退到更早的 PASS，
  不当作 NOT_TESTED。恢复方式只有一次正式重测。
- 其他指纹或其他档案下的历史 FAIL：显示为历史，不阻断当前判定，也不称为已修复；P0/P1 问题仍走独立的发布阻断。
- 只有登记 PASS 要求场景完整；任一场景观察到 FAIL 立即登记该项 FAIL，之后中断不会丢失它。
- 开始观察与提交时都核对 Source Freeze、候选、项指纹与被测环境身份；任一变化即拒绝登记。条件写入冲突时必须
  重新读取当前记录并重新观察，不自动刷新版本后重交旧答案。
- 正式人工重测与普通诊断分开：前者写入账本，后者不写（plan 11 :433 需相应修改）。

### plan 修改范围（追加）

除第 1 轮修订所列，还需修改：plan 11 :319（readiness 接受的证据类别加入人工观察）、:433（正式人工重测与诊断的
区别）、Owned Objects/Outputs 与人工操作说明；`docs/acceptance-cases/AGENTS.md:16`；`phase-12.md` 前言；
新增目标环境档案收据。自动化的 Last Successful Module Receipt 术语与失败策略保持原义。新人工证据校验器与
场景表属于全局或 harness 输入，不能归入"只做展示"的排除项。汇总不能单独授权 READY，必须能从记录引用重新验证。

### 待 USER 决定

1. **plan 11:91 的"当前与前一个受支持 Windows 11 版本"**。工具目前只记一个 Windows 构建号，从未执行这一要求。
   - (a) 按合同建模：必需档案 = 当前版本 + 前一个版本（按功能版本与 edition 区分，不是两个补丁号）；缺档案时
     NOT_READY，除非 USER 对该档案给出绑定版本与 Source Freeze 的明确 waiver。需要第二台机器或 VM。
   - (b) 修改合同：前一个版本改为可选。
   建议 (a)：不放宽合同，缺口以 waiver 显式可见，与现有发布特例的做法一致。
2. **选项 2 是否采纳**（维持第 1 轮修订的建议）。

## Resolution（2026-10-10）

USER 选择：plan 11:91 按 **(a)** 建模（当前与前一个受支持 Windows 11 版本均为必需目标环境档案，缺档案时
NOT_READY，除非 USER 对该档案给出绑定版本与 Source Freeze 的明确 waiver）；采纳**选项 2**（根因 3 按修正后
的设计实施，产品域保守取全部产品域；根因 4 只做第 1 步）。第 3 轮设计审查的结论落入实施前的最终设计。

## 第 3 轮设计审查（2026-10-10，USER 决定之后收到）

gpt-6-astra（max）第 3 轮：第 2 轮意见多数已修；新增 3 个 BLOCKER，均针对**跨候选复用**本身：

1. "本次采集"与"不复用"缺少可校验的轮次身份：Source Freeze 与 Candidate 只含内容字段，同源再次 Freeze 或同字节
   再次晋升身份相同，旧档案可以冒充新一轮采集。需要本轮资格化/晋升标识，由现场采集器读取，且不能混入可复用项
   的内容指纹。
2. 条件写入冲突仍可吞掉已观察到的 FAIL（A 发布 PASS 后 B 的 FAIL 提交被拒，B 中断，账本仍是 PASS）。冲突中的
   正式失败必须留下阻断记录，或对同一观察键单会话排他。
3. 环境采集没有绑定实际被测进程：先启动产品再改字体或 locale，开始与提交两次采集都是新环境，产品却仍用启动时
   的字体数据库。正式会话应在采集后启动并绑定被测实例。

SHOULD_FIX：locale 应为 `GetUserPreferredUILanguages`（cosmic-text 经 sys-locale 读取），不是系统 locale；
fontdb 按未排序的目录枚举加载，同 family/style/weight 时先出现者胜，排序后的内容集合抹去了这一顺序；显示适配器、
驱动与本地/虚拟会话身份未采集（产品 CPU 光栅化，但经 GDI `BitBlt` 与 DWM 合成，并使用 layered-window alpha）；
场景表漏了 M22（本提案新增的错误；补回后仍是 9 项不复用、15 项可复用）；多机器观察如何进入同一账本未定义
（存储只在同一 clone 内共享）；(a) 的版本选择（架构、servicing channel、edition、冻结日期）、项与档案的适用关系、
环境级 waiver 的政策含义未定。另指出 plan 09 手工矩阵中 Esc、手动收起与透明度 70/96/100 在 P12 没有归属，
属于**既有**缺口。NEEDS_VERIFICATION：字体摘要只读实测约 688 MiB、WSL 流式 SHA-256 约 7.5–8.8 s；若每项开始
与提交各扫一次，每环境读取量约 33 GiB，需要合并采集。

审查同时建议把根因 3 分两步：**先建立新格式与完整观察记录（仍绑定当前候选，不跨候选复用），再在上述身份问题
闭合后开启复用**。这是已批准范围内的先后顺序，不缩小范围；采纳为实施顺序：

1. 根因 4 第 1 步（本批，见下节）。
2. 根因 3-A：逐项逐场景的观察记录、目标环境档案、两个 Windows 版本的覆盖、FAIL 优先与四种状态，**仍绑定当前
   候选**；补回 M22，并处理 Esc/手动收起/透明度档位的归属。
3. 根因 3-B：轮次身份、进程绑定、冲突下的失败持久化、locale 与字体加载顺序、显示栈身份、多机器导入，闭合后
   开启跨候选复用。

## 根因 4 第 1 步实施记录（2026-10-10）

- `fingerprint.rs` 的产品域改为三张显式表：`EVERY_SESSION`（每次会话的启动路径或事件循环每轮执行的代码，归全部
  产品域）、`NOT_YET_NARROWED`（此前落入兜底、尚未追踪闭包的文件，保持全部产品域）、`FEATURE_SOURCES`（只经某个
  功能到达的代码与其域）。没有显式规则的产品文件运行时仍归全部产品域。
- 新守卫 `every_product_source_has_an_explicit_rule`：真实 checkout 中落入兜底的产品文件与不再命中任何 tracked 文件
  的规则都使测试失败；两种变异（删一条规则、加一条不存在的规则）均实测失败。
- **只放宽不缩窄**：172 个产品相关文件逐一比较前后掩码，没有任何文件的掩码失去位；71 个文件掩码变宽。对四个窄模块
  新增的失效：
  - 16 个窗口文件（`app/{controls,toolbar_paint,window_geometry_runtime,window_interaction,window_runtime}.rs`、
    `flow/window/*`、`platform/windows/{monitor,native_message,tool_window,tray,window_opacity,window_topmost}.rs`）
    新增使 Performance 与 G3 失效——它们在启动路径上（例如 `window_runtime.rs` 记录 `guards_ready` 启动诊断，
    `lifecycle.rs` 启动时创建托盘、应用透明度与 Tool Window 样式），原先只归 `SHELL`；
  - 16 个启动、note 加载与持久化文件（`startup/*`、`persistence/*`、`flow/persistence.rs`、`*_runtime.rs` 中的持久化
    胶水、`platform/windows/{file_watch,program_dir,single_instance}.rs`）新增使 G5 失效——G5 渲染用例写入 note 后由
    `startup/bootstrap.rs` 经 `persistence` 加载；
  - `app/export_runtime.rs` 新增使 Performance 与 G4 失效（它是 `StickyApp` 胶水，`export_in_flight` 被窗口守卫读取）。
  其余变宽（如 `app/input.rs` 从 `EDITOR` 到全部产品域、剪贴板加 `ASSETS|IMAGES`、`atomic_file.rs` 加 `EXPORT`）不改变
  现有模块判定，但对日后的更细消费者是诚实的。
- 模块掩码核对：导出是保留源文本的 Markdown，只经 `render/preview` 的图片引用改写（该目录掩码含 `EXPORT`），不经数学
  渲染；G3 掩码不含 `MATH` 成立，未修改任何模块掩码。
- plan 11 `#module-success-ledger` 增加分类原则一段；`phase-14.md` 新增 P14-A72；coverage matrix 与治理范围同步到 A72。
- 验证：Windows `cargo test -p stickymd-smoke --locked` 396 + 25 通过（17 ignored），clippy `-D warnings`、fmt、
  phase-00 治理通过；WSL Debian 回放 Linux CI：clippy 通过，326 + 19 通过。

### 代码审查第 1 轮（2026-10-10）

gpt-6-astra（max）复现了"172 个文件、71 个变宽、0 个丢位"与 `domains()` 不变，但给出 2 个 BLOCKER、2 个 SHOULD_FIX，
核实后全部成立并已修正：

- BLOCKER：`flow/{save,recovery,reconciliation}.rs` 只归 `PERSISTENCE|STARTUP`，与 G5 无交集；但 `StickyApp::new`
  创建 `RecoveryCoordinator`，事件循环每轮调用 `tick_autosave`，G5 粘贴用例等待 note 落盘后又经 reconciliation。
- BLOCKER：`atomic_file.rs` 与 G5 无交集；`config/storage.rs` 的 `save_config` 经 `atomic_publish` 写配置，G5 切换视图
  后等待配置落盘。
- SHOULD_FIX：只改产品源码的定向 CI 跳过 smoke 包测试，守卫不一定运行。改为由 `phase 00` 治理调用
  `verify_product_classification`（CI plan 作业每次运行），单元测试调用同一函数；实测在未分类产品文件出现时
  `phase 00` 失败。
- SHOULD_FIX：`flow/editor.rs`、`interaction/search.rs` 在 `app.rs` 启动时初始化，`flow/preview.rs` 启动时 `show`、
  每轮 `tick`，`assets/{storage,safe_boundary}.rs` 在 `main.rs` 启动时执行，caret overlay 在默认 Source 首帧后使用；
  这些已移入 `EVERY_SESSION`。`app/` 下只处理导出等功能的文件仍归全部产品域，注释改为如实说明理由：它们共享
  `StickyApp` 可变状态，每轮代码读取（如 `export_in_flight`、`asset_paste_pending` 进入窗口守卫）。

修正后对四个窄模块新增的失效：Performance 17、G3 16、G4 1、G5 20 个文件（172 个文件中 81 个掩码变宽，0 个丢位）。
新增回归 `every_session_persistence_invalidates_every_module`：note 加载、恢复检查、autosave、reconciliation、配置存储、
原子发布与资产存储文件都必须使每个模块失效。plan 11 分类段落与 P14-A72 同步补入编辑后的持久化与治理检查。
验证：Windows 397 + 25 通过，clippy、fmt、phase-00 通过；WSL Linux 回放 clippy 通过、327 + 19 通过、phase-00 通过。

### 代码审查第 2 轮（2026-10-10）

第 1 轮修复全部属实，复算仍为 81 个变宽、0 个丢位。1 个 BLOCKER（自 HEAD 遗留）：正式 Performance 计划按名称过滤运行
Release 单元基准，其中 Phase 7 的 `phase7_export_release_baseline` 位于 `export/mod.rs` 并调用 `export_snapshot`，但
Performance 掩码不含 `EXPORT`，只改导出代码时旧 Performance 记录仍可复用。修正：Performance 掩码加入 `EXPORT`
（导出目录因此新增使 Performance 失效，合计 18 个文件）；新增回归 `performance_baselines_outside_its_features_invalidate_it`。
逐一核对含 Performance 基准的产品文件，修正后掩码均与 Performance 相交。其余结论：剩余 `FEATURE_SOURCES` 与各模块
实际执行路径一致（G3 五个用例都不切换 Preview/Split，导出不经数学排版；Performance 的 typical seed 含行内公式但以
Source 视图启动，不做数学排版）；治理调用 `git ls-files` 与 CI plan、release 工作流兼容，没有 Git 元数据的源码副本会
失败，现有工作流不使用这种环境。

未做：按 Performance 任务的名称过滤自动推导"含基准的文件必须与掩码相交"的通用守卫；当前以回归列出已知文件。

验证：Windows 398 + 25 通过，clippy、fmt、phase-00 通过；WSL Linux 回放 clippy 通过、328 + 19 通过、phase-00 通过。
