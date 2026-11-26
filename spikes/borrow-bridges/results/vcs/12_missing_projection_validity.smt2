; benchmark generated from python API
(set-info :status unknown)
(declare-sort K 0)
(declare-sort Q 0)
(declare-sort Trace 0)
(declare-fun valid_key (K) Bool)
(declare-fun k () K)
(declare-fun valid_query (Q) Bool)
(declare-fun q () Q)
(declare-fun query_equiv (Q Q) Bool)
(declare-fun projection (K) Q)
(declare-fun query_trace (Q) Trace)
(declare-fun key_trace (K) Trace)
(assert
 (valid_key k))
(assert
 (valid_query q))
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
 (let ((?x8 (projection k)))
 (let ((?x11 (query_trace ?x8)))
 (let (($x14 (= ?x11 ?x13)))
 (let (($x9 (query_equiv ?x8 q)))
 (let (($x42 (valid_query q)))
 (let (($x17 (and (valid_query ?x8) $x42 $x9)))
 (=> $x17 $x14)))))))))
(assert
 (let ((?x13 (query_trace q)))
(let ((?x10 (key_trace k)))
(and (distinct ?x10 ?x13) true))))
(check-sat)
