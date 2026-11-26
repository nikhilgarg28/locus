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
(declare-fun probe () Int)
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
 (= final_initialized (store entry_initialized index true)))
(assert
 (= final_contents (store entry_contents index arbitrary_final_element)))
(assert
 (and (distinct probe index) true))
(assert
 (let (($x70 (= (select final_contents probe) (select entry_contents probe))))
(not $x70)))
(check-sat)
