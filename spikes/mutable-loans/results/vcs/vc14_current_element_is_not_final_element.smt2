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
(declare-fun slot_at_acquisition_return () Int)
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
 (and (distinct slot_at_acquisition_return arbitrary_final_element) true))
(assert
 (let (($x661 (= (select final_contents index) slot_at_acquisition_return)))
(not $x661)))
(check-sat)
