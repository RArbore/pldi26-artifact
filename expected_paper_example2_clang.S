	.file	"paper_example2.c"
	.text
	.globl	example2                        # -- Begin function example2
	.p2align	4
	.type	example2,@function
example2:                               # @example2
	.cfi_startproc
# %bb.0:
	movl	%edi, %ecx
	cmpl	$9, %edi
	jg	.LBB0_1
	.p2align	4
.LBB0_2:                                # =>This Inner Loop Header: Depth=1
	leal	5(%rcx), %edx
	movl	%ecx, %eax
	imull	%edx, %eax
	movl	%edi, %ecx
	imull	%edx, %ecx
	movl	%eax, %edi
	cmpl	$10, %ecx
	jl	.LBB0_2
# %bb.3:
	subl	%ecx, %eax
	retq
.LBB0_1:
	movl	%edi, %eax
	subl	%ecx, %eax
	retq
.Lfunc_end0:
	.size	example2, .Lfunc_end0-example2
	.cfi_endproc
                                        # -- End function
	.ident	"clang version 21.1.0 (https://github.com/llvm/llvm-project 3623fe661ae35c6c80ac221f14d85be76aa870f1)"
	.section	".note.GNU-stack","",@progbits
	.addrsig
