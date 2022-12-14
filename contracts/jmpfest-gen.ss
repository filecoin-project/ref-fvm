#!/usr/bin/env gxi

(import :std/iter
        :std/format
        :std/misc/shuffle)

(def (main what)
  (case what
    (("linear")
     (generate-linear))
    (("random")
     (generate-random))
    (else
     (error "I don't know how to generate " what))))

(def (generate-linear)
  (let* ((labels (make-labels))
         (targets (append (cdr labels) [(car labels)])))
    (with-output-to-file "jmpfest_linear_body.eas" (cut generate labels targets))))

(def (generate-random)
  (let* ((labels (make-labels))
         (targets (shuffle labels)))
    (with-output-to-file "jmpfest_random_body.eas" (cut generate labels targets))))

(def num-labels 1500)
(def num-iterations 1000000)

(def (make-labels)
  (map (cut format "L~a" <>) (iota num-labels)))

(def (generate labels targets)
  (ins "%push(~a)" num-iterations)
  (for ((lbl labels)
        (tgt targets))
    (ins "~a:" lbl)
    (ins "jumpdest")
    (ins "%push(1)")
    (ins "swap1")
    (ins "sub")
    (ins "dup1")
    (ins "iszero")
    (ins "%push(done)")
    (ins "jumpi")
    (ins "%push(~a)" tgt)
    (ins "jump"))
  (ins "done:")
  (ins "jumpdest")
  (ins "%push(0)")
  (ins "%push(0)")
  (ins "return"))

(def (ins fmt . args)
  (displayln (apply format fmt args)))
