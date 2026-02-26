	.file	"paper_example2.c"
	.text
	.p2align 4
	.globl	example2
	.type	example2, @function
example2:
.LFB0:
	.cfi_startproc
	cmpl	$9, %edi
	jg	.L4
	movl	%edi, %edx
	.p2align 5
	.p2align 4
	.p2align 3
.L3:
	movl	%edi, %eax
	movl	%edx, %edi
	leal	(%rdx,%rdx,4), %ecx
	imull	%edx, %edi
	addl	$5, %edx
	imull	%eax, %edx
	addl	%ecx, %edi
	cmpl	$9, %edx
	jle	.L3
	movl	%edi, %eax
	subl	%edx, %eax
	ret
	.p2align 4,,10
	.p2align 3
.L4:
	xorl	%eax, %eax
	ret
	.cfi_endproc
.LFE0:
	.size	example2, .-example2
	.ident	"GCC: (GNU) 15.2.0"
	.section	.note.GNU-stack,"",@progbits
