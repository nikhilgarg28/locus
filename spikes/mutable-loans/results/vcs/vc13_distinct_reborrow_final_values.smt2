; benchmark generated from python API
(set-info :status unknown)
(declare-fun inner_end () Int)
(declare-fun outer_end () Int)
(assert
 (= inner_end 1))
(assert
 (= outer_end 2))
(assert
 (let (($x598 (= inner_end outer_end)))
(not $x598)))
(check-sat)
