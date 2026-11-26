; benchmark generated from python API
(set-info :status unknown)
(declare-fun length () Int)
(declare-fun entry_initialized () (Array Int Bool))
(declare-fun capacity () Int)
(declare-fun index () Int)
(declare-fun normal_return () Bool)
(assert
 (let (($x33 (forall ((probe Int) )(=> (and (>= probe 0) (< probe capacity)) (= (select entry_initialized probe) (< probe length))))
 ))
 (let (($x16 (>= length 0)))
 (and $x16 (<= length capacity) $x33))))
(assert
 (>= index 0))
(assert
 (< index length))
(assert
 (not normal_return))
(check-sat)
