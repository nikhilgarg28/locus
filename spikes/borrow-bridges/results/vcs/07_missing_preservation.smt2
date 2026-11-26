; benchmark generated from python API
(set-info :status unknown)
(declare-sort K 0)
(declare-sort Q 0)
(declare-fun key_equiv (K K) Bool)
(declare-fun l () K)
(declare-fun k () K)
(declare-fun query_equiv (Q Q) Bool)
(declare-fun q () Q)
(declare-fun projection (K) Q)
(assert
 (key_equiv k l))
(assert
 (let ((?x8 (projection k)))
 (query_equiv ?x8 q)))
(assert
 (let ((?x16 (projection l)))
(let (($x29 (query_equiv ?x16 q)))
(not $x29))))
(check-sat)
