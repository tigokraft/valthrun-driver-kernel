// .btext is at the beginning on the code section
.section .btext
.global _seh_invoke
_seh_invoke:
mov rax, [rsp]
push rbx
push rsi
push rdi
push rbp
push r10
push r11
push r12
push r13
push r14
push r15
sub rsp, 0x28

mov [rsp], rax
call _inner

add rsp, 0x28
pop r15
pop r14
pop r13
pop r12
pop r11
pop r10
pop rbp
pop rdi
pop rsi
pop rbx
ret

_inner:
mov r10, rcx
push [r10]
mov rcx, [r10+0x10]
jmp [r10 + 0x08]