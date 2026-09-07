;; 文本处理工作台扩展包 v1 清单（schema 1）。
;; 挂载位 panel 由宿主两侧（前台用户窗 / 后台操作面）各自显式开放。
(uix-extension
  (schema-version 1)
  (id "text-bench")
  (version "0.1.0")
  (language r7rs-small)
  (entry "main.scm")
  (capabilities documents-query documents-commit mount-panel)
  (state-schema-version 1))
