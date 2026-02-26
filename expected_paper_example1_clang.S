	.file	"paper_example1.c"
	.text
	.globl	example1                        # -- Begin function example1
	.p2align	4
	.type	example1,@function
example1:                               # @example1
	.cfi_startproc
# %bb.0:
                                        # kill: def $edi killed $edi def $rdi
	movl	$49, %eax
	cmpl	$9, %edi
	jg	.LBB0_8
# %bb.1:
	movl	$10, %ecx
	subl	%edi, %ecx
	cmpl	$9, %edi
	jne	.LBB0_3
# %bb.2:
	movl	$42, %eax
	jmp	.LBB0_5
.LBB0_3:
	movl	%ecx, %edx
	andl	$-2, %edx
	movl	$42, %eax
	.p2align	4
.LBB0_4:                                # =>This Inner Loop Header: Depth=1
	addl	$2, %edi
	addl	$-2, %edx
	jne	.LBB0_4
.LBB0_5:
	testb	$1, %cl
	je	.LBB0_7
# %bb.6:
	movl	%edi, %ecx
	incl	%ecx
	leal	(%rax,%rdi), %edx
	addl	$3, %edx
	imull	%ecx, %edx
	cmpl	%edx, %edx
	movl	$24, %ecx
	cmovel	%eax, %ecx
	movl	%ecx, %eax
.LBB0_7:
	addl	$7, %eax
.LBB0_8:
                                        # kill: def $eax killed $eax killed $rax
	retq
.Lfunc_end0:
	.size	example1, .Lfunc_end0-example1
	.cfi_endproc
                                        # -- End function
	.ident	"clang version 21.1.0 (https://github.com/llvm/llvm-project 3623fe661ae35c6c80ac221f14d85be76aa870f1)"
	.section	".note.GNU-stack","",@progbits
	.addrsig
