; benchmark generated from python API
(set-info :status unknown)
(declare-fun entry_left () Int)
(declare-fun entry_right () Int)
(declare-fun final_left () Int)
(assert
 (>= entry_left 0))
(assert
 (>= entry_right 0))
(assert
 (<= (+ entry_left entry_right) 10))
(assert
 (>= final_left 0))
(assert
 (let (($x81 (<= (+ final_left entry_right) 10)))
(not $x81)))
(check-sat)
