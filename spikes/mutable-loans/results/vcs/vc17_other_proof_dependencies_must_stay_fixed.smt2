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
(declare-fun changed_sibling_length () Int)
(assert
 (let (($x33 (forall ((probe Int) )(let (($x30 (>= probe 0)))
 (let (($x31 (and $x30 (< probe capacity))))
 (=> $x31 (= (select entry_initialized probe) (< probe length))))))
 ))
 (let (($x16 (>= length 0)))
 (and $x16 (<= length capacity) $x33))))
(assert
 (>= index 0))
(assert
 (< index length))
(assert
 (= final_initialized (store entry_initialized index true)))
(assert
 (= final_contents (store entry_contents index arbitrary_final_element)))
(assert
 (let (($x82 (forall ((probe Int) )(let (($x30 (>= probe 0)))
(let (($x31 (and $x30 (< probe capacity))))
(=> $x31 (= (select final_initialized probe) (< probe changed_sibling_length))))))
))
(let (($x224 (>= changed_sibling_length 0)))
(let (($x47 (and $x224 (<= changed_sibling_length capacity) $x82)))
(not $x47)))))
(check-sat)
