;; 文本处理工作台扩展 v1：空白分段词数口径。
;; 宿主授权端口：documents-query（只读）、documents-commit（受控写）。
(define draft "")
(define result "尚未处理")
(define target "intro")
(define base-version 0)
(define status "就绪")

;; ---- 文本统计算法 ----
(define (count-if pred s)
  (define (walk i n)
    (if (= i (string-length s))
        n
        (walk (+ i 1) (if (pred (string-ref s i)) (+ n 1) n))))
  (walk 0 0))
(define (non-space-count s)
  (count-if (lambda (c) (not (char-whitespace? c))) s))
(define (line-count s)
  (+ 1 (count-if (lambda (c) (char=? c #\newline)) s)))
;; 词数 = 空白分段的连续非空白段数：进入段时计数一次，
;; 字符串结束时不再补计（修复进入与结束的重复计数）。
(define (word-count s)
  (define (walk i in-word n)
    (if (= i (string-length s))
        n
        (let ((space (char-whitespace? (string-ref s i))))
          (cond ((and (not space) (not in-word)) (walk (+ i 1) #t (+ n 1)))
                ((and space in-word) (walk (+ i 1) #f n))
                (else (walk (+ i 1) in-word n))))))
  (walk 0 #f 0))

;; ---- 空白规范化：压缩连续空白为单空格并去首尾 ----
(define (collapse-chars chars acc)
  (cond ((null? chars) (list->string (reverse acc)))
        ((char-whitespace? (car chars))
         (if (or (null? acc) (char=? (car acc) #\space))
             (collapse-chars (cdr chars) acc)
             (collapse-chars (cdr chars) (cons #\space acc))))
        (else (collapse-chars (cdr chars) (cons (car chars) acc)))))
(define (trim-tail s)
  (define (drop i)
    (if (and (> i 0) (char-whitespace? (string-ref s (- i 1))))
        (drop (- i 1))
        i))
  (substring s 0 (drop (string-length s))))
(define (normalize-text s)
  (trim-tail (collapse-chars (string->list s) '())))

(define (stats->text s)
  (string-append
   "字符 " (number->string (string-length s))
   " · 非空白 " (number->string (non-space-count s))
   " · 行 " (number->string (line-count s))
   " · 词 " (number->string (word-count s))))

;; ---- 动态面板 ----
(define (draft-input reset?)
  (if reset?
      (list 'input (list 'key "draft")
            (list 'placeholder "输入或读取要处理的文本")
            (list 'on-change 'on-draft-change)
            (list 'reset #t)
            draft)
      (list 'input (list 'key "draft")
            (list 'placeholder "输入或读取要处理的文本")
            (list 'on-change 'on-draft-change)
            draft)))
(define (declaration reset?)
  (list 'column (list 'key "root") (list 'pad 12.0) (list 'gap 8.0)
        (list 'text (list 'key "title") (list 'size 14.0) "文本处理工作台 v1")
        (draft-input reset?)
        (list 'row (list 'key "controls") (list 'gap 8.0)
              (list 'button (list 'key "process") (list 'on-click 'on-process) "处理文本")
              (list 'button (list 'key "load") (list 'on-click 'on-load) "读取文档")
              (list 'button (list 'key "commit") (list 'on-click 'on-commit) "提交到文档"))
        (list 'text (list 'key "stats") (string-append "统计: " result))
        (list 'text (list 'key "status") (string-append "状态: " status))
        (list 'text (list 'key "doc")
              (string-append "目标: " target " @v" (number->string base-version)))))

;; ---- 面板事件 ----
(define (on-draft-change text)
  (set! draft text)
  (submit-ui! 'panel (declaration #f)))
(define (on-process)
  (set! result (stats->text draft))
  (set! status "已处理")
  (submit-ui! 'panel (declaration #f)))
(define (on-load)
  (let* ((entry (documents-query target))
         (content (list-ref entry 0))
         (version (list-ref entry 1)))
    (set! base-version version)
    (set! draft content)
    (set! status (string-append "已读取 v" (number->string version)))
    (submit-ui! 'panel (declaration #t))))
(define (on-commit)
  (let ((outcome (documents-commit target base-version (normalize-text draft))))
    (if (eq? (list-ref outcome 0) 'ok)
        (begin
          (set! base-version (list-ref outcome 1))
          (set! status (string-append "已提交 v" (number->string (list-ref outcome 1)))))
        (set! status (string-append "版本冲突：文档已是 v"
                                    (number->string (list-ref outcome 1)))))
    (submit-ui! 'panel (declaration #f))))
(register-handler! "on-draft-change" on-draft-change)
(register-handler! "on-process" on-process)
(register-handler! "on-load" on-load)
(register-handler! "on-commit" on-commit)

;; ---- 无窗口命令（逻辑部分与面板共用同一实例）----
(register-command! "stats" (lambda (text) (stats->text text)))
(register-command! "normalize" (lambda (text) (normalize-text text)))
(register-command! "engine-version" (lambda () "0.1.0"))

;; ---- 状态迁移（schema 1：草稿与摘要跨代保留）----
(register-state-export!
  (lambda () (list draft result target base-version status)))
(register-state-import!
  (lambda (snapshot)
    (set! draft (list-ref snapshot 0))
    (set! result (list-ref snapshot 1))
    (set! target (list-ref snapshot 2))
    (set! base-version (list-ref snapshot 3))
    (set! status (list-ref snapshot 4))
    (submit-ui! 'panel (declaration #t))))
(submit-ui! 'panel (declaration #f))
