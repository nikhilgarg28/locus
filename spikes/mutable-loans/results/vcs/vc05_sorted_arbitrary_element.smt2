; benchmark generated from python API
(set-info :status unknown)
(declare-fun entry_right () Int)
(declare-fun entry_left () Int)
(declare-fun final_left () Int)
(assert
 (<= entry_left entry_right))
(assert
 (let (($x87 (<= final_left entry_right)))
(not $x87)))
(check-sat)
