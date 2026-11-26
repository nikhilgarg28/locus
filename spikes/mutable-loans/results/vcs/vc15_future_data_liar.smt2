; benchmark generated from python API
(set-info :status unknown)
(declare-fun future_data () Int)
(assert
 (or (= future_data 0) (= future_data 1)))
(assert
 (= future_data (- 1 future_data)))
(assert
 (not false))
(check-sat)
