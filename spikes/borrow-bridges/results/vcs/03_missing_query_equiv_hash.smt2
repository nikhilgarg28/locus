; benchmark generated from python API
(set-info :status unknown)
(declare-sort Q 0)
(declare-sort K 0)
(declare-sort Trace 0)
(declare-fun query_equiv (Q Q) Bool)
(declare-fun q () Q)
(declare-fun projection (K) Q)
(declare-fun k () K)
(declare-fun query_trace (Q) Trace)
(declare-fun key_trace (K) Trace)
(assert
 (let ((?x8 (projection k)))
 (query_equiv ?x8 q)))
(assert
 (let ((?x8 (projection k)))
 (let ((?x11 (query_trace ?x8)))
 (let ((?x10 (key_trace k)))
 (= ?x10 ?x11)))))
(assert
 (let ((?x13 (query_trace q)))
(let ((?x10 (key_trace k)))
(and (distinct ?x10 ?x13) true))))
(check-sat)
