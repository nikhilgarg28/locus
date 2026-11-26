	.att_syntax
	.file	"cost.d571f25d6cf31769-cgu.0"
	.section	.text._RINvNtCs8uuN1nrGffB_4core3ptr9drop_glueNtCsika7EMGbNEH_4cost5GuardEBD_,"ax",@progbits
	.prefalign	4, .Lfunc_end0, nop
	.type	_RINvNtCs8uuN1nrGffB_4core3ptr9drop_glueNtCsika7EMGbNEH_4cost5GuardEBD_,@function
_RINvNtCs8uuN1nrGffB_4core3ptr9drop_glueNtCsika7EMGbNEH_4cost5GuardEBD_:
	.cfi_startproc
	pushq	%rax
	.cfi_def_cfa_offset 16
	testb	$1, %dil
	je	.LBB0_3
	movq	_RNvNtNtCsh0g7Hz38UNf_3std9panicking11panic_count18GLOBAL_PANIC_COUNT@GOTPCREL(%rip), %rax
	movq	(%rax), %rax
	shlq	%rax
	testq	%rax, %rax
	jne	.LBB0_2
.LBB0_3:
	popq	%rax
	.cfi_def_cfa_offset 8
	retq
.LBB0_2:
	.cfi_def_cfa_offset 16
	callq	*_RNvNtNtCsh0g7Hz38UNf_3std9panicking11panic_count17is_zero_slow_path@GOTPCREL(%rip)
	testb	%al, %al
	jne	.LBB0_3
	callq	*_RNvNtCsh0g7Hz38UNf_3std7process5abort@GOTPCREL(%rip)
.Lfunc_end0:
	.size	_RINvNtCs8uuN1nrGffB_4core3ptr9drop_glueNtCsika7EMGbNEH_4cost5GuardEBD_, .Lfunc_end0-_RINvNtCs8uuN1nrGffB_4core3ptr9drop_glueNtCsika7EMGbNEH_4cost5GuardEBD_
	.cfi_endproc

	.section	.text._RNvXCsika7EMGbNEH_4costNtB2_5GuardNtNtNtCs8uuN1nrGffB_4core3ops4drop4Drop4drop,"ax",@progbits
	.globl	_RNvXCsika7EMGbNEH_4costNtB2_5GuardNtNtNtCs8uuN1nrGffB_4core3ops4drop4Drop4drop
	.prefalign	4, .Lfunc_end1, nop
	.type	_RNvXCsika7EMGbNEH_4costNtB2_5GuardNtNtNtCs8uuN1nrGffB_4core3ops4drop4Drop4drop,@function
_RNvXCsika7EMGbNEH_4costNtB2_5GuardNtNtNtCs8uuN1nrGffB_4core3ops4drop4Drop4drop:
	.cfi_startproc
	pushq	%rax
	.cfi_def_cfa_offset 16
	cmpb	$0, (%rdi)
	je	.LBB1_3
	movq	_RNvNtNtCsh0g7Hz38UNf_3std9panicking11panic_count18GLOBAL_PANIC_COUNT@GOTPCREL(%rip), %rax
	movq	(%rax), %rax
	shlq	%rax
	testq	%rax, %rax
	jne	.LBB1_2
.LBB1_3:
	popq	%rax
	.cfi_def_cfa_offset 8
	retq
.LBB1_2:
	.cfi_def_cfa_offset 16
	callq	*_RNvNtNtCsh0g7Hz38UNf_3std9panicking11panic_count17is_zero_slow_path@GOTPCREL(%rip)
	testb	%al, %al
	jne	.LBB1_3
	callq	*_RNvNtCsh0g7Hz38UNf_3std7process5abort@GOTPCREL(%rip)
.Lfunc_end1:
	.size	_RNvXCsika7EMGbNEH_4costNtB2_5GuardNtNtNtCs8uuN1nrGffB_4core3ops4drop4Drop4drop, .Lfunc_end1-_RNvXCsika7EMGbNEH_4costNtB2_5GuardNtNtNtCs8uuN1nrGffB_4core3ops4drop4Drop4drop
	.cfi_endproc

	.section	.text.guarded,"ax",@progbits
	.globl	guarded
	.prefalign	4, .Lfunc_end2, nop
	.type	guarded,@function
guarded:
.Lfunc_begin0:
	.cfi_startproc
	.cfi_personality 155, DW.ref.rust_eh_personality
	.cfi_lsda 27, .Lexception0
	pushq	%rbx
	.cfi_def_cfa_offset 16
	.cfi_offset %rbx, -16
.Ltmp0:
	callq	*acquisition@GOTPCREL(%rip)
.Ltmp1:
	popq	%rbx
	.cfi_def_cfa_offset 8
	retq
.LBB2_3:
	.cfi_def_cfa_offset 16
.Ltmp2:
	movq	%rax, %rbx
.Ltmp3:
	movl	$1, %edi
	callq	_RINvNtCs8uuN1nrGffB_4core3ptr9drop_glueNtCsika7EMGbNEH_4cost5GuardEBD_
.Ltmp4:
	movq	%rbx, %rdi
	callq	_Unwind_Resume@PLT
.LBB2_2:
.Ltmp5:
	callq	*_RNvNtCs8uuN1nrGffB_4core9panicking16panic_in_cleanup@GOTPCREL(%rip)
.Lfunc_end2:
	.size	guarded, .Lfunc_end2-guarded
	.cfi_endproc
	.section	.gcc_except_table.guarded,"a",@progbits
	.p2align	2, 0x0
GCC_except_table2:
.Lexception0:
	.byte	255
	.byte	155
	.uleb128 .Lttbase0-.Lttbaseref0
.Lttbaseref0:
	.byte	1
	.uleb128 .Lcst_end0-.Lcst_begin0
.Lcst_begin0:
	.uleb128 .Ltmp0-.Lfunc_begin0
	.uleb128 .Ltmp1-.Ltmp0
	.uleb128 .Ltmp2-.Lfunc_begin0
	.byte	0
	.uleb128 .Ltmp3-.Lfunc_begin0
	.uleb128 .Ltmp4-.Ltmp3
	.uleb128 .Ltmp5-.Lfunc_begin0
	.byte	1
	.uleb128 .Ltmp4-.Lfunc_begin0
	.uleb128 .Lfunc_end2-.Ltmp4
	.byte	0
	.byte	0
.Lcst_end0:
	.byte	127
	.byte	0
	.p2align	2, 0x0
.Lttbase0:
	.byte	0
	.p2align	2, 0x0

	.section	.text.unguarded,"ax",@progbits
	.globl	unguarded
	.prefalign	4, .Lfunc_end3, nop
	.type	unguarded,@function
unguarded:
	.cfi_startproc
	jmpq	*acquisition@GOTPCREL(%rip)
.Lfunc_end3:
	.size	unguarded, .Lfunc_end3-unguarded
	.cfi_endproc

	.hidden	DW.ref.rust_eh_personality
	.weak	DW.ref.rust_eh_personality
	.section	.data.DW.ref.rust_eh_personality,"awG",@progbits,DW.ref.rust_eh_personality,comdat
	.p2align	3, 0x0
	.type	DW.ref.rust_eh_personality,@object
	.size	DW.ref.rust_eh_personality, 8
DW.ref.rust_eh_personality:
	.quad	rust_eh_personality
	.ident	"rustc version 1.100.0 (0ef1bc386 2026-11-09)"
	.section	".note.GNU-stack","",@progbits
