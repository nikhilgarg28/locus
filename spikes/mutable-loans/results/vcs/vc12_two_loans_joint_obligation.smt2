; benchmark generated from python API
(set-info :status unknown)
(declare-fun entry_left () Int)
(declare-fun entry_right () Int)
(declare-fun final_left () Int)
(declare-fun final_right () Int)
(assert
 (= entry_left 0))
(assert
 (= entry_right 0))
(assert
 (<= (+ final_left entry_right) 10))
(assert
 (<= (+ entry_left final_right) 10))
(assert
 (let (($x123 (<= (+ final_left final_right) 10)))
(not $x123)))
(check-sat)
