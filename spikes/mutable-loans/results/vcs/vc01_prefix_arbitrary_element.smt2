; benchmark generated from python API
(set-info :status unknown)
(declare-fun length () Int)
(declare-fun entry_initialized () (Array Int Bool))
(declare-fun capacity () Int)
(declare-fun index () Int)
(declare-fun final_initialized () (Array Int Bool))
(declare-fun arbitrary_final_element () Int)
(declare-fun entry_contents () (Array Int Int))
(declare-fun final_contents () (Array Int Int))
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
 (= final_initialized (store entry_initialized index true)))
(assert
 (= final_contents (store entry_contents index arbitrary_final_element)))
(assert
 (let (($x44 (forall ((probe Int) )(let (($x30 (>= probe 0)))
(let (($x31 (and $x30 (< probe capacity))))
(=> $x31 (= (select final_initialized probe) (< probe length))))))
))
(let (($x17 (<= length capacity)))
(let (($x16 (>= length 0)))
(let (($x34 (and $x16 $x17 $x44)))
(not $x34))))))
(check-sat)
