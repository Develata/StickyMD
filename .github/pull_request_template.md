## 关联 Issue 与范围确认

<!--
代码修改必须先有 Issue，并由维护者确认问题、范围和方案。
填写 Fixes #... / Relates to #...，并链接范围确认记录。
纯文档错字、失效链接或明显测试补充可以说明为什么无需预先讨论。
-->

## 问题与根因

<!-- 说明为什么需要修改；Bug PR 应描述根因，而不只是现象。 -->

## 方案

<!-- 说明实现、关键不变量、复杂度与主要权衡。 -->

## Plan / acceptance 映射

<!--
列出适用的 docs/plan stable anchor、feature、AC 和 Phase matrix。
如果没有用户行为或工程合同变化，请明确写 None，并说明理由。
-->

## Authority 与架构影响

<!--
是否触碰 DocumentState、持久化、Preview、Editor Session、资产、配置、窗口或验证 evidence authority？
是否改变四层边界、Object Plane、public contract、持久格式或线程模型？若无写 None。
-->

## 失败路径与兼容性

<!-- 失败如何传播、回退和呈现？旧 note/config/assets 是否兼容？ -->

## 性能与内存

<!--
说明复杂度、分配/复制、cache/queue 上限、UI thread 工作量。
若涉及性能路径，请附 before/after；若不适用写 Not applicable，不要伪造测量。
-->

## 依赖变化

<!--
列出名称、版本、许可证、transitive/features、runtime/体积/线程影响，以及现有依赖为何不能完成。
无新增或升级时写 None。
-->

## 验证

<!--
列出实际运行的定向测试、fmt、Clippy、workspace tests 和相关 smoke，并给出结果。
未执行的 GUI/IME/视觉/显示器/性能项目必须明确写 NOT TESTED 与原因。
-->

## 用户可见结果

<!-- 适用时附合成 fixture、截图或短视频；不得包含真实便签或私人路径。 -->

---

## Checklist

- [ ] 代码修改已有 Issue 和维护者确认的范围，或本 PR 仅为允许直接提交的文档/测试修正。
- [ ] 没有修改架构合同来迁就现有实现。
- [ ] 骨架级修改已有 `docs/report/` 分析和维护者明确批准。
- [ ] 没有引入跨层 shortcut、平级 authority 或可绕过 canonical mutation 的入口。
- [ ] 成功与失败路径都已实现并验证。
- [ ] 没有引入 WebView、Electron、Tauri、JavaScript runtime、数据库、通用 async runtime 或 runtime 网络访问。
- [ ] 文件写入继续使用批准的原子替换；用户资产不会被自动删除。
- [ ] 新依赖已经完成许可证、advisory、transitive、feature、体积和运行时影响审计。
- [ ] 相关 smoke 与 acceptance matrix 已更新；人工未验收项仍为 `NOT TESTED`。
- [ ] PR 中列出的验证命令确实在当前改动上运行。
