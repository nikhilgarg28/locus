; benchmark generated from python API
(set-info :status unknown)
(declare-sort K 0)
(declare-sort Q 0)
(declare-fun key_cmp (K K) Int)
(declare-fun l () K)
(declare-fun k () K)
(declare-fun query_cmp (Q Q) Int)
(declare-fun projection (K) Q)
(assert
 (let ((?x55 (key_cmp k l)))
 (>= ?x55 (- 1))))
(assert
 (let ((?x55 (key_cmp k l)))
 (<= ?x55 1)))
(assert
 (let ((?x16 (projection l)))
 (let ((?x8 (projection k)))
 (let ((?x26 (query_cmp ?x8 ?x16)))
 (>= ?x26 (- 1))))))
(assert
 (let ((?x16 (projection l)))
 (let ((?x8 (projection k)))
 (let ((?x26 (query_cmp ?x8 ?x16)))
 (<= ?x26 1)))))
(assert
 (= (= (key_cmp k l) 0) (= (query_cmp (projection k) (projection l)) 0)))
(assert
 (let ((?x16 (projection l)))
(let ((?x8 (projection k)))
(let ((?x26 (query_cmp ?x8 ?x16)))
(let ((?x55 (key_cmp k l)))
(and (distinct ?x55 ?x26) true))))))
(check-sat)
