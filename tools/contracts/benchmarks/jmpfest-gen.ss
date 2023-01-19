#!/usr/bin/env gxi
;; This script was used to generate the bodies of the two jumpfest contracts,
;; (linear and random). You shouldn't have to run it, but it is here as reference
;; if you want to understand the logic behind the two contracts.
;; Some complain that it is written in an alien language, but it is a one-shot script
;; that took me 10 min to write (as opposed to the 3 hours and frustration it would
;; take me to write it in rust), so you'll have to deal. Rust is good(ish) for lot of things,
;; but suitable for quick hacking it is not.

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
    ;; verify that nothing jumps to itself
    (for ((lbl labels) (tgt targets))
      (when (equal? lbl tgt)
        (error "oh shit, self-jump; try again")))
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
