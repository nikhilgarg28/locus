; benchmark generated from python API
(set-info :status unknown)
(declare-sort Q 0)
(declare-sort K 0)
(declare-fun query_equiv (Q Q) Bool)
(declare-fun q () Q)
(declare-fun projection (K) Q)
(declare-fun k () K)
(declare-fun l () K)
(declare-fun key_equiv (K K) Bool)
(assert
 (let ((?x8 (projection k)))
 (query_equiv ?x8 q)))
(assert
 (let ((?x16 (projection l)))
 (query_equiv ?x16 q)))
(assert
 (let ((?x16 (projection l)))
 (let (($x30 (query_equiv q ?x16)))
 (let (($x29 (query_equiv ?x16 q)))
 (=> $x29 $x30)))))
(assert
 (let ((?x16 (projection l)))
 (let ((?x8 (projection k)))
 (let (($x36 (query_equiv ?x8 ?x16)))
 (let (($x30 (query_equiv q ?x16)))
 (let (($x9 (query_equiv ?x8 q)))
 (let (($x21 (and $x9 $x30)))
 (=> $x21 $x36))))))))
(assert
 (let (($x41 (key_equiv k l)))
(not $x41)))
(check-sat)
