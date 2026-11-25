; benchmark generated from python API
(set-info :status unknown)
(declare-fun length () Int)
(declare-fun entry_initialized () (Array Int Bool))
(declare-fun capacity () Int)
(declare-fun index () Int)
(declare-fun final_initialized () (Array Int Bool))
(assert
 (let (($x33 (forall ((probe Int) )(let (($x30 (>= probe 0)))
 (let (($x31 (and $x30 (< probe capacity))))
 (=> $x31 (= (select entry_initialized probe) (< probe length))))))
 ))
 (let (($x17 (<= length capacity)))
 (let (($x16 (>= length 0)))
 (and $x16 $x17 $x33)))))
(assert
 (>= index 0))
(assert
 (< index length))
(assert
 (let ((?x41 (store entry_initialized index false)))
 (= final_initialized ?x41)))
(assert
 (let (($x127 (forall ((probe Int) )(let (($x30 (>= probe 0)))
(let (($x31 (and $x30 (< probe capacity))))
(=> $x31 (= (select final_initialized probe) (< probe length))))))
))
(let (($x17 (<= length capacity)))
(let (($x16 (>= length 0)))
(let (($x536 (and $x16 $x17 $x127)))
(not $x536))))))
(check-sat)
