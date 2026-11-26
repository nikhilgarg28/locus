; benchmark generated from python API
(set-info :status unknown)
(declare-fun entry_contents () (Array Int Int))
(declare-fun length () Int)
(declare-fun index () Int)
(declare-fun arbitrary_final_element () Int)
(assert
 (forall ((probe Int) )(let ((?x65 (select entry_contents probe)))
 (let (($x30 (>= probe 0)))
 (let (($x84 (and $x30 (< (+ probe 1) length))))
 (=> $x84 (<= ?x65 (select entry_contents (+ probe 1))))))))
 )
(assert
 (>= index 0))
(assert
 (< index length))
(assert
 (let (($x72 (<= (select entry_contents (- index 1)) arbitrary_final_element)))
 (=> (> index 0) $x72)))
(assert
 (let (($x49 (<= arbitrary_final_element (select entry_contents (+ index 1)))))
 (=> (< (+ index 1) length) $x49)))
(assert
 (let (($x34 (forall ((probe Int) )(let ((?x47 (+ probe 1)))
(let ((?x23 (store entry_contents index arbitrary_final_element)))
(let ((?x85 (select ?x23 probe)))
(let (($x30 (>= probe 0)))
(let (($x84 (and $x30 (< ?x47 length))))
(=> $x84 (<= ?x85 (select ?x23 ?x47)))))))))
))
(not $x34)))
(check-sat)
