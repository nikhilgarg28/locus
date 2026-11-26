; benchmark generated from python API
(set-info :status unknown)
(declare-fun slot_at_acquisition_return () Int)
(declare-fun cached_element () Int)
(declare-fun arbitrary_final_element () Int)
(declare-fun memory_at_loan_end () Int)
(assert
 (= cached_element slot_at_acquisition_return))
(assert
 (= memory_at_loan_end arbitrary_final_element))
(assert
 (let (($x38 (= cached_element memory_at_loan_end)))
(not $x38)))
(check-sat)
