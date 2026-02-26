	.file	"paper_example1.c"
	.text
	.p2align 4
	.globl	example1
	.type	example1, @function
example1:
.LFB0:
	.cfi_startproc
	cmpl	$9, %edi
	jg	.L5
	movl	$42, %eax
	movl	$24, %r8d
	.p2align 5
	.p2align 4
	.p2align 3
.L4:
	movl	%edi, %esi
	addl	$1, %edi
	leal	(%rdi,%rax), %edx
	imull	%edi, %edx
	leal	(%rdx,%rdi,2), %ecx
	leal	3(%rax,%rsi), %edx
	imull	%edi, %edx
	cmpl	%edx, %ecx
	cmovne	%r8d, %eax
	cmpl	$10, %edi
	jne	.L4
	addl	$7, %eax
	ret
	.p2align 4,,10
	.p2align 3
.L5:
	movl	$49, %eax
	ret
	.cfi_endproc
.LFE0:
	.size	example1, .-example1
	.ident	"GCC: (GNU) 15.2.0"
	.section	.note.GNU-stack,"",@progbits
