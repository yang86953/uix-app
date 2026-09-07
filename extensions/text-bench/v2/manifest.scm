;; 文本处理工作台扩展包 v2 清单（schema 1）。
;; 算法与界面相对 v1 共同升级：拉丁词 + CJK 字口径、新增 CJK 统计行；
;; 状态 schema 不变（1），热替换时草稿与摘要按合同迁移。
(uix-extension
  (schema-version 1)
  (id "text-bench")
  (version "0.2.0")
  (language r7rs-small)
  (entry "main.scm")
  (capabilities documents-query documents-commit mount-panel)
  (state-schema-version 1))
