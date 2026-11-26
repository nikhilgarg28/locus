; benchmark generated from python API
(set-info :status unknown)
(declare-sort K 0)
(declare-sort Q 0)
(declare-fun key_cmp (K K) Int)
(declare-fun l () K)
(declare-fun k () K)
(declare-fun query_cmp (Q Q) Int)
(declare-fun projection (K) Q)
(declare-fun q () Q)
(declare-fun query_equiv (Q Q) Bool)
(assert
 (let ((?x55 (key_cmp k l)))
 (< ?x55 0)))
(assert
 (let ((?x16 (projection l)))
 (let ((?x8 (projection k)))
 (let ((?x26 (query_cmp ?x8 ?x16)))
 (let ((?x55 (key_cmp k l)))
 (= ?x55 ?x26))))))
(assert
 (let ((?x16 (projection l)))
 (let ((?x69 (query_cmp ?x16 q)))
 (< ?x69 0))))
(assert
 (let ((?x8 (projection k)))
 (let (($x9 (query_equiv ?x8 q)))
 (= $x9 (= (query_cmp ?x8 q) 0)))))
(assert
 (let ((?x8 (projection k)))
(query_equiv ?x8 q)))
(check-sat)
