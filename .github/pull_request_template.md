## Summary

<!-- 1–3 句话说明本 PR 做了什么、为什么。 -->

## Plan refs

<!-- 列出受影响的契约章节，例如：docs/plan/05_document_persistence.md#atomic-save -->

## Behavior change

<!-- 用户可见行为是否变化？如何变化？ -->

## Architecture impact

<!-- 是否触碰四层边界、Object Plane、authority 模型？若无写 "None"。 -->

## Failure paths

<!-- 本 PR 涉及哪些失败路径？它们如何传播、回退、呈现给用户？ -->

## Verification

<!-- 运行了哪些测试 / 手工验收？贴命令与结果。 -->

## Performance impact

<!-- 是否影响空闲 CPU、内存、启动时间、输入延迟？若无测量数据请写 "未测量"。 -->

## New dependencies

<!-- 新增依赖：名称、许可证、transitive 数量、体积影响、为何现有依赖无法完成。若无写 "None"。 -->

## USER approval required?

<!-- 是否涉及骨架级变更？若是，附 docs/report/ 报告路径与 USER 批准记录。 -->

---

## Checklist

- [ ] I did not change architecture contracts merely to match existing implementation.
- [ ] Skeleton-level changes have explicit USER approval.
- [ ] New dependencies were justified.
- [ ] No cross-layer shortcut was introduced.
- [ ] Failure paths are defined, not just success paths.
- [ ] No WebView / Electron / Tauri / JS runtime / general async runtime / network access was introduced.
- [ ] All file writes use atomic replace; no user file can be auto-deleted.
