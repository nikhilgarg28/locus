; benchmark generated from python API
(set-info :status unknown)
(declare-sort Q 0)
(declare-sort K 0)
(declare-fun projection (K) Q)
(declare-fun k () K)
(declare-fun actual_borrow (K) Q)
(declare-fun query_equiv (Q Q) Bool)
(declare-fun q () Q)
(assert
 (let ((?x8 (projection k)))
 (let ((?x23 (actual_borrow k)))
 (= ?x23 ?x8))))
(assert
 (let ((?x8 (projection k)))
 (query_equiv ?x8 q)))
(assert
 (not (query_equiv (actual_borrow k) q)))
(check-sat)
